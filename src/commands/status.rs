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

#[derive(Serialize)]
pub struct StatusOutput {
    pub session: Option<SessionInfo>,
}

#[derive(Serialize)]
pub struct SessionInfo {
    pub guid: String,
    pub public_key: String,
    pub address: String,
    pub chain_id: String,
    pub expires_at: u64,
    pub expires_in_seconds: i64,
    pub expires_at_formatted: String,
    pub is_expired: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policies: Option<Vec<String>>,
}

/// Raw stored format (for deserialization only)
#[derive(Deserialize)]
struct StoredPolicyInfo {
    contracts: std::collections::HashMap<String, StoredContractPolicy>,
}

#[derive(Deserialize)]
struct StoredContractPolicy {
    methods: Vec<StoredMethodPolicy>,
}

#[derive(Deserialize)]
struct StoredMethodPolicy {
    entrypoint: String,
}

pub async fn execute(
    config: &Config,
    formatter: &dyn OutputFormatter,
    account: Option<&str>,
) -> Result<()> {
    let storage_path = config.resolve_storage_path(account);
    let backend = FileSystemBackend::new(storage_path.clone());

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

                // Try to load stored policies as flat "address:entrypoint" list
                let policies = backend
                    .get("session_policies")
                    .ok()
                    .flatten()
                    .and_then(|v| match v {
                        StorageValue::String(data) => {
                            serde_json::from_str::<StoredPolicyInfo>(&data).ok()
                        }
                        _ => None,
                    })
                    .map(|info| {
                        let mut entries: Vec<String> = info
                            .contracts
                            .iter()
                            .flat_map(|(addr, contract)| {
                                contract
                                    .methods
                                    .iter()
                                    .map(move |m| format!("{addr}:{}", m.entrypoint))
                            })
                            .collect();
                        entries.sort();
                        entries
                    });

                let session_key_guid =
                    backend
                        .get("session_key_guid")
                        .ok()
                        .flatten()
                        .and_then(|v| match v {
                            StorageValue::String(s) => Some(s),
                            _ => None,
                        });

                match session_key_guid {
                    Some(guid) => {
                        let public_key = match backend.get("session_signer") {
                            Ok(Some(StorageValue::String(data))) => {
                                let credentials: Credentials = serde_json::from_str(&data)
                                    .map_err(|e| CliError::InvalidSessionData(e.to_string()))?;
                                let signing_key = starknet::signers::SigningKey::from_secret_scalar(
                                    credentials.private_key,
                                );
                                format!("0x{:x}", signing_key.verifying_key().scalar())
                            }
                            _ => String::new(),
                        };

                        Some(SessionInfo {
                            guid,
                            public_key,
                            address,
                            chain_id,
                            expires_at,
                            expires_in_seconds: expires_in,
                            expires_at_formatted: expires_at_dt
                                .format("%Y-%m-%d %H:%M:%S UTC")
                                .to_string(),
                            is_expired,
                            policies,
                        })
                    }
                    None => {
                        formatter.warning("Session data is outdated. Run 'controller session auth' to create a new session.");
                        None
                    }
                }
            }
            _ => None,
        }
    } else {
        None
    };

    let output = StatusOutput {
        session: session_info,
    };

    formatter.success(&output);

    if output.session.is_none() {
        formatter.info("No session found. Run 'controller session auth' to get started.");
    }

    Ok(())
}
