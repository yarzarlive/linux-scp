use std::path::Path;
use std::sync::Arc;
use russh::client::*;
use russh::keys::*;
use russh_sftp::client::SftpSession;

struct ClientHandler;

impl Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let key = server_public_key.public_key();
        println!("[HOST KEY] Algorithm: {}", key.algorithm());
        println!("[HOST KEY] SHA256 Fingerprint: {}", key.fingerprint(ssh_key::HashAlg::Sha256));
        Ok(true)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== LinuxSCP External SFTP Server Exec Spike ===");

    let host = "192.168.122.10";
    let port = 22;
    let username = "ubuntu";
    let key_path = shellexpand::tilde("~/.ssh/id_rsa").to_string();

    let config = Arc::new(russh::client::Config::default());
    let mut session = russh::client::connect(config, (host, port), ClientHandler).await?;
    let key_pair = russh::keys::load_secret_key(Path::new(&key_path), None)?;
    let priv_key_with_hash = PrivateKeyWithHashAlg::new(Arc::new(key_pair), Some(HashAlg::Sha512));
    let auth_res = session.authenticate_publickey(username, priv_key_with_hash).await?;
    assert!(auth_res.success());

    println!("Testing direct execution of external SFTP server on dedicated session channel (`/usr/lib/openssh/sftp-server`)...");
    let exec_channel = session.channel_open_session().await?;
    exec_channel.exec(true, "/usr/lib/openssh/sftp-server").await?;

    println!("Initializing SFTP framing over the executed process stream...");
    let exec_sftp = SftpSession::new(exec_channel.into_stream()).await?;
    println!("SUCCESS! SFTP session established over an executed remote process channel!");

    let entries = exec_sftp.read_dir("/home/ubuntu").await?;
    println!("Listed entries via executed external binary channel:");
    for entry in entries {
        println!("  -> {}", entry.file_name());
    }

    println!("\n=== External Exec SFTP Architecture Fully Proven! ===");
    Ok(())
}
