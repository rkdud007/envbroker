use anyhow::Result;
use std::path::Path;
use tracing::warn;

use crate::{config, paths};

/// List protected variable names for the current project.
pub fn list_vars(project_root: &Path, profile: &str) -> Result<()> {
    let config_path = paths::repo_config_path(project_root);
    let config = config::Config::load(&config_path)?;

    let vars = if profile == config.profile {
        config.vars
    } else {
        warn!(%profile, "profile not found, showing default");
        config.vars
    };

    for var in &vars {
        println!("{}", var);
    }

    Ok(())
}
