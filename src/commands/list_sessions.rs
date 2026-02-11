use crate::{
    api::{query_controller_sessions, SessionListInfo},
    config::Config,
    error::{CliError, Result},
    output::OutputFormatter,
};
use account_sdk::storage::{
    filestorage::FileSystemBackend, Credentials, StorageBackend, StorageValue,
};
use serde::Serialize;
use std::path::PathBuf;

/// A session entry in the list
#[derive(Serialize)]
pub struct SessionEntry {
    /// The session ID (GUID)
    pub session_key_guid: String,
    /// The controller address
    pub controller_address: String,
    /// The account/username
    pub account_id: String,
    /// Chain ID (e.g., "SN_MAIN", "SN_SEPOLIA")
    pub chain_id: String,
    /// Expiration timestamp (Unix seconds)
    pub expires_at: u64,
    /// Human-readable expiration time
    pub expires_at_formatted: String,
    /// Whether the session has expired
    pub is_expired: bool,
    /// Seconds until expiration (negative if expired)
    pub expires_in_seconds: i64,
    /// Whether the session has been revoked
    pub is_revoked: bool,
    /// Session creation time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

/// Output structure for the list-sessions command
#[derive(Serialize)]
pub struct ListSessionsOutput {
    pub status: String,
    pub sessions: Vec<SessionEntry>,
    pub total_count: usize,
    pub active_count: usize,
}

pub async fn execute(config: &Config, formatter: &dyn OutputFormatter) -> Result<()> {
    // Load the stored keypair to get the public key
    let storage_path = PathBuf::from(shellexpand::tilde(&config.session.storage_path).to_string());
    let backend = FileSystemBackend::new(storage_path);

    // Get the controller metadata to find the controller address
    let controller_metadata = backend
        .controller()
        .map_err(|e| CliError::Storage(e.to_string()))?;

    // Get the public key for context
    let public_key = match backend.get("session_signer") {
        Ok(Some(StorageValue::String(data))) => {
            let credentials: Credentials = serde_json::from_str(&data)
                .map_err(|e| CliError::InvalidSessionData(e.to_string()))?;

            let signing_key =
                starknet::signers::SigningKey::from_secret_scalar(credentials.private_key);
            let verifying_key = signing_key.verifying_key();
            format!("0x{:x}", verifying_key.scalar())
        }
        _ => {
            // No keypair found - we can still list sessions if controller exists
            String::new()
        }
    };

    let (controller_address, account_id) = match &controller_metadata {
        Some(controller) => {
            let address = format!("0x{:x}", controller.address);
            let username = controller.username.clone();
            (address, username)
        }
        None => {
            // No controller metadata, return empty list
            let output = ListSessionsOutput {
                status: "no_controller".to_string(),
                sessions: vec![],
                total_count: 0,
                active_count: 0,
            };
            formatter.success(&output);
            formatter.info("No controller found. Run 'controller generate-keypair' and 'controller register-session' first.");
            return Ok(());
        }
    };

    // Query the Cartridge API for sessions
    let api_sessions = query_controller_sessions(&config.session.api_url, &controller_address)
        .await
        .map_err(|e| CliError::ApiError(format!("Failed to query sessions: {}", e)))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Convert API sessions to our output format
    let sessions: Vec<SessionEntry> = api_sessions
        .into_iter()
        .map(|session| {
            let expires_in = session.expires_at as i64 - now as i64;
            let is_expired = expires_in <= 0 || session.is_revoked;

            let expires_at_dt = chrono::DateTime::from_timestamp(session.expires_at as i64, 0);
            let expires_at_formatted = expires_at_dt
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            SessionEntry {
                session_key_guid: session.id,
                controller_address: controller_address.clone(),
                account_id: session.controller.account_id.clone(),
                chain_id: session.chain_id.clone(),
                expires_at: session.expires_at,
                expires_at_formatted,
                is_expired,
                expires_in_seconds: expires_in,
                is_revoked: session.is_revoked,
                created_at: session.created_at,
            }
        })
        .collect();

    let total_count = sessions.len();
    let active_count = sessions.iter().filter(|s| !s.is_expired).count();

    let status = if total_count == 0 {
        "no_sessions"
    } else if active_count == 0 {
        "all_expired"
    } else {
        "active"
    };

    let output = ListSessionsOutput {
        status: status.to_string(),
        sessions,
        total_count,
        active_count,
    };

    formatter.success(&output);

    // Provide helpful context
    if total_count == 0 {
        formatter.info("No sessions found for this controller.");
        if !public_key.is_empty() {
            formatter.info("Run 'controller register-session' to create a session.");
        }
    } else if active_count == 0 {
        formatter.warning("All sessions have expired or been revoked.");
        formatter.info("Run 'controller register-session' to create a new session.");
    } else {
        formatter.info(&format!(
            "Found {} active session(s) out of {} total.",
            active_count, total_count
        ));
    }

    Ok(())
}
