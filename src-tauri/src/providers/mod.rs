pub mod github;
pub mod slack;

use anyhow::Result;

use crate::models::{
    GithubEvent, GithubNotificationThread, GithubPullRef, NotificationsPollResult,
    PullRequestMetadata, SlackMessageRef, SyncState,
};

pub trait SlackProvider: Send + Sync {
    fn search_messages(&self, keyword: &str) -> Result<Vec<SlackMessageRef>>;
    fn fetch_user_display_name(&self, user_id: &str) -> Result<Option<String>>;
    fn fetch_permalink(&self, channel_id: &str, message_ts: &str) -> Result<Option<String>>;
}

pub trait GithubProvider: Send + Sync {
    fn current_user_login(&self) -> Result<String>;
    fn fetch_pr_metadata(&self, pull: &GithubPullRef) -> Result<PullRequestMetadata>;
    fn fetch_notifications(
        &self,
        sync_state: &SyncState,
        min_poll_interval_seconds: u64,
    ) -> Result<NotificationsPollResult>;
    fn fetch_events_for_thread(
        &self,
        thread: &GithubNotificationThread,
        since: Option<&str>,
        current_user_login: &str,
        include_comment_events: bool,
    ) -> Result<Vec<GithubEvent>>;
}
