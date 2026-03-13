use anyhow::Result;
use std::path::Path;

use super::Scope;
use crate::{claude, config, keychain, paths};

/// Display envbroker status for the current project.
pub fn status(project_root: &Path) -> Result<()> {
    let config_path = paths::repo_config_path(project_root);

    if !config_path.exists() {
        println!("envbroker: not installed in this project");
        return Ok(());
    }

    let config = config::Config::load(&config_path)?;
    let project_id = paths::project_id(project_root);

    // Check each component.
    let ct_path = paths::ciphertext_path(project_root, &config.profile)?;
    let ct_exists = ct_path.exists();
    let keychain_exists = keychain::identity_exists(&project_id, &config.profile);
    let placeholder_exists = project_root.join(&config.env_placeholder_path).exists();

    let settings_path = claude::settings_path(project_root, &Scope::Local);
    let hook_installed = if settings_path.exists() {
        let settings = claude::load_settings(&settings_path)?;
        settings
            .get("hooks")
            .and_then(|h| h.get("PreToolUse"))
            .and_then(|arr| arr.as_array())
            .map(|arr| {
                arr.iter().any(|h| {
                    h.get("hooks")
                        .and_then(|v| v.as_array())
                        .map(|hooks| {
                            hooks.iter().any(|hook| {
                                hook.get("command")
                                    .and_then(|c| c.as_str())
                                    .is_some_and(|c| c.contains("envbroker"))
                            })
                        })
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    } else {
        false
    };

    // Load external metadata for last decrypt time.
    let meta_path = paths::meta_path(project_root)?;
    let last_decrypt = if meta_path.exists() {
        config::ExternalMeta::load(&meta_path)
            .ok()
            .and_then(|m| m.last_decrypt)
    } else {
        None
    };

    // Print status.
    let project_name = project_root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    println!("envbroker status");
    println!();
    println!("Project:            {}", project_name);
    println!("Managed:            yes");
    println!("Profile:            {}", config.profile);
    println!(
        "Placeholder .env:   {}",
        if placeholder_exists {
            "present"
        } else {
            "missing"
        }
    );
    println!(
        "Encrypted blob:     {}",
        if ct_exists { "present" } else { "missing" }
    );
    println!(
        "Keychain identity:  {}",
        if keychain_exists {
            "present"
        } else {
            "missing"
        }
    );
    println!(
        "Claude hook:        {}",
        if hook_installed {
            "installed (local scope)"
        } else {
            "not installed"
        }
    );
    println!("Protected vars:     {}", config.vars.len());
    println!(
        "Last decrypt:       {}",
        last_decrypt.as_deref().unwrap_or("never")
    );

    Ok(())
}
