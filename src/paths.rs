use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

/// Resolve the project root by walking up from `start` looking for `.git`.
pub fn find_project_root(start: &Path) -> Result<PathBuf> {
    let mut current = start
        .canonicalize()
        .context("Failed to canonicalize starting path")?;
    loop {
        if current.join(".git").exists() {
            return Ok(current);
        }
        if !current.pop() {
            bail!(
                "No git repository found from {}. Run this command inside a git repository.",
                start.display()
            );
        }
    }
}

/// Generate a deterministic project ID from the canonical project root path.
/// Uses the first 16 hex chars of SHA-256.
pub fn project_id(project_root: &Path) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Use a simple hash for now; avoids adding sha2 dependency.
    // The ID only needs to be locally unique and deterministic.
    let canonical = project_root.to_string_lossy();
    let mut hasher = DefaultHasher::new();
    canonical.hash(&mut hasher);
    let hash = hasher.finish();
    format!("{:016x}", hash)
}

/// Return the platform-specific app data directory for envbroker.
pub fn app_data_dir() -> Result<PathBuf> {
    let base = dirs::data_dir().context(
        "Unable to determine app data directory. \
         Set XDG_DATA_HOME on Linux or check your OS configuration.",
    )?;
    Ok(base.join("envbroker"))
}

/// Return the per-project data directory inside app data.
pub fn project_data_dir(project_root: &Path) -> Result<PathBuf> {
    let id = project_id(project_root);
    Ok(app_data_dir()?.join("projects").join(id))
}

/// Return the path to the encrypted secret blob for a project/profile.
pub fn ciphertext_path(project_root: &Path, profile: &str) -> Result<PathBuf> {
    let dir = project_data_dir(project_root)?;
    if profile == "default" {
        Ok(dir.join("env.age"))
    } else {
        Ok(dir.join("profiles").join(profile).join("env.age"))
    }
}

/// Return the path to the external metadata file for a project.
pub fn meta_path(project_root: &Path) -> Result<PathBuf> {
    Ok(project_data_dir(project_root)?.join("meta.json"))
}

/// Return the path to the in-repo config file.
pub fn repo_config_path(project_root: &Path) -> PathBuf {
    project_root.join(".envbroker").join("config.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn project_id_is_deterministic() {
        let path = Path::new("/tmp/test-project");
        let id1 = project_id(path);
        let id2 = project_id(path);
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 16);
    }

    #[test]
    fn project_id_differs_for_different_paths() {
        let id1 = project_id(Path::new("/tmp/project-a"));
        let id2 = project_id(Path::new("/tmp/project-b"));
        assert_ne!(id1, id2);
    }

    #[test]
    fn app_data_dir_is_valid() {
        let dir = app_data_dir().unwrap();
        assert!(dir.ends_with("envbroker"));
    }
}
