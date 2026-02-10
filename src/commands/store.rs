use crate::{
    config::Config,
    error::{CliError, Result},
    output::OutputFormatter,
};
use account_sdk::{
    abigen::controller::Signer as AbiSigner,
    account::session::hash::Session,
    storage::{
        filestorage::FileSystemBackend, Credentials, SessionMetadata, StorageBackend, StorageValue,
    },
};
use base64::{engine::general_purpose, Engine};
use cainome_cairo_serde::NonZero;
use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionRegistration {
    username: String,
    address: String,
    owner_guid: String,
    expires_at: String,
    #[serde(default)]
    guardian_key_guid: String,
    #[serde(default)]
    metadata_hash: String,
    #[serde(default)]
    #[allow(dead_code)] // Part of deserialization schema but not used in code
    session_key_guid: String,
    #[serde(default)]
    #[allow(dead_code)] // Part of deserialization schema but not used in code
    transaction_hash: Option<String>,
}

#[derive(Serialize)]
pub struct StoreOutput {
    pub message: String,
    pub username: String,
    pub address: String,
    pub expires_at: u64,
    pub expires_at_formatted: String,
}

pub async fn execute(
    config: &Config,
    formatter: &dyn OutputFormatter,
    session_data: Option<String>,
    from_file: Option<String>,
) -> Result<()> {
    // Get session data either from argument or file
    let data = if let Some(file_path) = from_file {
        std::fs::read_to_string(&file_path)
            .map_err(|e| CliError::InvalidInput(format!("Failed to read file: {}", e)))?
            .trim()
            .to_string()
    } else if let Some(data) = session_data {
        data
    } else {
        return Err(CliError::InvalidInput(
            "Either session_data argument or --from-file must be provided".to_string(),
        ));
    };

    // Decode base64 session data
    let decoded = general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|e| CliError::InvalidSessionData(format!("Failed to decode base64: {}", e)))?;

    let session_str = String::from_utf8(decoded).map_err(|e| {
        CliError::InvalidSessionData(format!("Invalid UTF-8 in session data: {}", e))
    })?;

    // Parse session registration
    let registration: SessionRegistration = serde_json::from_str(&session_str)
        .map_err(|e| CliError::InvalidSessionData(format!("Failed to parse JSON: {}", e)))?;

    // Load the stored keypair to get session key GUID
    let storage_path = PathBuf::from(shellexpand::tilde(&config.session.storage_path).to_string());
    let mut backend = FileSystemBackend::new(storage_path);

    let credentials = match backend.get("session_signer") {
        Ok(Some(StorageValue::String(data))) => {
            let creds: Credentials = serde_json::from_str(&data)
                .map_err(|e| CliError::InvalidSessionData(e.to_string()))?;
            creds
        }
        _ => {
            return Err(CliError::NoSession);
        }
    };

    // Parse Felt values
    let address = Felt::from_hex(&registration.address)
        .map_err(|e| CliError::InvalidSessionData(format!("Invalid address: {}", e)))?;

    let owner_guid = Felt::from_hex(&registration.owner_guid)
        .map_err(|e| CliError::InvalidSessionData(format!("Invalid owner GUID: {}", e)))?;

    let expires_at = registration
        .expires_at
        .parse::<u64>()
        .map_err(|e| CliError::InvalidSessionData(format!("Invalid expiration: {}", e)))?;

    // Use provided guids or defaults
    let guardian_key_guid = if registration.guardian_key_guid.is_empty() {
        Felt::ZERO
    } else {
        Felt::from_hex(&registration.guardian_key_guid).map_err(|e| {
            CliError::InvalidSessionData(format!("Invalid guardian key GUID: {}", e))
        })?
    };

    let _metadata_hash = if registration.metadata_hash.is_empty() {
        Felt::ZERO
    } else {
        Felt::from_hex(&registration.metadata_hash)
            .map_err(|e| CliError::InvalidSessionData(format!("Invalid metadata hash: {}", e)))?
    };

    // Get session key GUID from our stored keypair
    let signing_key = starknet::signers::SigningKey::from_secret_scalar(credentials.private_key);
    let verifying_key = signing_key.verifying_key();
    let session_key_guid = verifying_key.scalar();

    // Create session signer
    let session_signer = AbiSigner::Starknet(account_sdk::abigen::controller::StarknetSigner {
        pubkey: NonZero::new(session_key_guid).ok_or_else(|| {
            CliError::InvalidSessionData("Session key GUID cannot be zero".to_string())
        })?,
    });

    // Create Session with empty policies (wildcard session)
    let session = Session::new_wildcard(expires_at, &session_signer, guardian_key_guid)
        .map_err(|e| CliError::InvalidSessionData(format!("Failed to create session: {}", e)))?;

    // Create SessionMetadata
    let session_metadata = SessionMetadata {
        session,
        max_fee: None,
        credentials: Some(credentials),
        is_registered: true,
    };

    // Store the session
    backend
        .set_session("session", session_metadata)
        .map_err(|e| CliError::Storage(e.to_string()))?;

    // Also store controller metadata for the status command
    let chain_id = Felt::from_hex(&config.session.default_chain_id).unwrap_or(Felt::ZERO);
    let controller_metadata = account_sdk::storage::ControllerMetadata {
        username: registration.username.clone(),
        address,
        class_hash: Felt::ZERO, // Will be filled in later if needed
        chain_id,
        rpc_url: config.session.default_rpc_url.clone(),
        salt: Felt::ZERO,
        owner: account_sdk::storage::Owner::Account(owner_guid),
    };

    backend
        .set_controller(&chain_id, address, controller_metadata)
        .map_err(|e| CliError::Storage(e.to_string()))?;

    let expires_at_dt =
        chrono::DateTime::from_timestamp(expires_at as i64, 0).unwrap_or_else(chrono::Utc::now);

    let output = StoreOutput {
        message: "Session stored successfully".to_string(),
        username: registration.username,
        address: format!("0x{:x}", address),
        expires_at,
        expires_at_formatted: expires_at_dt.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
    };

    formatter.success(&output);

    Ok(())
}
