use crate::{config::Config, error::Result, output::OutputFormatter};
use account_sdk::storage::{
    filestorage::FileSystemBackend, Credentials, StorageBackend, StorageValue,
};
use serde::Serialize;
use starknet::signers::SigningKey;
use std::path::PathBuf;

#[derive(Serialize)]
pub struct KeypairOutput {
    pub public_key: String,
    pub stored_at: String,
    pub message: String,
}

pub async fn execute(config: &Config, formatter: &dyn OutputFormatter) -> Result<()> {
    formatter.info("Generating new session keypair...");

    // Generate new keypair
    let signing_key = SigningKey::from_random();
    let verifying_key = signing_key.verifying_key();
    let public_key = format!("0x{:x}", verifying_key.scalar());
    let private_key = signing_key.secret_scalar();

    // Set up storage
    let storage_path = PathBuf::from(shellexpand::tilde(&config.session.storage_path).to_string());
    let mut backend = FileSystemBackend::new(storage_path.clone());

    // Store the keypair as session credentials
    let credentials = Credentials {
        private_key,
        authorization: vec![],
    };

    // Store credentials using "session_signer" key
    let credentials_json = serde_json::to_string(&credentials)
        .map_err(|e| crate::error::CliError::InvalidInput(e.to_string()))?;

    backend
        .set("session_signer", &StorageValue::String(credentials_json))
        .map_err(|e| crate::error::CliError::Storage(e.to_string()))?;

    let output = KeypairOutput {
        public_key: public_key.clone(),
        stored_at: storage_path.display().to_string(),
        message: "Keypair generated successfully. Use this public key for session registration."
            .to_string(),
    };

    if config.cli.json_output {
        formatter.success(&output);
    } else {
        formatter
            .info("Keypair generated successfully. Use this public key for session registration.");
        println!("\nPublic Key: {}\n", public_key);
    }

    Ok(())
}
