use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::path::Path;

use crate::cli::Scope;

/// Return the Claude settings file path for the given scope relative to project root.
pub fn settings_path(project_root: &Path, scope: &Scope) -> std::path::PathBuf {
    match scope {
        Scope::Local => project_root.join(".claude").join("settings.local.json"),
        Scope::Project => project_root.join(".claude").join("settings.json"),
        Scope::User => dirs::home_dir()
            .unwrap_or_default()
            .join(".claude")
            .join("settings.json"),
    }
}

/// Merge envbroker hooks and deny rules into existing Claude settings.
/// Preserves all existing settings that are unrelated to envbroker.
pub fn merge_settings(existing: &Value, hook_script_dir: &str) -> Result<Value> {
    let mut settings = match existing {
        Value::Object(map) => map.clone(),
        _ => Map::new(),
    };

    // Merge permissions.deny rules.
    let permissions = settings
        .entry("permissions")
        .or_insert_with(|| Value::Object(Map::new()));
    let permissions_obj = permissions
        .as_object_mut()
        .context("permissions must be an object")?;

    let deny = permissions_obj
        .entry("deny")
        .or_insert_with(|| Value::Array(Vec::new()));
    let deny_arr = deny
        .as_array_mut()
        .context("permissions.deny must be an array")?;

    let deny_rules = ["Read(./.env)"];
    for rule in &deny_rules {
        let rule_val = Value::String(rule.to_string());
        if !deny_arr.contains(&rule_val) {
            deny_arr.push(rule_val);
        }
    }

    // Merge hooks.
    let hooks = settings
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()));
    let hooks_obj = hooks.as_object_mut().context("hooks must be an object")?;

    // PreToolUse hook.
    let pretooluse_hook = serde_json::json!({
        "matcher": "Bash",
        "hooks": [
            {
                "type": "command",
                "command": format!("{}/envbroker-pretooluse", hook_script_dir)
            }
        ]
    });
    merge_hook_entry(hooks_obj, "PreToolUse", pretooluse_hook)?;

    // PostToolUseFailure hook.
    let posttooluse_hook = serde_json::json!({
        "matcher": "Bash",
        "hooks": [
            {
                "type": "command",
                "command": format!("{}/envbroker-posttoolusefailure", hook_script_dir)
            }
        ]
    });
    merge_hook_entry(hooks_obj, "PostToolUseFailure", posttooluse_hook)?;

    Ok(Value::Object(settings))
}

/// Add a hook entry to a hooks object, avoiding duplicates.
fn merge_hook_entry(hooks_obj: &mut Map<String, Value>, key: &str, entry: Value) -> Result<()> {
    let hook_arr = hooks_obj
        .entry(key)
        .or_insert_with(|| Value::Array(Vec::new()));
    let arr = hook_arr
        .as_array_mut()
        .context(format!("hooks.{} must be an array", key))?;

    // Check if an envbroker hook already exists.
    let already_exists = arr.iter().any(|h| {
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
    });

    if !already_exists {
        arr.push(entry);
    }

    Ok(())
}

/// Remove envbroker hooks and deny rules from Claude settings.
pub fn remove_settings(existing: &Value) -> Value {
    let mut settings = match existing {
        Value::Object(map) => map.clone(),
        _ => return existing.clone(),
    };

    // Remove deny rules.
    if let Some(permissions) = settings.get_mut("permissions")
        && let Some(deny) = permissions.get_mut("deny")
            && let Some(arr) = deny.as_array_mut() {
                arr.retain(|v| v.as_str().map(|s| s != "Read(./.env)").unwrap_or(true));
            }

    // Remove envbroker hooks.
    if let Some(hooks) = settings.get_mut("hooks")
        && let Some(hooks_obj) = hooks.as_object_mut() {
            for (_key, hook_arr) in hooks_obj.iter_mut() {
                if let Some(arr) = hook_arr.as_array_mut() {
                    arr.retain(|h| {
                        !h.get("hooks")
                            .and_then(|v| v.as_array())
                            .map(|hooks| {
                                hooks.iter().any(|hook| {
                                    hook.get("command")
                                        .and_then(|c| c.as_str())
                                        .is_some_and(|c| c.contains("envbroker"))
                                })
                            })
                            .unwrap_or(false)
                    });
                }
            }
        }

    Value::Object(settings)
}

/// Generate the PreToolUse hook shell script content.
pub fn pretooluse_hook_script() -> String {
    r#"#!/usr/bin/env bash
exec envbroker hook pretooluse
"#
    .to_string()
}

/// Generate the PostToolUseFailure hook shell script content.
pub fn posttoolusefailure_hook_script() -> String {
    r#"#!/usr/bin/env bash
exec envbroker hook posttoolusefailure
"#
    .to_string()
}

/// Write hook scripts to disk and make them executable.
pub fn write_hook_scripts(project_root: &Path) -> Result<()> {
    let hooks_dir = project_root.join(".claude").join("hooks");
    std::fs::create_dir_all(&hooks_dir).with_context(|| {
        format!(
            "Failed to create hooks directory at {}",
            hooks_dir.display()
        )
    })?;

    let pretooluse_path = hooks_dir.join("envbroker-pretooluse");
    std::fs::write(&pretooluse_path, pretooluse_hook_script()).with_context(|| {
        format!(
            "Failed to write hook script at {}",
            pretooluse_path.display()
        )
    })?;

    let posttooluse_path = hooks_dir.join("envbroker-posttoolusefailure");
    std::fs::write(&posttooluse_path, posttoolusefailure_hook_script()).with_context(|| {
        format!(
            "Failed to write hook script at {}",
            posttooluse_path.display()
        )
    })?;

    // Make executable on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&pretooluse_path, perms.clone())?;
        std::fs::set_permissions(&posttooluse_path, perms)?;
    }

    Ok(())
}

/// Load existing Claude settings from a file, returning empty object if not found.
pub fn load_settings(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(Value::Object(Map::new()));
    }
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read Claude settings at {}", path.display()))?;
    let value: Value = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse Claude settings at {}", path.display()))?;
    Ok(value)
}

/// Save Claude settings to a file, creating parent directories as needed.
pub fn save_settings(path: &Path, settings: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }
    let content =
        serde_json::to_string_pretty(settings).context("Failed to serialize Claude settings")?;
    std::fs::write(path, &content)
        .with_context(|| format!("Failed to write Claude settings to {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_into_empty_settings() {
        let empty = Value::Object(Map::new());
        let result = merge_settings(&empty, ".claude/hooks").unwrap();

        let deny = result["permissions"]["deny"].as_array().unwrap();
        assert_eq!(deny.len(), 1);
        assert!(deny.contains(&Value::String("Read(./.env)".to_string())));

        let pretooluse = result["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pretooluse.len(), 1);

        let posttooluse = result["hooks"]["PostToolUseFailure"].as_array().unwrap();
        assert_eq!(posttooluse.len(), 1);
    }

    #[test]
    fn merge_preserves_existing_settings() {
        let existing = serde_json::json!({
            "permissions": {
                "deny": ["Read(some/other/file)"],
                "allow": ["Write(foo)"]
            },
            "someOtherKey": true
        });
        let result = merge_settings(&existing, ".claude/hooks").unwrap();

        // Existing deny rule preserved.
        let deny = result["permissions"]["deny"].as_array().unwrap();
        assert!(deny.contains(&Value::String("Read(some/other/file)".to_string())));
        // Envbroker rules added.
        assert!(deny.contains(&Value::String("Read(./.env)".to_string())));
        // Other settings preserved.
        assert_eq!(result["someOtherKey"], Value::Bool(true));
        assert_eq!(result["permissions"]["allow"][0], "Write(foo)");
    }

    #[test]
    fn merge_is_idempotent() {
        let empty = Value::Object(Map::new());
        let first = merge_settings(&empty, ".claude/hooks").unwrap();
        let second = merge_settings(&first, ".claude/hooks").unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn remove_settings_cleans_envbroker() {
        let empty = Value::Object(Map::new());
        let with_envbroker = merge_settings(&empty, ".claude/hooks").unwrap();
        let cleaned = remove_settings(&with_envbroker);

        let deny = cleaned["permissions"]["deny"].as_array().unwrap();
        assert!(deny.is_empty());

        let pretooluse = cleaned["hooks"]["PreToolUse"].as_array().unwrap();
        assert!(pretooluse.is_empty());
    }

    #[test]
    fn remove_preserves_non_envbroker_settings() {
        let existing = serde_json::json!({
            "permissions": {
                "deny": ["Read(some/other/file)", "Read(./.env)"]
            },
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Bash",
                        "hooks": [{"type": "command", "command": "some-other-hook"}]
                    },
                    {
                        "matcher": "Bash",
                        "hooks": [{"type": "command", "command": ".claude/hooks/envbroker-pretooluse"}]
                    }
                ]
            },
            "other": "preserved"
        });
        let cleaned = remove_settings(&existing);

        let deny = cleaned["permissions"]["deny"].as_array().unwrap();
        assert_eq!(deny.len(), 1);
        assert!(deny.contains(&Value::String("Read(some/other/file)".to_string())));

        let pretooluse = cleaned["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pretooluse.len(), 1);

        assert_eq!(cleaned["other"], "preserved");
    }
}
