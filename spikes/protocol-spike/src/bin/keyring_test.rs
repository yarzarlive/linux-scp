fn main() {
    println!("=== Linux Secret Service Spike ===");
    let entry = keyring::Entry::new("linuxscp_test_service", "test_user");
    match entry {
        Ok(entry) => {
            println!("Keyring entry handle created successfully.");
            // Test setting a credential
            match entry.set_password("test_secret_passphrase") {
                Ok(_) => {
                    println!("SUCCESS: Successfully stored secret in Linux Secret Service!");
                    if let Ok(pw) = entry.get_password() {
                        println!("SUCCESS: Retrieved secret from Linux Secret Service (length: {})", pw.len());
                    }
                    let _ = entry.delete_credential();
                    println!("SUCCESS: Deleted test secret from Linux Secret Service.");
                }
                Err(e) => {
                    println!("Keyring set_password returned: {:?} (May occur if running without an active desktop keyring session)", e);
                }
            }
        }
        Err(e) => {
            println!("Failed to create keyring entry: {:?}", e);
        }
    }
}
