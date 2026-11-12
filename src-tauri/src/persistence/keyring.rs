use crate::errors::AppError;

pub const KEYRING_SERVICE_NAME: &str = "org.linuxscp.client";

/// Linux Secret Service credential manager interface.
/// Strictly enforces that secret material never falls back to plaintext files.
pub struct KeyringService;

impl KeyringService {
    /// Stores a secret (password or passphrase) for a given profile ID.
    pub fn set_secret(profile_id: &str, secret: &str) -> Result<(), AppError> {
        let entry = keyring::Entry::new(KEYRING_SERVICE_NAME, profile_id)
            .map_err(|e| AppError::Security(format!("Failed to create keyring entry: {}", e)))?;

        entry.set_password(secret).map_err(|e| {
            AppError::Security(format!(
                "Linux Secret Service failure during secret storage: {}. Plaintext fallback disallowed.",
                e
            ))
        })
    }

    /// Retrieves a secret for a given profile ID.
    pub fn get_secret(profile_id: &str) -> Result<Option<String>, AppError> {
        let entry = keyring::Entry::new(KEYRING_SERVICE_NAME, profile_id)
            .map_err(|e| AppError::Security(format!("Failed to create keyring entry: {}", e)))?;

        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AppError::Security(format!(
                "Linux Secret Service failure during secret retrieval: {}. Plaintext fallback disallowed.",
                e
            ))),
        }
    }

    /// Deletes a secret from Secret Service for a given profile ID.
    pub fn delete_secret(profile_id: &str) -> Result<(), AppError> {
        let entry = keyring::Entry::new(KEYRING_SERVICE_NAME, profile_id)
            .map_err(|e| AppError::Security(format!("Failed to create keyring entry: {}", e)))?;

        match entry.delete_credential() {
            Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AppError::Security(format!(
                "Linux Secret Service failure during secret deletion: {}",
                e
            ))),
        }
    }

    /// Checks if a secret is saved for a given profile ID without exposing secret bytes.
    pub fn has_secret(profile_id: &str) -> Result<bool, AppError> {
        let entry = keyring::Entry::new(KEYRING_SERVICE_NAME, profile_id)
            .map_err(|e| AppError::Security(format!("Failed to create keyring entry: {}", e)))?;

        match entry.get_password() {
            Ok(_) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(e) => Err(AppError::Security(format!(
                "Linux Secret Service check failure: {}",
                e
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyring_service_operations() {
        let test_profile_id = format!("test-profile-{}", uuid::Uuid::new_v4());
        let secret = "super_secret_passphrase_123";

        // Store
        let store_res = KeyringService::set_secret(&test_profile_id, secret);
        if let Err(err) = store_res {
            // If running in headless environment without D-Bus secret service, verify error is security error
            assert!(matches!(err, AppError::Security(_)));
            return;
        }

        // Check has_secret
        assert!(KeyringService::has_secret(&test_profile_id).unwrap());

        // Retrieve
        let retrieved = KeyringService::get_secret(&test_profile_id).unwrap();
        assert_eq!(retrieved, Some(secret.to_string()));

        // Delete
        assert!(KeyringService::delete_secret(&test_profile_id).is_ok());

        // Check again
        assert!(!KeyringService::has_secret(&test_profile_id).unwrap());
        assert_eq!(KeyringService::get_secret(&test_profile_id).unwrap(), None);
    }
}
