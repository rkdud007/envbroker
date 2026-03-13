use anyhow::{Context, Result, bail};
use std::path::Path;
use tracing::{info, warn};

use super::Scope;
use crate::{claude, config, crypto, dotenv, keychain, paths};

/// Run the full install flow for Claude Code integration.
pub fn install_claude(
    project_root: &Path,
    env_file: &Path,
    profile: &str,
    scope: &Scope,
) -> Result<()> {
    // 1. Resolve source dotenv path.
    let env_path = if env_file.is_absolute() {
        env_file.to_path_buf()
    } else {
        project_root.join(env_file)
    };

    if !env_path.exists() {
        bail!(
            "No .env file found at {}. Create one first or use --env-file to specify the path.",
            env_path.display()
        );
    }

    // 2. Parse dotenv file.
    let entries = dotenv::read_dotenv(&env_path)?;
    if entries.is_empty() {
        bail!(
            "The .env file at {} contains no variables.",
            env_path.display()
        );
    }
    let var_names = dotenv::var_names(&entries);
    info!(count = entries.len(), "parsed .env variables");

    // 3. Generate project metadata.
    let project_id = paths::project_id(project_root);
    let ct_path = paths::ciphertext_path(project_root, profile)?;

    info!(%project_id, "generated project identity");

    // 4. Generate or retrieve age identity from keychain.
    let identity = match keychain::retrieve_identity(&project_id, profile) {
        Ok(secret) => {
            warn!("existing keychain identity found, reusing it");
            crypto::Identity::from_secret_string(&secret)?
        }
        Err(_) => {
            let id = crypto::Identity::generate();
            keychain::store_identity(&project_id, profile, &id.to_secret_string())?;
            info!("generated and stored new age identity in keychain");
            id
        }
    };

    // 5. Encrypt the dotenv payload.
    let plaintext = dotenv::serialize_dotenv(&entries);
    let ciphertext = identity.encrypt(plaintext.as_bytes())?;
    info!(bytes = ciphertext.len(), "encrypted secret payload");

    // 6. Persist ciphertext outside the repo.
    if let Some(parent) = ct_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }
    std::fs::write(&ct_path, &ciphertext)
        .with_context(|| format!("Failed to write ciphertext to {}", ct_path.display()))?;
    info!(path = %ct_path.display(), "wrote encrypted secrets");

    // 7. Rewrite .env to placeholders.
    let placeholder = dotenv::generate_placeholder(&entries, "ENVBROKER_REQUIRED");
    std::fs::write(&env_path, &placeholder)
        .with_context(|| format!("Failed to write placeholder .env to {}", env_path.display()))?;
    info!(path = %env_path.display(), "rewrote .env with placeholders");

    // 8. Clean up placeholder artifacts (e.g. SQLite WAL files from prior runs).
    clean_placeholder_artifacts(project_root, "ENVBROKER_REQUIRED");

    // 9. Check git tracking.
    check_git_tracking(project_root, &env_path);

    // 10. Write repo-local config.
    let config = config::Config::new(
        project_id.clone(),
        project_root.to_string_lossy().to_string(),
        profile.to_string(),
        ct_path.to_string_lossy().to_string(),
        var_names.clone(),
        "keychain".to_string(),
    );
    let config_path = paths::repo_config_path(project_root);
    config.save(&config_path)?;
    info!(path = %config_path.display(), "wrote project config");

    // 11. Write external metadata.
    let checksum = format!("{:x}", ciphertext.len());
    let meta = config::ExternalMeta::new(profile, &checksum);
    let meta_path = paths::meta_path(project_root)?;
    meta.save(&meta_path)?;

    // 12. Write hook scripts.
    claude::write_hook_scripts(project_root)?;
    info!("installed hook scripts");

    // 13. Merge Claude settings.
    let settings_path = claude::settings_path(project_root, scope);
    let existing = claude::load_settings(&settings_path)?;
    let merged = claude::merge_settings(&existing, ".claude/hooks")?;
    claude::save_settings(&settings_path, &merged)?;
    info!(path = %settings_path.display(), "updated Claude settings");

    // 14. Print summary.
    println!();
    println!("envbroker installed successfully!");
    println!();
    println!("  Project:    {}", project_root.display());
    println!("  Profile:    {}", profile);
    println!(
        "  Variables:  {} ({})",
        var_names.len(),
        var_names.join(", ")
    );
    println!("  Encrypted:  {}", ct_path.display());
    println!("  Scope:      {:?}", scope);
    println!();
    println!("The original .env has been replaced with placeholders.");
    println!("Real secrets are encrypted outside the repository.");
    println!("Use `envbroker run -- <command>` to execute with secrets.");

    Ok(())
}

/// Remove files created by applications that treated the placeholder marker as a real path.
/// For example, SQLite creates `ENVBROKER_REQUIRED-shm` and `ENVBROKER_REQUIRED-wal`
/// when `DATABASE_URL=ENVBROKER_REQUIRED` is used as a database path.
fn clean_placeholder_artifacts(project_root: &Path, marker: &str) {
    let Ok(entries) = std::fs::read_dir(project_root) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        // Match the marker itself (as a file) and any suffixed variants like -shm, -wal.
        if (name_str == marker || name_str.starts_with(&format!("{}-", marker)))
            && let Ok(ft) = entry.file_type()
            && ft.is_file()
            && std::fs::remove_file(entry.path()).is_ok()
        {
            info!(path = %entry.path().display(), "removed placeholder artifact");
        }
    }
}

/// Warn if .env is tracked by git.
fn check_git_tracking(project_root: &Path, env_path: &Path) {
    let relative = env_path.strip_prefix(project_root).unwrap_or(env_path);

    let output = std::process::Command::new("git")
        .args(["ls-files", &relative.to_string_lossy()])
        .current_dir(project_root)
        .output();

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.trim().is_empty() {
            warn!(
                ".env is tracked by git. Consider adding it to .gitignore \
                 and cleaning git history with `git rm --cached .env`."
            );
        }
    }
}
