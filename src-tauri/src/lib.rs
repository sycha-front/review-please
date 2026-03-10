mod app;
mod cli;
mod commands;
mod config;
mod db;
mod keychain;
mod models;
mod providers;
mod services;
mod tray;

pub fn run() -> anyhow::Result<()> {
    match cli::CliCommand::from_env(std::env::args().skip(1))? {
        Some(command) => cli::run(command),
        None => app::run_tray_app(),
    }
}
