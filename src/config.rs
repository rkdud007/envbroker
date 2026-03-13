use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Repo-local metadata stored in `.envbroker/config.json`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub version: u32,
    pub project_id: String,
    pub project_root: String,
    pub profile: String,
    pub env_placeholder_path: String,
    pub ciphertext_path: String,
    pub vars: Vec<String>,
    #[serde(default)]
    pub auth_method: AuthMethod,
    pub elevation_policy: ElevationPolicy,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMethod {
    #[default]
    Keychain,
    Passphrase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ElevationMode {
    PlaceholderDriven,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ElevationPolicy {
    pub mode: ElevationMode,
    pub placeholder_marker: String,
    pub trigger_message: String,
    pub always_ask_on_envbroker_run: bool,
}

impl Config {
    /// Create a new config with default elevation policy.
    pub fn new(
        project_id: String,
        project_root: String,
        profile: String,
        ciphertext_path: String,
        vars: Vec<String>,
        auth_method: AuthMethod,
    ) -> Self {
        Self {
            version: 1,
            project_id,
            project_root,
            profile,
            env_placeholder_path: ".env".to_string(),
            ciphertext_path,
            vars,
            auth_method,
            elevation_policy: ElevationPolicy {
                mode: ElevationMode::PlaceholderDriven,
                placeholder_marker: "ENVBROKER_REQUIRED".to_string(),
                trigger_message: "This command appears to have failed because the repository \
                    uses envbroker placeholders. Ask the user for permission and rerun it with \
                    envbroker run -- ..."
                    .to_string(),
                always_ask_on_envbroker_run: true,
            },
        }
    }

    /// Load config from a file path.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config at {}", path.display()))?;
        let config: Config = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse config at {}", path.display()))?;
        Ok(config)
    }

    /// Save config to a file path, creating parent directories as needed.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {}", parent.display()))?;
        }
        let content = serde_json::to_string_pretty(self).context("Failed to serialize config")?;
        std::fs::write(path, &content)
            .with_context(|| format!("Failed to write config to {}", path.display()))?;
        Ok(())
    }
}

/// External metadata stored outside the repo.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExternalMeta {
    pub created_at: String,
    pub install_version: String,
    pub ciphertext_checksum: String,
    pub profile: String,
    pub last_rotation: Option<String>,
    pub last_decrypt: Option<String>,
}

impl ExternalMeta {
    pub fn new(profile: &str, ciphertext_checksum: &str) -> Self {
        let now = unix_timestamp_now();
        Self {
            created_at: now,
            install_version: env!("CARGO_PKG_VERSION").to_string(),
            ciphertext_checksum: ciphertext_checksum.to_string(),
            profile: profile.to_string(),
            last_rotation: None,
            last_decrypt: None,
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read metadata at {}", path.display()))?;
        let meta: ExternalMeta = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse metadata at {}", path.display()))?;
        Ok(meta)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {}", parent.display()))?;
        }
        let content = serde_json::to_string_pretty(self).context("Failed to serialize metadata")?;
        std::fs::write(path, &content)
            .with_context(|| format!("Failed to write metadata to {}", path.display()))?;
        Ok(())
    }
}

/// Return the current time as a Unix timestamp string.
fn unix_timestamp_now() -> String {
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    duration.as_secs().to_string()
}
