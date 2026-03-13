use anyhow::{Context, Result};
use serde::Deserialize;
use std::io::Read;

use crate::config;

/// Input JSON from Claude Code hooks.
#[derive(Debug, Deserialize)]
pub struct HookInput {
    pub tool_name: Option<String>,
    pub tool_input: Option<ToolInput>,
    pub cwd: Option<String>,
    #[allow(dead_code)]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ToolInput {
    pub command: Option<String>,
}

/// Read hook input JSON from stdin.
fn read_input() -> Result<HookInput> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .context("Failed to read hook input from stdin")?;
    let parsed: HookInput =
        serde_json::from_str(&input).context("Failed to parse hook input JSON")?;
    Ok(parsed)
}

/// Write JSON to stdout.
fn write_json(value: &serde_json::Value) -> Result<()> {
    let json = serde_json::to_string(value).context("Failed to serialize hook output")?;
    println!("{}", json);
    Ok(())
}

/// Handle PreToolUse hook events.
///
/// Uses Claude Code's `hookSpecificOutput` format:
/// - `permissionDecision`: "allow" | "deny" | "ask"
/// - `permissionDecisionReason`: shown to user (allow/ask) or Claude (deny)
/// - `updatedInput`: modifies tool input before execution
/// - `additionalContext`: added to Claude's context
pub fn handle_pretooluse() -> Result<()> {
    let input = read_input()?;

    let tool_name = input.tool_name.as_deref().unwrap_or("");
    if tool_name != "Bash" {
        return Ok(());
    }

    let command = input
        .tool_input
        .as_ref()
        .and_then(|ti| ti.command.as_deref())
        .unwrap_or("");

    let cwd = input.cwd.as_deref().unwrap_or(".");
    if !is_envbroker_managed(cwd) {
        return Ok(());
    }

    // Deny direct reads of .env file.
    if is_secret_read_attempt(command) {
        write_json(&serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": "This file is managed by envbroker. The .env file contains placeholder values, not real secrets. Use `envbroker list-vars` to see protected variable names."
            }
        }))?;
        return Ok(());
    }

    // Ask for user approval on envbroker run commands — they require interactive
    // TTY passphrase input that only the user can provide.
    if is_envbroker_run(command) {
        write_json(&serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "ask",
                "permissionDecisionReason": "This command will decrypt and inject secrets. You will be prompted for your envbroker passphrase in the terminal."
            }
        }))?;
        return Ok(());
    }

    // Allow everything else.
    Ok(())
}

/// Handle PostToolUseFailure hook events.
///
/// The `error` field from Claude Code only contains the exit status string,
/// not actual stderr output. So for envbroker-managed projects, we always
/// provide guidance since we can't inspect the actual error.
pub fn handle_posttoolusefailure() -> Result<()> {
    let input = read_input()?;

    let tool_name = input.tool_name.as_deref().unwrap_or("");
    if tool_name != "Bash" {
        return Ok(());
    }

    let cwd = input.cwd.as_deref().unwrap_or(".");
    if !is_envbroker_managed(cwd) {
        return Ok(());
    }

    let config = load_config_from_cwd(cwd);
    let command = input
        .tool_input
        .as_ref()
        .and_then(|ti| ti.command.as_deref())
        .unwrap_or("<unknown>");

    let vars = config
        .as_ref()
        .map(|c| c.vars.join(", "))
        .unwrap_or_else(|_| "unknown".to_string());

    let exact_command = format!("envbroker run -- {}", command);
    let context = format!(
        "STOP. This repository is managed by envbroker. The .env file contains \
         ENVBROKER_REQUIRED placeholder values instead of real secrets. \
         This failure is likely caused by missing secrets, NOT a code bug.\n\n\
         DO NOT:\n\
         - Do not find or set real environment variable values yourself\n\
         - Do not modify the .env file\n\
         - Do not inline environment variables in the command\n\
         - Do not debug this as a normal application error\n\n\
         INSTEAD: Ask the user for permission to rerun with secrets injected:\n\n\
         {}\n\n\
         The user will be prompted for their envbroker passphrase in the terminal.\n\n\
         Protected variables: {}",
        exact_command, vars
    );

    write_json(&serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PostToolUseFailure",
            "additionalContext": context
        }
    }))?;

    Ok(())
}

/// Check if a command is an `envbroker run` invocation.
fn is_envbroker_run(command: &str) -> bool {
    let trimmed = command.trim();
    trimmed == "envbroker run"
        || trimmed.starts_with("envbroker run ")
        || trimmed.starts_with("envbroker run --")
        || trimmed.contains("&& envbroker run")
        || trimmed.contains("; envbroker run")
        || trimmed.contains("| envbroker run")
}

/// Check if a command attempts to directly read the .env file.
fn is_secret_read_attempt(command: &str) -> bool {
    let readers = ["cat", "head", "tail", "less", "more", "bat"];
    let targets = [".env", "./.env"];

    let trimmed = command.trim();
    readers.iter().any(|reader| {
        targets.iter().any(|target| {
            let pattern = format!("{} {}", reader, target);
            trimmed == pattern
                || trimmed.starts_with(&format!("{} ", pattern))
                || trimmed.contains(&format!("&& {}", pattern))
                || trimmed.contains(&format!("; {}", pattern))
                || trimmed.contains(&format!("| {}", pattern))
        })
    })
}

/// Walk up from `cwd` looking for `.envbroker/config.json`, returning its path if found.
fn find_config_path(cwd: &str) -> Option<std::path::PathBuf> {
    let mut current = std::path::PathBuf::from(cwd);
    loop {
        let config_path = current.join(".envbroker").join("config.json");
        if config_path.exists() {
            return Some(config_path);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Check if a directory is inside an envbroker-managed project.
fn is_envbroker_managed(cwd: &str) -> bool {
    find_config_path(cwd).is_some()
}

/// Try to load envbroker config from a working directory.
fn load_config_from_cwd(cwd: &str) -> Result<config::Config> {
    let config_path =
        find_config_path(cwd).context("No envbroker config found in directory hierarchy")?;
    config::Config::load(&config_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_secret_read_attempts() {
        assert!(is_secret_read_attempt("cat .env"));
        assert!(is_secret_read_attempt("cat ./.env"));
        assert!(is_secret_read_attempt("head .env"));
        assert!(is_secret_read_attempt("tail ./.env"));
        assert!(!is_secret_read_attempt("echo hello"));
        assert!(!is_secret_read_attempt("cargo run"));
        assert!(!is_secret_read_attempt("cat src/main.rs"));
        assert!(!is_secret_read_attempt("cat .envbroker/config.json"));
    }

    #[test]
    fn detect_secret_read_in_compound_commands() {
        assert!(is_secret_read_attempt("ls && cat .env"));
        assert!(is_secret_read_attempt("echo foo; cat .env"));
        assert!(is_secret_read_attempt("grep foo bar | cat .env"));
    }

    #[test]
    fn non_managed_dir_is_not_managed() {
        assert!(!is_envbroker_managed("/tmp/nonexistent-project"));
    }

    #[test]
    fn detect_envbroker_run_commands() {
        assert!(is_envbroker_run("envbroker run -- cargo run"));
        assert!(is_envbroker_run("envbroker run --profile dev -- npm start"));
        assert!(is_envbroker_run("envbroker run --yes -- python app.py"));
        assert!(!is_envbroker_run("cargo run"));
        assert!(!is_envbroker_run("echo envbroker"));
        assert!(!is_envbroker_run("envbroker status"));
        assert!(!is_envbroker_run("envbroker doctor"));
    }

    #[test]
    fn pretooluse_deny_output_format() {
        let output = serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": "test reason"
            }
        });
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("hookSpecificOutput"));
        assert!(json.contains("permissionDecision"));
        assert!(json.contains("deny"));
    }

    #[test]
    fn posttoolusefailure_output_format() {
        let output = serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "PostToolUseFailure",
                "additionalContext": "test context"
            }
        });
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("hookSpecificOutput"));
        assert!(json.contains("PostToolUseFailure"));
        assert!(json.contains("additionalContext"));
    }
}
