use crate::{config::Config, error::{CliError, Result}, output::OutputFormatter};
use account_sdk::storage::{filestorage::FileSystemBackend, Credentials, StorageBackend, StorageValue};
use serde::Serialize;
use std::path::PathBuf;
use chrono::{DateTime, Utc};

#[derive(Serialize)]
pub struct StatusOutput {
    pub status: String,
    pub session: Option<SessionInfo>,
    pub keypair: Option<KeypairInfo>,
}

#[derive(Serialize)]
pub struct SessionInfo {
    pub address: String,
    pub expires_at: u64,
    pub expires_in_seconds: i64,
    pub expires_at_formatted: String,
    pub is_expired: bool,
}

#[derive(Serialize)]
pub struct KeypairInfo {
    pub public_key: String,
    pub has_private_key: bool,
}

pub async fn execute(config: &Config, formatter: &dyn OutputFormatter) -> Result<()> {
    let storage_path = PathBuf::from(shellexpand::tilde(&config.session.storage_path).to_string());
    let backend = FileSystemBackend::new(storage_path.clone());

    // Check for stored keypair
    let keypair_info = match backend.get("session_signer") {
        Ok(Some(StorageValue::String(data))) => {
            let credentials: Credentials = serde_json::from_str(&data)
                .map_err(|e| CliError::InvalidSessionData(e.to_string()))?;

            let signing_key = starknet::signers::SigningKey::from_secret_scalar(credentials.private_key);
            let verifying_key = signing_key.verifying_key();

            Some(KeypairInfo {
                public_key: format!("0x{:x}", verifying_key.scalar()),
                has_private_key: true,
            })
        }
        _ => None,
    };

    // Check for stored session and controller metadata
    let session_info = match backend.session("session") {
        Ok(Some(metadata)) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let expires_at = metadata.session.inner.expires_at;
            let expires_in = expires_at as i64 - now as i64;
            let is_expired = metadata.session.is_expired();

            let expires_at_dt = DateTime::from_timestamp(expires_at as i64, 0)
                .unwrap_or_else(|| Utc::now());

            // Try to get account address from controller metadata
            let address = match backend.controller() {
                Ok(Some(controller)) => format!("0x{:x}", controller.address),
                _ => "unknown".to_string(),
            };

            Some(SessionInfo {
                address,
                expires_at,
                expires_in_seconds: expires_in,
                expires_at_formatted: expires_at_dt.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                is_expired,
            })
        }
        _ => None,
    };

    let status = if session_info.is_some() && !session_info.as_ref().unwrap().is_expired {
        "active"
    } else if keypair_info.is_some() {
        "keypair_only"
    } else {
        "no_session"
    };

    let output = StatusOutput {
        status: status.to_string(),
        session: session_info,
        keypair: keypair_info,
    };

    formatter.success(&output);

    if status == "no_session" {
        formatter.info("No session found. Run 'controller-cli generate-keypair' to get started.");
    } else if status == "keypair_only" {
        formatter.info("Keypair found but no active session. Run 'controller-cli register-session' to create a session.");
    } else if let Some(session) = &output.session {
        if session.is_expired {
            formatter.warning("Session has expired. Run 'controller-cli register-session' to create a new session.");
        }
    }

    Ok(())
}
