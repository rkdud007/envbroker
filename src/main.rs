mod claude;
mod cli;
mod config;
mod crypto;
mod dotenv;
mod keychain;
mod paths;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

use cli::{Cli, Command};

/// Resolve the project root from the current working directory.
fn project_root() -> Result<PathBuf> {
    let cwd = std::env::current_dir().context("Failed to determine current directory")?;
    paths::find_project_root(&cwd)
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Install { agent } => match agent {
            cli::InstallAgent::Claude {
                scope,
                env_file,
                profile,
            } => cli::install::install_claude(&project_root()?, &env_file, &profile, &scope),
        },
        Command::Uninstall { agent } => match agent {
            cli::UninstallAgent::Claude { scope } => {
                cli::uninstall::uninstall_claude(&project_root()?, &scope)
            }
        },
        Command::Run {
            profile,
            yes,
            command,
        } => {
            let exit_code = cli::run::run(&project_root()?, &profile, &command, yes)?;
            std::process::exit(exit_code);
        }
        Command::Status => cli::status::status(&project_root()?),
        Command::Doctor => cli::doctor::doctor(&project_root()?),
        Command::ListVars { profile } => cli::list_vars::list_vars(&project_root()?, &profile),
        Command::Hook { hook_type } => match hook_type {
            cli::HookType::Pretooluse => cli::hooks::handle_pretooluse(),
            cli::HookType::Posttoolusefailure => cli::hooks::handle_posttoolusefailure(),
        },
    }
}
