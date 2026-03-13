use anyhow::{Context, Result};
use std::path::Path;
use tracing::{info, warn};

use super::Scope;
use crate::{claude, config, crypto, keychain, paths};

/// Uninstall envbroker from a Claude Code project.
pub fn uninstall_claude(project_root: &Path, scope: &Scope) -> Result<()> {
    let config_path = paths::repo_config_path(project_root);

    if !config_path.exists() {
        println!("envbroker is not installed in this project.");
        return Ok(());
    }

    let config = config::Config::load(&config_path)?;
    let project_id = paths::project_id(project_root);

    // 1. Restore plaintext .env and delete keychain entry.
    restore_env_and_purge_keychain(project_root, &config, &project_id)?;

    // 2. Remove Claude settings entries.
    let settings_path = claude::settings_path(project_root, scope);
    if settings_path.exists() {
        let existing = claude::load_settings(&settings_path)?;
        let cleaned = claude::remove_settings(&existing);
        claude::save_settings(&settings_path, &cleaned)?;
        info!(path = %settings_path.display(), "removed envbroker from Claude settings");
    }

    // 3. Remove hook scripts.
    let hooks_dir = project_root.join(".claude").join("hooks");
    for script in ["envbroker-pretooluse", "envbroker-posttoolusefailure"] {
        let script_path = hooks_dir.join(script);
        if script_path.exists() {
            std::fs::remove_file(&script_path)
                .with_context(|| format!("Failed to remove {}", script_path.display()))?;
            info!(path = %script_path.display(), "removed hook script");
        }
    }

    // 4. Remove repo-local config.
    let envbroker_dir = project_root.join(".envbroker");
    if envbroker_dir.exists() {
        std::fs::remove_dir_all(&envbroker_dir)
            .with_context(|| format!("Failed to remove {}", envbroker_dir.display()))?;
        info!("removed .envbroker directory");
    }

    // 5. Purge encrypted secrets.
    let ct_path = paths::ciphertext_path(project_root, &config.profile)?;
    if ct_path.exists() {
        std::fs::remove_file(&ct_path)
            .with_context(|| format!("Failed to remove {}", ct_path.display()))?;
        info!(path = %ct_path.display(), "removed encrypted secrets");
    }

    let meta_path = paths::meta_path(project_root)?;
    if meta_path.exists() {
        std::fs::remove_file(&meta_path).ok();
    }

    let project_dir = paths::project_data_dir(project_root)?;
    if project_dir.exists() {
        std::fs::remove_dir_all(&project_dir).ok();
    }

    println!();
    println!("envbroker uninstalled.");
    println!("Plaintext .env has been restored.");

    Ok(())
}

/// Restore the original .env by decrypting with the keychain identity,
/// then delete the keychain entry.
fn restore_env_and_purge_keychain(
    project_root: &Path,
    config: &config::Config,
    project_id: &str,
) -> Result<()> {
    let ct_path = paths::ciphertext_path(project_root, &config.profile)?;
    if !ct_path.exists() {
        warn!("cannot restore .env: encrypted secrets not found");
        if keychain::delete_identity(project_id, &config.profile).is_ok() {
            info!("removed keychain identity");
        }
        return Ok(());
    }

    let identity_secret = keychain::retrieve_identity(project_id, &config.profile)?;
    let identity = crypto::Identity::from_secret_string(&identity_secret)?;

    let ciphertext =
        std::fs::read(&ct_path).with_context(|| format!("Failed to read {}", ct_path.display()))?;
    let plaintext = identity.decrypt(&ciphertext)?;
    let plaintext_str =
        String::from_utf8(plaintext).context("Decrypted payload is not valid UTF-8")?;

    let env_path = project_root.join(&config.env_placeholder_path);
    std::fs::write(&env_path, &plaintext_str)
        .with_context(|| format!("Failed to write {}", env_path.display()))?;
    info!(path = %env_path.display(), "restored plaintext .env");

    if keychain::delete_identity(project_id, &config.profile).is_ok() {
        info!("removed keychain identity");
    }

    Ok(())
}
