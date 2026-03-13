use anyhow::Result;
use std::path::Path;
use tracing::warn;

use crate::{config, paths};

/// List protected variable names for the current project.
pub fn list_vars(project_root: &Path, profile: &str) -> Result<()> {
    let config_path = paths::repo_config_path(project_root);
    let config = config::Config::load(&config_path)?;

    if profile != config.profile {
        warn!(%profile, expected = %config.profile, "requested profile not found, showing default");
    }

    for var in &config.vars {
        println!("{}", var);
    }

    Ok(())
}
