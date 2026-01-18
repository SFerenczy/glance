//! Secure secret storage using OS keyring.
//!
//! Provides abstraction over keyring for storing passwords and API keys.
//! Falls back to plaintext storage with explicit user consent when keyring
//! is unavailable.

use crate::error::{GlanceError, Result};
use keyring::Entry;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::warn;

const SERVICE_NAME: &str = "db-glance";

/// Status of the secure storage backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretStorageStatus {
    /// OS keyring is available and working.
    Secure,
    /// Keyring unavailable; using plaintext with user consent.
    PlaintextConsented,
    /// Keyring unavailable; no consent given yet.
    PlaintextPending,
}

/// Manages secure storage of secrets (passwords, API keys).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SecretStorage {
    keyring_available: bool,
    plaintext_consented: Arc<AtomicBool>,
}

impl Default for SecretStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl SecretStorage {
    /// Creates a new secret storage instance, probing keyring availability.
    pub fn new() -> Self {
        let keyring_available = Self::probe_keyring();
        Self {
            keyring_available,
            plaintext_consented: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Probes whether the OS keyring is available and persistent.
    fn probe_keyring() -> bool {
        let test_entry = match Entry::new(SERVICE_NAME, "__probe__") {
            Ok(e) => e,
            Err(e) => {
                tracing::debug!("Keyring probe: failed to create entry: {}", e);
                return false;
            }
        };

        // Check if this is a mock credential (non-persistent)
        let entry_debug = format!("{:?}", test_entry);
        if entry_debug.contains("MockCredential") {
            tracing::debug!(
                "Keyring probe: detected mock credential backend (non-persistent), treating as unavailable"
            );
            return false;
        }

        match test_entry.set_password("test") {
            Ok(()) => {
                let _ = test_entry.delete_credential();
                tracing::debug!("Keyring probe: keyring is available");
                true
            }
            Err(e) => {
                tracing::debug!("Keyring probe: failed to set password: {}", e);
                false
            }
        }
    }

    /// Returns the current status of secret storage.
    pub fn status(&self) -> SecretStorageStatus {
        if self.keyring_available {
            SecretStorageStatus::Secure
        } else if self.plaintext_consented.load(Ordering::Relaxed) {
            SecretStorageStatus::PlaintextConsented
        } else {
            SecretStorageStatus::PlaintextPending
        }
    }

    /// Returns whether secure storage (keyring) is available.
    pub fn is_secure(&self) -> bool {
        self.keyring_available
    }

    /// Records user consent for plaintext storage.
    pub fn consent_to_plaintext(&self) {
        self.plaintext_consented.store(true, Ordering::Relaxed);
    }

    /// Stores a secret in the keyring.
    ///
    /// Returns the secret key identifier for later retrieval.
    pub fn store(&self, key: &str, secret: &str) -> Result<()> {
        if !self.keyring_available {
            return Err(GlanceError::persistence(
                "Keyring unavailable. Use store_plaintext with user consent.",
            ));
        }

        let entry = Entry::new(SERVICE_NAME, key).map_err(|e| {
            GlanceError::persistence(format!("Failed to create keyring entry: {e}"))
        })?;

        entry
            .set_password(secret)
            .map_err(|e| GlanceError::persistence(format!("Failed to store secret: {e}")))?;

        Ok(())
    }

    /// Retrieves a secret from the keyring.
    pub fn retrieve(&self, key: &str) -> Result<Option<String>> {
        if !self.keyring_available {
            tracing::debug!(
                "Keyring not available, cannot retrieve secret for key: {}",
                key
            );
            return Ok(None);
        }

        let entry = Entry::new(SERVICE_NAME, key)
            .map_err(|e| GlanceError::persistence(format!("Failed to access keyring: {e}")))?;

        match entry.get_password() {
            Ok(secret) => {
                tracing::debug!("Successfully retrieved secret for key: {}", key);
                Ok(Some(secret))
            }
            Err(keyring::Error::NoEntry) => {
                tracing::debug!("No keyring entry found for key: {}", key);
                Ok(None)
            }
            Err(e) => Err(GlanceError::persistence(format!(
                "Failed to retrieve secret: {e}"
            ))),
        }
    }

    /// Deletes a secret from the keyring.
    pub fn delete(&self, key: &str) -> Result<()> {
        if !self.keyring_available {
            return Ok(());
        }

        let entry = Entry::new(SERVICE_NAME, key)
            .map_err(|e| GlanceError::persistence(format!("Failed to access keyring: {e}")))?;

        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => {
                warn!("Failed to delete secret from keyring: {e}");
                Ok(())
            }
        }
    }

    /// Generates a keyring key for a connection password.
    pub fn connection_password_key(connection_name: &str) -> String {
        format!("conn:{}", connection_name)
    }

    /// Generates a keyring key for an LLM API key.
    pub fn llm_api_key(provider: &str) -> String {
        format!("llm:{}", provider)
    }

    /// Masks a secret for display, showing only the last 4 characters.
    pub fn mask_secret(secret: &str) -> String {
        if secret.len() <= 4 {
            "*".repeat(secret.len())
        } else {
            format!("{}...{}", "*".repeat(4), &secret[secret.len() - 4..])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_secret_short() {
        assert_eq!(SecretStorage::mask_secret("abc"), "***");
    }

    #[test]
    fn test_mask_secret_long() {
        assert_eq!(
            SecretStorage::mask_secret("sk-1234567890abcdef"),
            "****...cdef"
        );
    }

    #[test]
    fn test_connection_password_key() {
        assert_eq!(SecretStorage::connection_password_key("mydb"), "conn:mydb");
    }

    #[test]
    fn test_llm_api_key() {
        assert_eq!(SecretStorage::llm_api_key("openai"), "llm:openai");
    }
}
