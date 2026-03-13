use anyhow::{Context, Result, bail};
use std::path::Path;
use tracing::info;

use crate::{config, crypto, dotenv, keychain, paths};

/// Execute a command with decrypted secrets injected into its environment.
/// The macOS Keychain prompt serves as the user approval gate.
pub fn run(
    project_root: &Path,
    profile: &str,
    command: &[String],
    _skip_confirm: bool,
) -> Result<i32> {
    if command.is_empty() {
        bail!("No command specified after --. Usage: envbroker run -- <command> [args...]");
    }

    // 1. Load project config for preflight display.
    let config_path = paths::repo_config_path(project_root);
    let config = config::Config::load(&config_path)?;

    // 2. Resolve ciphertext path.
    let ct_path = paths::ciphertext_path(project_root, profile)?;
    if !ct_path.exists() {
        bail!(
            "Encrypted secrets not found at {}.\n\
             Run `envbroker install claude` first.",
            ct_path.display()
        );
    }

    // 3. Show preflight summary.
    let project_name = project_root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let cmd_display = command.join(" ");

    eprintln!();
    eprintln!("envbroker: inject secrets into `{}`?", cmd_display);
    eprintln!();
    eprintln!("  Project:  {}", project_name);
    eprintln!("  Profile:  {}", profile);
    eprintln!(
        "  Secrets:  {} ({} vars)",
        config.vars.join(", "),
        config.vars.len()
    );
    eprintln!();

    // 4. Retrieve identity from keychain (triggers OS password prompt on macOS).
    let project_id = paths::project_id(project_root);
    let identity_secret = keychain::retrieve_identity(&project_id, profile)?;
    let identity = crypto::Identity::from_secret_string(&identity_secret)?;

    // 5. Decrypt.
    let ciphertext = std::fs::read(&ct_path)
        .with_context(|| format!("Failed to read ciphertext from {}", ct_path.display()))?;
    let plaintext = identity.decrypt(&ciphertext)?;

    // 6. Parse env entries.
    let plaintext_str =
        String::from_utf8(plaintext).context("Decrypted payload is not valid UTF-8")?;
    let entries = dotenv::parse_dotenv(&plaintext_str)?;
    info!(count = entries.len(), "decrypted secret variables");

    // 7. Spawn child process with merged environment.
    let exit_code = spawn_with_env(command, &entries)?;

    Ok(exit_code)
}

/// Spawn a child process with secret env vars merged into the environment.
fn spawn_with_env(command: &[String], entries: &[dotenv::EnvEntry]) -> Result<i32> {
    let program = &command[0];
    let args = &command[1..];

    let mut cmd = std::process::Command::new(program);
    cmd.args(args);

    // Inherit current environment, then overlay secrets.
    for entry in entries {
        cmd.env(&entry.key, &entry.value);
    }

    // Preserve stdin/stdout/stderr passthrough.
    cmd.stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());

    let status = cmd
        .status()
        .with_context(|| format!("Failed to execute `{}`", program))?;

    Ok(status.code().unwrap_or(1))
}
