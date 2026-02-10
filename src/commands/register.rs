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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_url: Option<String>,
    pub public_key: String,
    pub message: String,
}

pub async fn execute(
    config: &Config,
    formatter: &dyn OutputFormatter,
    policy_file: String,
    rpc_url: Option<String>,
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

    // Load policies from file
    let policy_content = std::fs::read_to_string(&policy_file)
        .map_err(|e| CliError::InvalidInput(format!("Failed to read policy file: {}", e)))?;

    let policy_file: PolicyFile = serde_json::from_str(&policy_content)
        .map_err(|e| CliError::InvalidInput(format!("Invalid policy file format: {}", e)))?;

    // Convert to the format expected by the keychain
    let mut policies = serde_json::json!({
        "verified": false,
        "contracts": {}
    });

    // Also build Policy structures for storage
    let mut policy_vec = Vec::new();

    if let Some(contracts) = policies.as_object_mut() {
        if let Some(contracts_obj) = contracts.get_mut("contracts") {
            if let Some(contracts_map) = contracts_obj.as_object_mut() {
                for (address, contract) in policy_file.contracts {
                    contracts_map.insert(
                        address.clone(),
                        serde_json::json!({
                            "methods": contract.methods
                        }),
                    );

                    // Parse address and create Policy for each method
                    let contract_address = starknet::core::types::Felt::from_hex(&address)
                        .map_err(|e| {
                            CliError::InvalidInput(format!(
                                "Invalid contract address {}: {}",
                                address, e
                            ))
                        })?;

                    for method in &contract.methods {
                        // Compute selector from entrypoint name
                        let selector =
                            starknet::core::utils::get_selector_from_name(&method.entrypoint)
                                .map_err(|e| {
                                    CliError::InvalidInput(format!(
                                        "Invalid entrypoint name {}: {}",
                                        method.entrypoint, e
                                    ))
                                })?;

                        policy_vec.push(account_sdk::account::session::policy::Policy::Call(
                            account_sdk::account::session::policy::CallPolicy {
                                contract_address,
                                selector,
                                authorized: Some(method.authorized),
                            },
                        ));
                    }
                }
            }
        }
    }

    if let Some(messages) = policy_file.messages {
        policies["messages"] = serde_json::json!(messages);
    }

    let policies_json = serde_json::to_string(&policies)
        .map_err(|e| CliError::InvalidInput(format!("Failed to serialize policies: {}", e)))?;
    let parsed_policies = policy_vec;

    // Check if there's an active unexpired session for the same keypair
    let controller_metadata = backend.controller().ok().flatten();
    if let Some(controller) = &controller_metadata {
        let session_key = format!(
            "@cartridge/session/0x{:x}/0x{:x}",
            controller.address, controller.chain_id
        );
        if let Ok(Some(metadata)) = backend.session(&session_key) {
            if !metadata.session.is_expired()
                && metadata.session.inner.session_key_guid == {
                    use starknet::macros::short_string;
                    use starknet_crypto::poseidon_hash;

                    let pubkey_felt =
                        starknet::core::types::Felt::from_hex(&public_key).unwrap_or_default();
                    poseidon_hash(short_string!("Starknet Signer"), pubkey_felt)
                }
            {
                formatter.warning("Active session exists for this keypair. A session keypair can only be registered once.");
                formatter.info(
                    "Run 'controller generate-keypair' to create a new keypair, then re-register.",
                );
                return Ok(());
            }
        }
    }

    // Use CLI flag if provided, otherwise use config
    let effective_rpc_url = rpc_url.as_ref().unwrap_or(&config.session.default_rpc_url);

    // If --rpc-url was provided, validate it's a Cartridge RPC endpoint
    if let Some(ref url) = rpc_url {
        if !url.starts_with("https://api.cartridge.gg") {
            return Err(CliError::InvalidInput(
                "Only Cartridge RPC endpoints are supported. Use: https://api.cartridge.gg/x/starknet/mainnet or https://api.cartridge.gg/x/starknet/sepolia".to_string()
            ));
        }
    }

    // If --rpc-url was provided, validate it works by querying chain_id
    if rpc_url.is_some() {
        formatter.info("Validating RPC endpoint...");
        let provider = starknet::providers::jsonrpc::JsonRpcClient::new(
            starknet::providers::jsonrpc::HttpTransport::new(
                url::Url::parse(effective_rpc_url)
                    .map_err(|e| CliError::InvalidInput(format!("Invalid RPC URL: {}", e)))?,
            ),
        );

        match starknet::providers::Provider::chain_id(&provider).await {
            Ok(_chain_id) => {
                // Validation successful, continue
            }
            Err(e) => {
                return Err(CliError::InvalidInput(format!(
                    "RPC endpoint not responding: {}",
                    e
                )));
            }
        }
    }

    // Build the authorization URL
    let mut url = Url::parse(&format!("{}/session", config.session.keychain_url))
        .map_err(|e| CliError::InvalidInput(format!("Invalid keychain URL: {}", e)))?;

    url.query_pairs_mut()
        .append_pair("public_key", &public_key)
        .append_pair("redirect_uri", "https://x.cartridge.gg")
        .append_pair("policies", &policies_json)
        .append_pair("rpc_url", effective_rpc_url)
        .append_pair("mode", "cli"); // Tell keychain this is CLI mode (don't include session data in redirect)

    let authorization_url = url.to_string();

    // Try to shorten the URL for a cleaner display
    let short_url = api::shorten_url(&config.session.api_url, &authorization_url)
        .await
        .ok();

    // Show URL and start polling
    let display_url = short_url.as_deref().unwrap_or(&authorization_url);

    let output = RegisterOutput {
        authorization_url: authorization_url.clone(),
        short_url: short_url.clone(),
        public_key: public_key.clone(),
        message:
            "Open this URL in your browser to authorize the session. Waiting for authorization..."
                .to_string(),
    };

    if config.cli.json_output {
        formatter.success(&output);
    } else {
        formatter.info(&format!("Chain: {}", config.session.default_chain_id));
        formatter.info("Authorization URL:");
        println!("\n{}\n", display_url);
        formatter.info("Waiting for authorization (timeout: 5 minutes)...");
    }

    // Calculate session_key_guid for long-polling query
    // GUID = poseidon_hash("Starknet Signer", public_key)
    let session_key_guid = {
        use starknet::macros::short_string;
        use starknet_crypto::poseidon_hash;

        let pubkey_felt = starknet::core::types::Felt::from_hex(&public_key)
            .map_err(|e| CliError::InvalidInput(format!("Invalid public key: {}", e)))?;

        let guid = poseidon_hash(short_string!("Starknet Signer"), pubkey_felt);
        format!("0x{:x}", guid)
    };

    // Query with long-polling (backend holds connection for ~2 minutes)
    // Retry if backend times out without finding session
    let max_attempts = 3; // 3 attempts × 2min = ~6 minutes total
    let mut attempts = 0;

    loop {
        attempts += 1;

        match api::query_session_info(&config.session.api_url, &session_key_guid).await? {
            Some(session_info) => {
                let chain_id = session_info.chain_id.clone();

                // Store the session with policies
                store_session_from_api(
                    &mut backend,
                    session_info,
                    &public_key,
                    parsed_policies.clone(),
                )?;

                // Store chain_id and RPC URL for status/execute
                backend
                    .set("session_chain_id", &StorageValue::String(chain_id.clone()))
                    .map_err(|e| CliError::Storage(e.to_string()))?;
                backend
                    .set(
                        "session_rpc_url",
                        &StorageValue::String(effective_rpc_url.clone()),
                    )
                    .map_err(|e| CliError::Storage(e.to_string()))?;

                if config.cli.json_output {
                    formatter.success(&serde_json::json!({
                        "message": "Session registered and stored successfully",
                        "public_key": public_key,
                        "chain_id": chain_id,
                    }));
                } else {
                    formatter.info("Session registered and stored successfully.");
                }

                return Ok(());
            }
            None => {
                // Backend timed out without finding session
                if attempts >= max_attempts {
                    return Err(CliError::CallbackTimeout(max_attempts * 120)); // ~6 minutes
                }
                // Backend will retry automatically on next call
                continue;
            }
        }
    }
}

/// Store session credentials from API response
fn store_session_from_api(
    backend: &mut FileSystemBackend,
    session_info: api::SessionInfo,
    public_key: &str,
    policies: Vec<account_sdk::account::session::policy::Policy>,
) -> Result<()> {
    use account_sdk::{
        account::session::hash::Session,
        storage::{ControllerMetadata, Credentials, Owner, SessionMetadata, StorageValue},
    };

    // Load the private key from session_signer storage
    let private_key = match backend.get("session_signer") {
        Ok(Some(StorageValue::String(data))) => {
            let credentials: Credentials = serde_json::from_str(&data)
                .map_err(|e| CliError::InvalidSessionData(e.to_string()))?;
            credentials.private_key
        }
        _ => {
            return Err(CliError::NoSession);
        }
    };

    // Parse authorization as Vec<Felt>
    let authorization = session_info.authorization_as_felts()?;

    // Parse address and chain_id from subscription response
    let address = session_info.address_as_felt()?;
    let chain_id = session_info.chain_id_as_felt()?;

    // Parse public key to create session signer
    let pubkey_felt = starknet::core::types::Felt::from_hex(public_key)
        .map_err(|e| CliError::InvalidInput(format!("Invalid public key: {}", e)))?;

    // Create StarknetSigner from public key (pubkey is already a Felt, no conversion needed)
    use cainome_cairo_serde::NonZero;
    let session_signer = account_sdk::abigen::controller::Signer::Starknet(
        account_sdk::abigen::controller::StarknetSigner {
            pubkey: NonZero::new(pubkey_felt)
                .ok_or_else(|| CliError::InvalidInput("Invalid public key (zero)".to_string()))?,
        },
    );

    // Use Session::new() which properly computes merkle root and proofs
    let session = Session::new(
        policies,
        session_info.expires_at,
        &session_signer,
        starknet::core::types::Felt::ZERO, // guardian_key_guid
    )
    .map_err(|e| CliError::InvalidSessionData(format!("Failed to create session: {}", e)))?;

    // Create session metadata
    let session_metadata = SessionMetadata {
        credentials: Some(Credentials {
            authorization: authorization.clone(),
            private_key, // Use the actual private key from session_signer storage
        }),
        session,
        max_fee: None,
        is_registered: true,
    };

    // Create minimal controller metadata with placeholder values
    // We only need address and chain_id for SessionAccount::new()
    let controller_metadata = ControllerMetadata {
        address,
        chain_id,
        class_hash: starknet::core::types::Felt::ZERO, // Not needed for execution
        rpc_url: "".to_string(), // Not used (CLI uses config.session.default_rpc_url)
        salt: starknet::core::types::Felt::ZERO, // Not needed for execution
        owner: Owner::Account(starknet::core::types::Felt::ZERO), // Not needed for execution with authorization
        username: session_info.controller.account_id.clone(),     // Use account_id as username
    };

    // Store session and controller metadata using the correct key format
    // Key format: @cartridge/session/0x{address:x}/0x{chain_id:x}
    let session_key = format!("@cartridge/session/0x{:x}/0x{:x}", address, chain_id);

    backend
        .set_session(&session_key, session_metadata)
        .map_err(|e| CliError::Storage(e.to_string()))?;

    backend
        .set_controller(&chain_id, address, controller_metadata)
        .map_err(|e| CliError::Storage(e.to_string()))?;

    Ok(())
}
