pub mod doctor;
pub mod hooks;
pub mod install;
pub mod list_vars;
pub mod run;
pub mod status;
pub mod uninstall;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "envbroker",
    about = "Local secrets broker for coding-agent workflows"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Install envbroker integration for an agent.
    Install {
        #[command(subcommand)]
        agent: InstallAgent,
    },
    /// Uninstall envbroker integration for an agent.
    Uninstall {
        #[command(subcommand)]
        agent: UninstallAgent,
    },
    /// Run a command with decrypted secrets injected.
    Run {
        /// Secret profile to use.
        #[arg(long, default_value = "default")]
        profile: String,

        /// Skip interactive TTY confirmation (for CI/scripted use).
        /// Requires ENVBROKER_PASSPHRASE environment variable to be set.
        #[arg(long)]
        yes: bool,

        /// Command and arguments to execute (after --).
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },
    /// Show envbroker status for the current project.
    Status,
    /// Diagnose common installation or runtime issues.
    Doctor,
    /// Import secrets from a .env file into an existing installation.
    ImportEnv {
        /// Path to the .env file to import.
        #[arg(long, default_value = ".env")]
        env_file: PathBuf,

        /// Secret profile to use.
        #[arg(long, default_value = "default")]
        profile: String,
    },
    /// List protected variable names for the current project.
    ListVars {
        /// Secret profile to use.
        #[arg(long, default_value = "default")]
        profile: String,
    },
    /// Internal hook subcommands (not for direct use).
    #[command(hide = true)]
    Hook {
        #[command(subcommand)]
        hook_type: HookType,
    },
}

#[derive(Subcommand)]
pub enum InstallAgent {
    /// Install Claude Code integration.
    Claude {
        /// Settings scope.
        #[arg(long, default_value = "local")]
        scope: Scope,

        /// Path to the .env file.
        #[arg(long, default_value = ".env")]
        env_file: PathBuf,

        /// Secret profile name.
        #[arg(long, default_value = "default")]
        profile: String,
    },
}

#[derive(Subcommand)]
pub enum UninstallAgent {
    /// Uninstall Claude Code integration.
    Claude {
        /// Settings scope.
        #[arg(long, default_value = "local")]
        scope: Scope,
    },
}

#[derive(Clone, Debug, clap::ValueEnum)]
pub enum Scope {
    Local,
    Project,
    User,
}

#[derive(Subcommand)]
pub enum HookType {
    /// Handle PreToolUse hook events.
    Pretooluse,
    /// Handle PostToolUseFailure hook events.
    Posttoolusefailure,
}
