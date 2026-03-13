use anyhow::{Context, Result, bail};
use std::path::Path;

/// A parsed key-value entry from a .env file.
#[derive(Debug, Clone)]
pub struct EnvEntry {
    pub key: String,
    pub value: String,
}

/// Parse a .env file into a list of key-value entries.
/// Handles comments, blank lines, and optional quoting.
pub fn parse_dotenv(content: &str) -> Result<Vec<EnvEntry>> {
    let mut entries = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Skip empty lines and comments.
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let Some(eq_pos) = trimmed.find('=') else {
            bail!(
                "Invalid .env syntax at line {}: missing '=' in '{}'",
                line_num + 1,
                trimmed
            );
        };

        let key = trimmed[..eq_pos].trim().to_string();
        if key.is_empty() {
            bail!("Invalid .env syntax at line {}: empty key", line_num + 1);
        }

        let raw_value = trimmed[eq_pos + 1..].trim();
        let value = unquote(raw_value);

        entries.push(EnvEntry { key, value });
    }

    Ok(entries)
}

/// Remove surrounding single or double quotes from a value.
fn unquote(s: &str) -> String {
    for quote in ['"', '\''] {
        if let Some(inner) = s.strip_prefix(quote).and_then(|s| s.strip_suffix(quote)) {
            return inner.to_string();
        }
    }
    s.to_string()
}

/// Read and parse a .env file from disk.
pub fn read_dotenv(path: &Path) -> Result<Vec<EnvEntry>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    parse_dotenv(&content)
}

/// Generate placeholder .env content from parsed entries.
/// All values are replaced with the sentinel marker.
pub fn generate_placeholder(entries: &[EnvEntry], marker: &str) -> String {
    let header = [
        "# Managed by envbroker. Real values are encrypted outside this repository.".to_string(),
        "# ENVBROKER_ACTIVE".to_string(),
    ];
    let lines: Vec<String> = header
        .into_iter()
        .chain(entries.iter().map(|e| format!("{}={}", e.key, marker)))
        .collect();
    lines.join("\n") + "\n"
}

/// Serialize entries back to dotenv format (for encryption).
pub fn serialize_dotenv(entries: &[EnvEntry]) -> String {
    let needs_quoting =
        |v: &str| v.contains(' ') || v.contains('#') || v.contains('\'') || v.contains('"');

    let lines: Vec<String> = entries
        .iter()
        .map(|entry| {
            if needs_quoting(&entry.value) {
                format!("{}=\"{}\"", entry.key, entry.value.replace('"', "\\\""))
            } else {
                format!("{}={}", entry.key, entry.value)
            }
        })
        .collect();
    lines.join("\n") + "\n"
}

/// Extract just the variable names from entries.
pub fn var_names(entries: &[EnvEntry]) -> Vec<String> {
    entries.iter().map(|e| e.key.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_env() {
        let content = "FOO=bar\nBAZ=qux\n";
        let entries = parse_dotenv(content).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].key, "FOO");
        assert_eq!(entries[0].value, "bar");
        assert_eq!(entries[1].key, "BAZ");
        assert_eq!(entries[1].value, "qux");
    }

    #[test]
    fn parse_with_comments_and_blanks() {
        let content = "# comment\n\nFOO=bar\n\n# another\nBAZ=qux\n";
        let entries = parse_dotenv(content).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn parse_quoted_values() {
        let content = "FOO=\"hello world\"\nBAR='single'\n";
        let entries = parse_dotenv(content).unwrap();
        assert_eq!(entries[0].value, "hello world");
        assert_eq!(entries[1].value, "single");
    }

    #[test]
    fn parse_missing_equals_fails() {
        let content = "FOOBAR\n";
        assert!(parse_dotenv(content).is_err());
    }

    #[test]
    fn placeholder_generation() {
        let entries = vec![
            EnvEntry {
                key: "API_KEY".into(),
                value: "secret123".into(),
            },
            EnvEntry {
                key: "DB_URL".into(),
                value: "postgres://...".into(),
            },
        ];
        let placeholder = generate_placeholder(&entries, "ENVBROKER_REQUIRED");
        assert!(placeholder.contains("API_KEY=ENVBROKER_REQUIRED"));
        assert!(placeholder.contains("DB_URL=ENVBROKER_REQUIRED"));
        assert!(placeholder.contains("# ENVBROKER_ACTIVE"));
        assert!(!placeholder.contains("secret123"));
    }

    #[test]
    fn roundtrip_serialize() {
        let entries = vec![
            EnvEntry {
                key: "FOO".into(),
                value: "bar".into(),
            },
            EnvEntry {
                key: "BAZ".into(),
                value: "hello world".into(),
            },
        ];
        let serialized = serialize_dotenv(&entries);
        let parsed = parse_dotenv(&serialized).unwrap();
        assert_eq!(parsed[0].key, "FOO");
        assert_eq!(parsed[0].value, "bar");
        assert_eq!(parsed[1].key, "BAZ");
        assert_eq!(parsed[1].value, "hello world");
    }
}
