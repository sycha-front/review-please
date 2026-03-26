use std::{collections::HashSet, sync::Arc};

use anyhow::{anyhow, Result};

use crate::{
    config::{self, AppConfig},
    db::{ReviewStore, SqliteStore},
    keychain::{CredentialStore, SecurityCredentialStore, GITHUB_TOKEN_ACCOUNT, SLACK_TOKEN_ACCOUNT},
    providers::{github::LocalGithubProvider, slack::LocalSlackProvider, GithubProvider, SlackProvider},
    services::{
        github_events,
        slack_ingest,
    },
};

pub enum CliCommand {
    Setup(SetupArgs),
    Doctor,
    Dump { format: String },
    SyncOnce,
    ResetState,
    ClearCredentials,
    Help,
}

pub struct SetupArgs {
    pub keyword: String,
    pub slack_token: String,
    pub github_token: String,
    pub slack_poll_interval_seconds: Option<u64>,
    pub github_min_poll_interval_seconds: Option<u64>,
    pub done_menu_limit: Option<usize>,
}

impl CliCommand {
    pub fn from_env<I>(args: I) -> Result<Option<Self>>
    where
        I: IntoIterator<Item = String>,
    {
        let values: Vec<String> = args.into_iter().collect();
        if values.is_empty() {
            return Ok(None);
        }
        let command = values[0].as_str();
        match command {
            "setup" => Ok(Some(Self::Setup(parse_setup(&values[1..])?))),
            "doctor" => Ok(Some(Self::Doctor)),
            "dump" => {
                let mut format = "json".to_string();
                let mut iter = values[1..].iter();
                while let Some(flag) = iter.next() {
                    if flag == "--format" {
                        format = iter
                            .next()
                            .ok_or_else(|| anyhow!("missing value for --format"))?
                            .clone();
                    }
                }
                Ok(Some(Self::Dump { format }))
            }
            "sync-once" => Ok(Some(Self::SyncOnce)),
            "reset-state" => Ok(Some(Self::ResetState)),
            "clear-credentials" => Ok(Some(Self::ClearCredentials)),
            "help" | "--help" | "-h" => Ok(Some(Self::Help)),
            _ => Err(anyhow!("unknown command `{command}`; run `review-please help`")),
        }
    }
}

pub fn run(command: CliCommand) -> Result<()> {
    match command {
        CliCommand::Setup(args) => run_setup(args),
        CliCommand::Doctor => run_doctor(),
        CliCommand::Dump { format } => run_dump(&format),
        CliCommand::SyncOnce => run_sync_once(),
        CliCommand::ResetState => run_reset_state(),
        CliCommand::ClearCredentials => run_clear_credentials(),
        CliCommand::Help => {
            print_help();
            Ok(())
        }
    }
}

fn parse_setup(args: &[String]) -> Result<SetupArgs> {
    let mut keyword = None;
    let mut slack_token = None;
    let mut github_token = None;
    let mut slack_poll_interval_seconds = None;
    let mut github_min_poll_interval_seconds = None;
    let mut done_menu_limit = None;

    let mut iter = args.iter();
    while let Some(flag) = iter.next() {
        let value = iter
            .next()
            .ok_or_else(|| anyhow!("missing value for {flag}"))?;
        match flag.as_str() {
            "--keyword" => keyword = Some(value.clone()),
            "--slack-token" => slack_token = Some(value.clone()),
            "--github-token" => github_token = Some(value.clone()),
            "--slack-poll-seconds" => slack_poll_interval_seconds = Some(value.parse::<u64>()?),
            "--github-poll-seconds" => github_min_poll_interval_seconds = Some(value.parse::<u64>()?),
            "--done-menu-limit" => done_menu_limit = Some(value.parse::<usize>()?),
            _ => return Err(anyhow!("unknown setup flag `{flag}`")),
        }
    }

    Ok(SetupArgs {
        keyword: keyword.ok_or_else(|| anyhow!("--keyword is required"))?,
        slack_token: slack_token.ok_or_else(|| anyhow!("--slack-token is required"))?,
        github_token: github_token.ok_or_else(|| anyhow!("--github-token is required"))?,
        slack_poll_interval_seconds,
        github_min_poll_interval_seconds,
        done_menu_limit,
    })
}

fn print_help() {
    println!(
        "review-please commands:
  review-please setup --keyword <value> --slack-token <token> --github-token <token> [--slack-poll-seconds <n>] [--github-poll-seconds <n>] [--done-menu-limit <n>]
  review-please doctor
  review-please dump --format json
  review-please sync-once
  review-please reset-state
  review-please clear-credentials"
    );
}

fn store() -> Result<Arc<dyn ReviewStore>> {
    let store = SqliteStore::from_default_location()?;
    store.init_schema()?;
    Ok(Arc::new(store))
}

fn credentials() -> Arc<dyn CredentialStore> {
    Arc::new(SecurityCredentialStore)
}

fn slack_provider(credentials: Arc<dyn CredentialStore>) -> Arc<dyn SlackProvider> {
    Arc::new(LocalSlackProvider::new(credentials))
}

fn github_provider(credentials: Arc<dyn CredentialStore>) -> Arc<dyn GithubProvider> {
    Arc::new(LocalGithubProvider::new(credentials))
}

fn run_setup(args: SetupArgs) -> Result<()> {
    let credentials = credentials();
    credentials.set(SLACK_TOKEN_ACCOUNT, &args.slack_token)?;
    credentials.set(GITHUB_TOKEN_ACCOUNT, &args.github_token)?;

    let mut config = AppConfig::default();
    config.slack_mention_keyword = args.keyword;
    if let Some(value) = args.slack_poll_interval_seconds {
        config.slack_poll_interval_seconds = value;
    }
    if let Some(value) = args.github_min_poll_interval_seconds {
        config.github_min_poll_interval_seconds = value;
    }
    if let Some(value) = args.done_menu_limit {
        config.done_menu_limit = value;
    }
    config.validate()?;
    let path = config.save()?;
    println!("saved config to {}", path.display());
    Ok(())
}

fn run_doctor() -> Result<()> {
    let config = AppConfig::load_or_default()?;
    let credentials = credentials();
    let slack_token = credentials.get(SLACK_TOKEN_ACCOUNT)?.is_some();
    let github_token = credentials.get(GITHUB_TOKEN_ACCOUNT)?.is_some();
    println!("config_path={}", config::config_path()?.display());
    println!("data_dir={}", config::data_dir()?.display());
    println!("slack_keyword_set={}", !config.slack_mention_keywords().is_empty());
    println!("slack_token_set={slack_token}");
    println!("github_token_set={github_token}");

    if slack_token && !config.slack_mention_keywords().is_empty() {
        let slack = slack_provider(credentials.clone());
        let mut seen_messages = HashSet::new();
        for keyword in config.slack_mention_keywords() {
            for message in slack.search_messages(&keyword)? {
                seen_messages.insert(format!(
                    "{}:{}:{}",
                    message.channel_id.as_deref().unwrap_or_default(),
                    message.user_id.as_str(),
                    message.ts.as_str()
                ));
            }
        }
        let count = seen_messages.len();
        println!("slack_search_ok=true");
        println!("slack_search_count={count}");
    }

    if github_token {
        let github = github_provider(credentials.clone());
        let login = github.current_user_login()?;
        println!("github_login={login}");
    }

    Ok(())
}

fn run_dump(format: &str) -> Result<()> {
    if format != "json" {
        return Err(anyhow!("unsupported format `{format}`; only json is implemented"));
    }
    let config = AppConfig::load_or_default()?;
    let store = store()?;
    let dump = store.dump(
        config.done_menu_limit,
        "OK",
        store.last_error_message()?,
        &config.github_username,
    )?;
    println!("{}", serde_json::to_string_pretty(&dump)?);
    Ok(())
}

fn run_sync_once() -> Result<()> {
    let config = AppConfig::load()?;
    config.validate()?;
    let store = store()?;
    let credentials = credentials();
    let slack = slack_provider(credentials.clone());
    let github = github_provider(credentials);
    let slack_outcome = slack_ingest::run(&config, store.clone(), slack, github.clone())?;
    let github_outcome = github_events::run(&config, store.clone(), github)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "new_pending_count": slack_outcome.new_pending_count,
            "completed_request_count": github_outcome.completed_request_count,
            "completed_pr_keys": github_outcome.completed_pr_keys,
        }))?
    );
    Ok(())
}

fn run_reset_state() -> Result<()> {
    let store = store()?;
    store.clear_state()?;
    println!("reset {}", config::database_path()?.display());
    Ok(())
}

fn run_clear_credentials() -> Result<()> {
    let credentials = credentials();
    credentials.delete(SLACK_TOKEN_ACCOUNT)?;
    credentials.delete(GITHUB_TOKEN_ACCOUNT)?;
    println!("cleared stored credentials");
    Ok(())
}
