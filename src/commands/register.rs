use crate::{
    api,
    config::Config,
    error::{CliError, Result},
    output::OutputFormatter,
};
use account_sdk::storage::{
    filestorage::FileSystemBackend, Credentials, StorageBackend, StorageValue,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use url::Url;

#[derive(Serialize, Deserialize)]
pub struct PolicyFile {
    pub contracts: std::collections::HashMap<String, ContractPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<Vec<serde_json::Value>>,
}

#[derive(Serialize, Deserialize)]
pub struct ContractPolicy {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub methods: Vec<MethodPolicy>,
}

#[derive(Serialize, Deserialize)]
pub struct MethodPolicy {
    pub name: String,
    pub entrypoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
    #[serde(default = "default_authorized")]
    pub authorized: bool,
}

fn default_authorized() -> bool {
    true
}

#[derive(Serialize)]
pub struct RegisterOutput {
    pub authorization_url: String,
    pub public_key: String,
    pub message: String,
}

pub async fn execute(
    config: &Config,
    formatter: &dyn OutputFormatter,
    policy_file: Option<String>,
) -> Result<()> {
    // Load the stored keypair
    let storage_path = PathBuf::from(shellexpand::tilde(&config.session.storage_path).to_string());
    let mut backend = FileSystemBackend::new(storage_path);

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
            return Err(CliError::NoSession);
        }
    };

    // Load policies from file if provided
    let policies_json = if let Some(policy_file_path) = policy_file {
        let policy_content = std::fs::read_to_string(&policy_file_path)
            .map_err(|e| CliError::InvalidInput(format!("Failed to read policy file: {}", e)))?;

        let policy_file: PolicyFile = serde_json::from_str(&policy_content)
            .map_err(|e| CliError::InvalidInput(format!("Invalid policy file format: {}", e)))?;

        // Convert to the format expected by the keychain
        let mut policies = serde_json::json!({
            "verified": false,
            "contracts": {}
        });

        if let Some(contracts) = policies.as_object_mut() {
            if let Some(contracts_obj) = contracts.get_mut("contracts") {
                if let Some(contracts_map) = contracts_obj.as_object_mut() {
                    for (address, contract) in policy_file.contracts {
                        contracts_map.insert(
                            address,
                            serde_json::json!({
                                "methods": contract.methods
                            }),
                        );
                    }
                }
            }
        }

        if let Some(messages) = policy_file.messages {
            policies["messages"] = serde_json::json!(messages);
        }

        serde_json::to_string(&policies)
            .map_err(|e| CliError::InvalidInput(format!("Failed to serialize policies: {}", e)))?
    } else {
        // Default empty policies for wildcard session
        serde_json::to_string(&serde_json::json!({
            "verified": false,
            "contracts": {}
        }))
        .unwrap()
    };

    // Build the authorization URL
    let mut url = Url::parse(&format!("{}/session", config.session.keychain_url))
        .map_err(|e| CliError::InvalidInput(format!("Invalid keychain URL: {}", e)))?;

    url.query_pairs_mut()
        .append_pair("public_key", &public_key)
        .append_pair("redirect_uri", "https://x.cartridge.gg")
        .append_pair("redirect_query_name", "startapp")
        .append_pair("policies", &policies_json)
        .append_pair("rpc_url", &config.session.default_rpc_url)
        .append_pair("mode", "cli"); // Tell keychain this is CLI mode (don't include session data in redirect)

    let authorization_url = url.to_string();

    // First, check if session already exists (idempotency)
    formatter.info("Checking for existing session...");

    // Calculate session_key_guid from the public key
    let session_key_guid = {
        let pubkey_felt = starknet::core::types::Felt::from_hex(&public_key)
            .map_err(|e| CliError::InvalidInput(format!("Invalid public key: {}", e)))?;
        format!("0x{:x}", pubkey_felt)
    };

    // Check if session already exists
    if let Some(session_info) =
        api::query_session_info(&config.session.api_url, &session_key_guid).await?
    {
        formatter.info("Session already exists! Storing credentials...");

        // Store the session directly
        store_session_from_api(&mut backend, session_info, &public_key)?;

        formatter.success(&serde_json::json!({
            "message": "Session already registered and stored successfully",
            "public_key": public_key,
        }));

        return Ok(());
    }

    // No existing session, show URL and start polling
    let output = RegisterOutput {
        authorization_url: authorization_url.clone(),
        public_key: public_key.clone(),
        message:
            "Open this URL in your browser to authorize the session. Waiting for authorization..."
                .to_string(),
    };

    formatter.success(&output);

    if !config.cli.json_output {
        formatter.info("\nAuthorization URL:");
        println!("\n{}\n", authorization_url);
        formatter.info("\nWaiting for authorization (timeout: 5 minutes)...");
    }

    // Poll for session creation
    let timeout_secs = 300; // 5 minutes
    let poll_interval = Duration::from_secs(3); // Poll every 3 seconds
    let start = Instant::now();

    loop {
        // Check timeout
        if start.elapsed().as_secs() >= timeout_secs {
            return Err(CliError::CallbackTimeout(timeout_secs));
        }

        // Query session info
        if let Some(session_info) =
            api::query_session_info(&config.session.api_url, &session_key_guid).await?
        {
            formatter.info("Authorization received! Storing session...");

            // Store the session
            store_session_from_api(&mut backend, session_info, &public_key)?;

            formatter.success(&serde_json::json!({
                "message": "Session registered and stored successfully",
                "public_key": public_key,
            }));

            return Ok(());
        }

        // Wait before next poll
        tokio::time::sleep(poll_interval).await;
    }
}

/// Store session credentials from API response
fn store_session_from_api(
    backend: &mut FileSystemBackend,
    session_info: api::SessionInfo,
    public_key: &str,
) -> Result<()> {
    use account_sdk::{
        account::session::hash::Session,
        storage::{ControllerMetadata, Credentials, Owner, SessionMetadata},
    };

    // Parse authorization as Vec<Felt>
    let authorization = session_info.authorization_as_felts()?;

    // Parse address and chain_id from subscription response
    let address = session_info.address_as_felt()?;
    let chain_id = session_info.chain_id_as_felt()?;

    // Calculate session_key_guid from public key
    let pubkey_felt = starknet::core::types::Felt::from_hex(public_key)
        .map_err(|e| CliError::InvalidInput(format!("Invalid public key: {}", e)))?;
    let session_key_guid = pubkey_felt;

    // Create session metadata
    let session_metadata = SessionMetadata {
        credentials: Some(Credentials {
            authorization: authorization.clone(),
            private_key: starknet::core::types::Felt::ZERO, // Will be filled from session_signer storage
        }),
        session: Session {
            inner: account_sdk::abigen::controller::Session {
                expires_at: session_info.expires_at,
                allowed_policies_root: starknet::core::types::Felt::ZERO, // TODO: Calculate from policies
                metadata_hash: starknet::core::types::Felt::ZERO,
                session_key_guid,
                guardian_key_guid: starknet::core::types::Felt::ZERO,
            },
            requested_policies: vec![],
            proved_policies: vec![],
            metadata: "{}".to_string(),
        },
        max_fee: None,
        is_registered: true,
    };

    // Create minimal controller metadata with placeholder values
    // We only need address and chain_id for SessionAccount::new()
    let controller_metadata = ControllerMetadata {
        address,
        chain_id,
        class_hash: starknet::core::types::Felt::ZERO, // Not needed for execution
        rpc_url: "".to_string(),                       // Not used (CLI uses config.session.default_rpc_url)
        salt: starknet::core::types::Felt::ZERO,       // Not needed for execution
        owner: Owner::Account(starknet::core::types::Felt::ZERO), // Not needed for execution with authorization
        username: session_info.controller.account_id.clone(), // Use account_id as username
    };

    // Store session and controller metadata
    backend
        .set_session("session", session_metadata)
        .map_err(|e| CliError::Storage(e.to_string()))?;

    backend
        .set_controller(&chain_id, address, controller_metadata)
        .map_err(|e| CliError::Storage(e.to_string()))?;

    Ok(())
}
