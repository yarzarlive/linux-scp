use crate::errors::AppError;
use crate::persistence::database::Database;
use rusqlite::params;
use serde::{Deserialize, Serialize};

/// Persistent known-host record stored in SQLite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnownHostEntry {
    pub host: String,
    pub port: u16,
    pub algorithm: String,
    pub fingerprint_sha256: String,
    pub public_key_openssh: String,
    pub trusted_at_ms: u64,
}

/// Strict host-key verification decision outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum HostKeyDecision {
    /// Presented host key matches the stored fingerprint in known_hosts.
    Trusted { entry: KnownHostEntry },
    /// Host has no entry in known_hosts; requires explicit user confirmation before trusting.
    Unknown {
        host: String,
        port: u16,
        algorithm: String,
        fingerprint_sha256: String,
        public_key_openssh: String,
    },
    /// Host key does NOT match the stored fingerprint; strictly blocked to prevent MITM attacks.
    Changed {
        host: String,
        port: u16,
        expected_algorithm: String,
        expected_fingerprint: String,
        presented_algorithm: String,
        presented_fingerprint: String,
    },
}

pub struct KnownHostsRepository<'a> {
    db: &'a Database,
}

impl<'a> KnownHostsRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Retrieves a stored known-host entry for (host, port).
    pub fn get_known_host(
        &self,
        host: &str,
        port: u16,
    ) -> Result<Option<KnownHostEntry>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "SELECT host, port, algorithm, fingerprint_sha256, public_key_openssh, trusted_at_ms
                 FROM known_hosts
                 WHERE host = ?1 AND port = ?2",
            )
            .map_err(|e| AppError::Database(format!("Failed to prepare known_hosts query: {}", e)))?;

        let mut rows = stmt
            .query_map(params![host.trim(), port], |row| {
                Ok(KnownHostEntry {
                    host: row.get(0)?,
                    port: row.get(1)?,
                    algorithm: row.get(2)?,
                    fingerprint_sha256: row.get(3)?,
                    public_key_openssh: row.get(4)?,
                    trusted_at_ms: row.get::<_, i64>(5)? as u64,
                })
            })
            .map_err(|e| AppError::Database(format!("Failed to query known_hosts: {}", e)))?;

        if let Some(row) = rows.next() {
            let entry =
                row.map_err(|e| AppError::Database(format!("Error reading known_host: {}", e)))?;
            Ok(Some(entry))
        } else {
            Ok(None)
        }
    }

    /// Evaluates a server's presented public key against the persistent known-hosts store.
    /// Never modifies or auto-trusts keys.
    pub fn verify_host_key(
        &self,
        host: &str,
        port: u16,
        algorithm: &str,
        fingerprint_sha256: &str,
        public_key_openssh: &str,
    ) -> Result<HostKeyDecision, AppError> {
        let normalized_host = host.trim();
        match self.get_known_host(normalized_host, port)? {
            None => Ok(HostKeyDecision::Unknown {
                host: normalized_host.to_string(),
                port,
                algorithm: algorithm.to_string(),
                fingerprint_sha256: fingerprint_sha256.to_string(),
                public_key_openssh: public_key_openssh.to_string(),
            }),
            Some(existing) => {
                if existing.fingerprint_sha256 == fingerprint_sha256 {
                    Ok(HostKeyDecision::Trusted { entry: existing })
                } else {
                    Ok(HostKeyDecision::Changed {
                        host: normalized_host.to_string(),
                        port,
                        expected_algorithm: existing.algorithm,
                        expected_fingerprint: existing.fingerprint_sha256,
                        presented_algorithm: algorithm.to_string(),
                        presented_fingerprint: fingerprint_sha256.to_string(),
                    })
                }
            }
        }
    }

    /// Explicitly trusts and stores an unknown host key after user approval.
    /// Strictly refuses to overwrite an existing mismatched key.
    pub fn trust_unknown_host_key(
        &self,
        host: &str,
        port: u16,
        algorithm: &str,
        fingerprint_sha256: &str,
        public_key_openssh: &str,
    ) -> Result<KnownHostEntry, AppError> {
        let normalized_host = host.trim();
        if normalized_host.is_empty() || fingerprint_sha256.trim().is_empty() {
            return Err(AppError::Validation(
                "Host and SHA256 fingerprint are required to trust a host key".to_string(),
            ));
        }

        if let Some(existing) = self.get_known_host(normalized_host, port)? {
            if existing.fingerprint_sha256 != fingerprint_sha256 {
                return Err(AppError::Security(format!(
                    "Host key mismatch for {}:{}. Automatic replacement of a changed host key is strictly forbidden.",
                    normalized_host, port
                )));
            }
            return Ok(existing);
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let entry = KnownHostEntry {
            host: normalized_host.to_string(),
            port,
            algorithm: algorithm.to_string(),
            fingerprint_sha256: fingerprint_sha256.to_string(),
            public_key_openssh: public_key_openssh.to_string(),
            trusted_at_ms: now,
        };

        self.db
            .connection()
            .execute(
                "INSERT INTO known_hosts (host, port, algorithm, fingerprint_sha256, public_key_openssh, trusted_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    entry.host,
                    entry.port,
                    entry.algorithm,
                    entry.fingerprint_sha256,
                    entry.public_key_openssh,
                    entry.trusted_at_ms as i64,
                ],
            )
            .map_err(|e| AppError::Database(format!("Failed to store trusted host key: {}", e)))?;

        Ok(entry)
    }

    /// Lists all trusted known hosts.
    pub fn list_known_hosts(&self) -> Result<Vec<KnownHostEntry>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "SELECT host, port, algorithm, fingerprint_sha256, public_key_openssh, trusted_at_ms
                 FROM known_hosts
                 ORDER BY host ASC, port ASC",
            )
            .map_err(|e| AppError::Database(format!("Failed to prepare list_known_hosts: {}", e)))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(KnownHostEntry {
                    host: row.get(0)?,
                    port: row.get(1)?,
                    algorithm: row.get(2)?,
                    fingerprint_sha256: row.get(3)?,
                    public_key_openssh: row.get(4)?,
                    trusted_at_ms: row.get::<_, i64>(5)? as u64,
                })
            })
            .map_err(|e| AppError::Database(format!("Failed to query known_hosts: {}", e)))?;

        let mut list = Vec::new();
        for item in rows {
            list.push(
                item.map_err(|e| AppError::Database(format!("Error reading known_host: {}", e)))?,
            );
        }
        Ok(list)
    }

    /// Removes a known-host entry for (host, port).
    pub fn delete_known_host(&self, host: &str, port: u16) -> Result<(), AppError> {
        self.db
            .connection()
            .execute(
                "DELETE FROM known_hosts WHERE host = ?1 AND port = ?2",
                params![host.trim(), port],
            )
            .map_err(|e| AppError::Database(format!("Failed to delete known_host: {}", e)))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unknown_host_key_decision() {
        let db = Database::open_in_memory().unwrap();
        let repo = KnownHostsRepository::new(&db);

        let decision = repo
            .verify_host_key(
                "192.168.122.10",
                22,
                "ssh-ed25519",
                "SHA256:abc123fingerprint",
                "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI...",
            )
            .unwrap();

        assert_eq!(
            decision,
            HostKeyDecision::Unknown {
                host: "192.168.122.10".to_string(),
                port: 22,
                algorithm: "ssh-ed25519".to_string(),
                fingerprint_sha256: "SHA256:abc123fingerprint".to_string(),
                public_key_openssh: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI...".to_string(),
            }
        );
    }

    #[test]
    fn test_explicit_trust_and_subsequent_trusted_decision() {
        let db = Database::open_in_memory().unwrap();
        let repo = KnownHostsRepository::new(&db);

        repo.trust_unknown_host_key(
            "192.168.122.10",
            22,
            "ssh-ed25519",
            "SHA256:abc123fingerprint",
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI...",
        )
        .expect("Should store trusted host key");

        let decision = repo
            .verify_host_key(
                "192.168.122.10",
                22,
                "ssh-ed25519",
                "SHA256:abc123fingerprint",
                "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI...",
            )
            .unwrap();

        assert!(matches!(decision, HostKeyDecision::Trusted { .. }));
    }

    #[test]
    fn test_changed_host_key_strictly_blocked_and_never_overwritten() {
        let db = Database::open_in_memory().unwrap();
        let repo = KnownHostsRepository::new(&db);

        repo.trust_unknown_host_key(
            "192.168.122.10",
            22,
            "ssh-ed25519",
            "SHA256:original_fingerprint",
            "ssh-ed25519 ORIGINAL_KEY",
        )
        .unwrap();

        // Present a different key for the same host:port
        let decision = repo
            .verify_host_key(
                "192.168.122.10",
                22,
                "ssh-rsa",
                "SHA256:attacker_mitm_fingerprint",
                "ssh-rsa ATTACKER_KEY",
            )
            .unwrap();

        assert_eq!(
            decision,
            HostKeyDecision::Changed {
                host: "192.168.122.10".to_string(),
                port: 22,
                expected_algorithm: "ssh-ed25519".to_string(),
                expected_fingerprint: "SHA256:original_fingerprint".to_string(),
                presented_algorithm: "ssh-rsa".to_string(),
                presented_fingerprint: "SHA256:attacker_mitm_fingerprint".to_string(),
            }
        );

        // Attempting to overwrite via trust_unknown_host_key must fail with Security error
        let overwrite_attempt = repo.trust_unknown_host_key(
            "192.168.122.10",
            22,
            "ssh-rsa",
            "SHA256:attacker_mitm_fingerprint",
            "ssh-rsa ATTACKER_KEY",
        );
        assert!(overwrite_attempt.is_err());
        assert!(matches!(
            overwrite_attempt.unwrap_err(),
            AppError::Security(_)
        ));

        // Verify original key remains intact in the store
        let stored = repo.get_known_host("192.168.122.10", 22).unwrap().unwrap();
        assert_eq!(stored.fingerprint_sha256, "SHA256:original_fingerprint");
    }
}
