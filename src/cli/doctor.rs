use anyhow::Result;
use std::path::Path;

use super::Scope;
use crate::{claude, config, keychain, paths};

/// Run diagnostic checks and report results.
pub fn doctor(project_root: &Path) -> Result<()> {
    println!("envbroker doctor");
    println!();

    let mut pass = 0;
    let mut fail = 0;
    let mut warn_count = 0;

    // 1. Project config exists.
    let config_path = paths::repo_config_path(project_root);
    if config_path.exists() {
        check_pass("Project config", &config_path.to_string_lossy());
        pass += 1;
    } else {
        check_fail(
            "Project config",
            "not found",
            "Run `envbroker install claude --scope local`",
        );
        fail += 1;
        print_summary(pass, fail, warn_count);
        return Ok(());
    }

    // 2. Config is parseable.
    let config = match config::Config::load(&config_path) {
        Ok(c) => {
            check_pass("Config parseable", "ok");
            pass += 1;
            c
        }
        Err(e) => {
            check_fail(
                "Config parseable",
                &format!("{}", e),
                "Reinstall with `envbroker install claude`",
            );
            fail += 1;
            print_summary(pass, fail, warn_count);
            return Ok(());
        }
    };

    let project_id = paths::project_id(project_root);

    // 3. Ciphertext exists.
    let ct_path = paths::ciphertext_path(project_root, &config.profile)?;
    if ct_path.exists() {
        check_pass("Encrypted secrets", &ct_path.to_string_lossy());
        pass += 1;
    } else {
        check_fail(
            "Encrypted secrets",
            "not found",
            "Reinstall with `envbroker install claude`",
        );
        fail += 1;
    }

    // 4. Keychain entry exists.
    if keychain::identity_exists(&project_id, &config.profile) {
        check_pass("Keychain identity", "present");
        pass += 1;
    } else {
        check_fail(
            "Keychain identity",
            &format!(
                "no identity for service 'envbroker', account '{}:{}'",
                project_id, config.profile
            ),
            "Reinstall with `envbroker install claude`",
        );
        fail += 1;
    }

    // 5. Placeholder .env exists.
    let env_path = project_root.join(&config.env_placeholder_path);
    if env_path.exists() {
        let content = std::fs::read_to_string(&env_path).unwrap_or_default();
        if content.contains(&config.elevation_policy.placeholder_marker) {
            check_pass("Placeholder .env", "present with markers");
            pass += 1;
        } else {
            check_warn(
                "Placeholder .env",
                "file exists but does not contain placeholder markers",
            );
            warn_count += 1;
        }
    } else {
        check_fail(
            "Placeholder .env",
            "not found",
            "Reinstall with `envbroker install claude`",
        );
        fail += 1;
    }

    // 6. Claude settings contain envbroker hook.
    let settings_path = claude::settings_path(project_root, &Scope::Local);
    if settings_path.exists() {
        let settings = claude::load_settings(&settings_path)?;
        let has_hook = claude::has_envbroker_hook(&settings, "PreToolUse");

        if has_hook {
            check_pass("Claude hook", "installed in settings");
            pass += 1;
        } else {
            check_fail(
                "Claude hook",
                "not found in Claude settings",
                "Reinstall with `envbroker install claude`",
            );
            fail += 1;
        }

        // 7. Deny rules present.
        let has_deny = claude::has_envbroker_deny_rule(&settings);

        if has_deny {
            check_pass("Deny rules", ".env read denied");
            pass += 1;
        } else {
            check_fail(
                "Deny rules",
                "Read(./.env) deny rule missing",
                "Reinstall with `envbroker install claude`",
            );
            fail += 1;
        }
    } else {
        check_fail(
            "Claude settings",
            "file not found",
            "Reinstall with `envbroker install claude`",
        );
        fail += 1;
    }

    // 8. Hook scripts exist and are executable.
    let hooks_dir = project_root.join(".claude").join("hooks");
    for script in ["envbroker-pretooluse", "envbroker-posttoolusefailure"] {
        let script_path = hooks_dir.join(script);
        if script_path.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::metadata(&script_path)?.permissions();
                if perms.mode() & 0o111 != 0 {
                    check_pass(&format!("Hook script {}", script), "executable");
                    pass += 1;
                } else {
                    check_warn(
                        &format!("Hook script {}", script),
                        "exists but not executable",
                    );
                    warn_count += 1;
                }
            }
            #[cfg(not(unix))]
            {
                check_pass(&format!("Hook script {}", script), "present");
                pass += 1;
            }
        } else {
            check_fail(
                &format!("Hook script {}", script),
                "not found",
                "Reinstall with `envbroker install claude`",
            );
            fail += 1;
        }
    }

    println!();
    print_summary(pass, fail, warn_count);

    Ok(())
}

fn check_pass(name: &str, detail: &str) {
    println!("  [PASS] {}: {}", name, detail);
}

fn check_fail(name: &str, detail: &str, fix: &str) {
    println!("  [FAIL] {}: {}", name, detail);
    println!("         Fix: {}", fix);
}

fn check_warn(name: &str, detail: &str) {
    println!("  [WARN] {}: {}", name, detail);
}

fn print_summary(pass: usize, fail: usize, warn: usize) {
    println!(
        "Result: {} passed, {} failed, {} warnings",
        pass, fail, warn
    );
    if fail > 0 {
        println!("Run `envbroker install claude --scope local` to fix issues.");
    }
}
