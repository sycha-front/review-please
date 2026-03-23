use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
        Arc, Mutex, RwLock,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};

#[cfg(test)]
use crate::models::SyncState;

use crate::{
    config::AppConfig,
    db::ReviewStore,
    models::{TrayState, utc_now_string},
    providers::{GithubProvider, SlackProvider},
    services::{
        github_events::{self, GITHUB_SYNC_SOURCE},
        notification::NotificationService,
        slack_ingest::{self, SLACK_SYNC_SOURCE},
    },
    tray::TrayController,
};

pub trait SyncCoordinator: Send + Sync {
    fn start(&self) -> Result<()>;
    fn stop(&self) -> Result<()>;
    fn sync_now(&self) -> Result<()>;
    fn refresh_tray(&self) -> Result<()>;
    fn last_error(&self) -> Option<String>;
    fn status_label(&self) -> String;
}

#[derive(Debug)]
enum ControlMessage {
    SyncNow,
    Stop,
}

#[derive(Debug, Clone)]
struct StatusSnapshot {
    status: String,
    last_error: Option<String>,
}

pub struct LocalSyncCoordinator {
    config: Arc<RwLock<AppConfig>>,
    store: Arc<dyn ReviewStore>,
    slack_provider: Arc<dyn SlackProvider>,
    github_provider: Arc<dyn GithubProvider>,
    notifications: Arc<dyn NotificationService>,
    tray: Arc<TrayController>,
    worker_tx: Mutex<Option<Sender<ControlMessage>>>,
    worker_handle: Mutex<Option<JoinHandle<()>>>,
    status: Arc<Mutex<StatusSnapshot>>,
    running: Arc<AtomicBool>,
}

impl LocalSyncCoordinator {
    pub fn new(
        config: Arc<RwLock<AppConfig>>,
        store: Arc<dyn ReviewStore>,
        slack_provider: Arc<dyn SlackProvider>,
        github_provider: Arc<dyn GithubProvider>,
        notifications: Arc<dyn NotificationService>,
        tray: Arc<TrayController>,
    ) -> Self {
        Self {
            config,
            store,
            slack_provider,
            github_provider,
            notifications,
            tray,
            worker_tx: Mutex::new(None),
            worker_handle: Mutex::new(None),
            status: Arc::new(Mutex::new(StatusSnapshot {
                status: "OK".to_string(),
                last_error: None,
            })),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    fn current_config(&self) -> AppConfig {
        self.config.read().expect("config lock").clone()
    }

    fn set_status(&self, status: &str, last_error: Option<String>) -> Result<()> {
        {
            let mut snapshot = self.status.lock().expect("status lock");
            snapshot.status = status.to_string();
            snapshot.last_error = last_error;
        }
        self.refresh_tray()
    }

    fn snapshot(&self) -> Result<TrayState> {
        let status = self.status.lock().expect("status lock").clone();
        self.store.tray_state(&status.status, status.last_error)
    }

    fn record_failure(&self, source: &str, error: &str) -> Result<u64> {
        let mut sync_state = self.store.get_sync_state(source)?;
        sync_state.last_polled_at = Some(utc_now_string());
        sync_state.last_error = Some(error.to_string());
        sync_state.consecutive_failures += 1;
        self.store.save_sync_state(&sync_state)?;
        Ok(sync_state.consecutive_failures)
    }

    fn run_slack_sync(&self) -> Result<u64> {
        let config = self.current_config();
        match slack_ingest::run(
            &config,
            self.store.clone(),
            self.slack_provider.clone(),
            self.github_provider.clone(),
        ) {
            Ok(outcome) => {
                if outcome.new_pending_count > 0 && config.notify_on_new_pending {
                    let _ = self.notifications.notify(
                        "pr-please",
                        &format!("{} new review requests", outcome.new_pending_count),
                    );
                }
                Ok(outcome.new_pending_count)
            }
            Err(error) => {
                let failures = self.record_failure(SLACK_SYNC_SOURCE, &error.to_string())?;
                if failures == 3 && config.notify_on_errors {
                    let _ = self.notifications.notify("pr-please", "Slack sync failed");
                }
                Err(error)
            }
        }
    }

    fn run_github_sync(&self) -> Result<u64> {
        let config = self.current_config();
        match github_events::run(&config, self.store.clone(), self.github_provider.clone()) {
            Ok(outcome) => {
                if config.notify_on_done {
                    for pr_key in &outcome.completed_pr_keys {
                        let _ = self.notifications.notify(
                            "pr-please",
                            &format!("Review completed for {pr_key}"),
                        );
                    }
                }
                Ok(outcome.completed_request_count)
            }
            Err(error) => {
                let failures = self.record_failure(GITHUB_SYNC_SOURCE, &error.to_string())?;
                if failures == 3 && config.notify_on_errors {
                    let _ = self.notifications.notify("pr-please", "GitHub sync failed");
                }
                Err(error)
            }
        }
    }

    fn worker_loop(self: Arc<Self>, rx: Receiver<ControlMessage>) {
        let mut next_slack = Instant::now();
        let mut next_github = Instant::now();

        while self.running.load(Ordering::SeqCst) {
            let now = Instant::now();
            let due_slack = now >= next_slack;
            let due_github = now >= next_github;

            if due_slack || due_github {
                let mut last_error = None;
                let config = self.current_config();
                let _ = self.set_status("Syncing", None);
                if due_slack {
                    if let Err(error) = self.run_slack_sync() {
                        last_error = Some(error.to_string());
                    }
                    next_slack = Instant::now() + Duration::from_secs(config.slack_poll_interval_seconds);
                }
                if due_github {
                    if let Err(error) = self.run_github_sync() {
                        last_error = Some(error.to_string());
                    }
                    let poll_interval = self
                        .store
                        .get_sync_state(GITHUB_SYNC_SOURCE)
                        .ok()
                        .and_then(|state| state.github_poll_interval_seconds)
                        .unwrap_or(config.github_min_poll_interval_seconds);
                    next_github = Instant::now() + Duration::from_secs(poll_interval);
                }
                if let Some(error) = last_error {
                    let _ = self.set_status("Error", Some(error));
                } else {
                    let _ = self.set_status("OK", None);
                }
            }

            let timeout = Duration::from_secs(1);
            match rx.recv_timeout(timeout) {
                Ok(ControlMessage::SyncNow) => {
                    next_slack = Instant::now();
                    next_github = Instant::now();
                }
                Ok(ControlMessage::Stop) => break,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        self.running.store(false, Ordering::SeqCst);
    }
}

impl SyncCoordinator for LocalSyncCoordinator {
    fn start(&self) -> Result<()> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        let (tx, rx) = mpsc::channel();
        *self.worker_tx.lock().expect("worker tx") = Some(tx);
        let coordinator = Arc::new(Self {
            config: self.config.clone(),
            store: self.store.clone(),
            slack_provider: self.slack_provider.clone(),
            github_provider: self.github_provider.clone(),
            notifications: self.notifications.clone(),
            tray: self.tray.clone(),
            worker_tx: Mutex::new(None),
            worker_handle: Mutex::new(None),
            status: self.status.clone(),
            running: self.running.clone(),
        });
        let thread_coordinator = coordinator.clone();
        let handle = thread::spawn(move || thread_coordinator.worker_loop(rx));
        *self.worker_handle.lock().expect("worker handle") = Some(handle);
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        self.running.store(false, Ordering::SeqCst);
        if let Some(tx) = self.worker_tx.lock().expect("worker tx").take() {
            let _ = tx.send(ControlMessage::Stop);
        }
        if let Some(handle) = self.worker_handle.lock().expect("worker handle").take() {
            handle.join().map_err(|_| anyhow::anyhow!("sync thread panicked"))?;
        }
        Ok(())
    }

    fn sync_now(&self) -> Result<()> {
        let tx = self
            .worker_tx
            .lock()
            .expect("worker tx")
            .as_ref()
            .cloned()
            .context("sync worker is not running")?;
        tx.send(ControlMessage::SyncNow)
            .context("failed to request sync")
    }

    fn refresh_tray(&self) -> Result<()> {
        self.tray.update(&self.snapshot()?)
    }

    fn last_error(&self) -> Option<String> {
        self.status.lock().expect("status").last_error.clone()
    }

    fn status_label(&self) -> String {
        self.status.lock().expect("status").status.clone()
    }
}

#[cfg(test)]
pub fn mark_sync_failure(mut state: SyncState, message: &str) -> SyncState {
    state.last_polled_at = Some(utc_now_string());
    state.last_error = Some(message.to_string());
    state.consecutive_failures += 1;
    state
}

#[cfg(test)]
mod tests {
    use crate::models::SyncState;

    use super::mark_sync_failure;

    #[test]
    fn increments_failure_counter() {
        let state = SyncState::new("github_notifications");
        let failed = mark_sync_failure(state, "boom");
        assert_eq!(failed.consecutive_failures, 1);
        assert_eq!(failed.last_error.as_deref(), Some("boom"));
    }
}
