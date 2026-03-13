use anyhow::{Context, Result};

const SERVICE_NAME: &str = "envbroker";

/// Store an age identity string in the OS keychain.
pub fn store_identity(project_id: &str, profile: &str, identity_secret: &str) -> Result<()> {
    let account = format!("{}:{}", project_id, profile);
    let entry =
        keyring::Entry::new(SERVICE_NAME, &account).context("Failed to create keychain entry")?;
    entry
        .set_password(identity_secret)
        .context("Failed to store identity in keychain")?;
    Ok(())
}

/// Retrieve an age identity string from the OS keychain.
pub fn retrieve_identity(project_id: &str, profile: &str) -> Result<String> {
    let account = format!("{}:{}", project_id, profile);
    let entry =
        keyring::Entry::new(SERVICE_NAME, &account).context("Failed to create keychain entry")?;
    let password = entry.get_password().with_context(|| {
        format!(
            "Failed to retrieve identity from keychain for service '{}', account '{}'",
            SERVICE_NAME, account
        )
    })?;
    Ok(password)
}

/// Delete an age identity from the OS keychain.
pub fn delete_identity(project_id: &str, profile: &str) -> Result<()> {
    let account = format!("{}:{}", project_id, profile);
    let entry =
        keyring::Entry::new(SERVICE_NAME, &account).context("Failed to create keychain entry")?;
    entry
        .delete_credential()
        .context("Failed to delete identity from keychain")?;
    Ok(())
}

/// Check whether an identity exists in the OS keychain.
pub fn identity_exists(project_id: &str, profile: &str) -> bool {
    let account = format!("{}:{}", project_id, profile);
    let Ok(entry) = keyring::Entry::new(SERVICE_NAME, &account) else {
        return false;
    };
    entry.get_password().is_ok()
}
