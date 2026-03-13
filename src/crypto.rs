use age::secrecy::ExposeSecret;
use anyhow::{Context, Result};

/// A wrapper around an age x25519 identity for encryption/decryption.
pub struct Identity {
    inner: age::x25519::Identity,
}

impl Identity {
    /// Generate a new random age x25519 identity.
    pub fn generate() -> Self {
        Self {
            inner: age::x25519::Identity::generate(),
        }
    }

    /// Parse an identity from its secret key string (AGE-SECRET-KEY-1...).
    pub fn from_secret_string(s: &str) -> Result<Self> {
        let inner: age::x25519::Identity = s
            .parse()
            .map_err(|e| anyhow::anyhow!("Failed to parse age identity: {}", e))?;
        Ok(Self { inner })
    }

    /// Serialize the identity to its secret key string for storage.
    pub fn to_secret_string(&self) -> String {
        self.inner.to_string().expose_secret().to_string()
    }

    /// Get the public recipient for this identity.
    pub fn recipient(&self) -> age::x25519::Recipient {
        self.inner.to_public()
    }

    /// Encrypt plaintext data to this identity's public key.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let recipient = self.recipient();
        age::encrypt(&recipient, plaintext).context("Failed to encrypt data with age")
    }

    /// Decrypt ciphertext using this identity.
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        age::decrypt(&self.inner, ciphertext).context("Failed to decrypt data with age")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_and_roundtrip_identity() {
        let identity = Identity::generate();
        let secret = identity.to_secret_string();

        assert!(secret.starts_with("AGE-SECRET-KEY-1"));

        let restored = Identity::from_secret_string(&secret).unwrap();
        assert_eq!(
            identity.recipient().to_string(),
            restored.recipient().to_string()
        );
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let identity = Identity::generate();
        let plaintext = b"OPENAI_API_KEY=sk-test-12345\nDATABASE_URL=postgres://localhost/mydb\n";

        let ciphertext = identity.encrypt(plaintext).unwrap();
        assert_ne!(ciphertext, plaintext);

        let decrypted = identity.decrypt(&ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn decrypt_with_wrong_key_fails() {
        let identity1 = Identity::generate();
        let identity2 = Identity::generate();
        let plaintext = b"secret data";

        let ciphertext = identity1.encrypt(plaintext).unwrap();
        let result = identity2.decrypt(&ciphertext);
        assert!(result.is_err());
    }

    #[test]
    fn invalid_identity_string_fails() {
        let result = Identity::from_secret_string("not-a-valid-key");
        assert!(result.is_err());
    }
}
