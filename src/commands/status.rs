use crate::{
    config::Config,
    error::{CliError, Result},
    output::OutputFormatter,
};
use account_sdk::storage::{
    filestorage::FileSystemBackend, Credentials, StorageBackend, StorageValue,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize)]
pub struct StatusOutput {
    pub status: String,
    pub session: Option<SessionInfo>,
    pub keypair: Option<KeypairInfo>,
}

#[derive(Serialize)]
pub struct SessionInfo {
    pub address: String,
    pub chain_id: String,
    pub expires_at: u64,
    pub expires_in_seconds: i64,
    pub expires_at_formatted: String,
    pub is_expired: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policies: Option<PolicyInfo>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PolicyInfo {
    pub contracts: std::collections::HashMap<String, ContractPolicy>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ContractPolicy {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub methods: Vec<MethodPolicy>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MethodPolicy {
    pub name: String,
    pub entrypoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub authorized: bool,
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

            let signing_key =
                starknet::signers::SigningKey::from_secret_scalar(credentials.private_key);
            let verifying_key = signing_key.verifying_key();

            Some(KeypairInfo {
                public_key: format!("0x{:x}", verifying_key.scalar()),
                has_private_key: true,
            })
        }
        _ => None,
    };

    // Check for stored session and controller metadata
    // First get controller metadata to construct the proper session key
    let controller_metadata = backend.controller().ok().flatten();

    let session_info = if let Some(controller) = &controller_metadata {
        // Construct the session key using the same format as Controller
        let session_key = format!(
            "@cartridge/session/0x{:x}/0x{:x}",
            controller.address, controller.chain_id
        );

        match backend.session(&session_key) {
            Ok(Some(metadata)) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                let expires_at = metadata.session.inner.expires_at;
                let expires_in = expires_at as i64 - now as i64;
                let is_expired = metadata.session.is_expired();

                let expires_at_dt =
                    DateTime::from_timestamp(expires_at as i64, 0).unwrap_or_else(Utc::now);

                let address = format!("0x{:x}", controller.address);
                let chain_id =
                    starknet::core::utils::parse_cairo_short_string(&controller.chain_id)
                        .unwrap_or_else(|_| format!("0x{:x}", controller.chain_id));

                // Try to load stored policies
                let policies =
                    backend
                        .get("session_policies")
                        .ok()
                        .flatten()
                        .and_then(|v| match v {
                            StorageValue::String(data) => {
                                serde_json::from_str::<PolicyInfo>(&data).ok()
                            }
                            _ => None,
                        });

                Some(SessionInfo {
                    address,
                    chain_id,
                    expires_at,
                    expires_in_seconds: expires_in,
                    expires_at_formatted: expires_at_dt.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                    is_expired,
                    policies,
                })
            }
            _ => None,
        }
    } else {
        None
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
        formatter.info("No session found. Run 'controller generate' to get started.");
    } else if status == "keypair_only" {
        formatter.info(
            "Keypair found but no active session. Run 'controller register' to create a session.",
        );
    } else if let Some(session) = &output.session {
        if session.is_expired {
            formatter
                .warning("Session has expired. Run 'controller register' to create a new session.");
        }
    }

    Ok(())
}
