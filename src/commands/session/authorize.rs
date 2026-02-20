use crate::{
    api,
    config::Config,
    error::{CliError, Result},
    output::OutputFormatter,
    presets,
};
use account_sdk::storage::{
    filestorage::FileSystemBackend, Credentials, StorageBackend, StorageValue,
};
use serde::{Deserialize, Serialize};
use starknet::signers::SigningKey;
use std::fmt::Display;
use url::Url;

#[derive(Serialize, Deserialize)]
pub struct PolicyFile {
    pub contracts: std::collections::HashMap<String, ContractPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<Vec<serde_json::Value>>,
}

// Simplified policy storage for status command
#[derive(Serialize, Deserialize, Clone)]
pub struct PolicyStorage {
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
    #[serde(default = "default_authorized")]
    pub authorized: bool,
}

fn default_authorized() -> bool {
    true
}

#[derive(Serialize)]
pub struct AuthorizeOutput {
    pub authorization_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_url: Option<String>,
    pub public_key: String,
    pub message: String,
}

fn try_open_authorization_url(formatter: &dyn OutputFormatter, url: &str) {
    let _ = try_open_authorization_url_with(formatter, url, webbrowser::open);
}

fn try_open_authorization_url_with<F, E>(
    formatter: &dyn OutputFormatter,
    url: &str,
    opener: F,
) -> bool
where
    F: FnOnce(&str) -> std::result::Result<(), E>,
    E: Display,
{
    match opener(url) {
        Ok(()) => true,
        Err(e) => {
            formatter.warning(&format!(
                "Could not open browser automatically: {e}. Please open the URL manually."
            ));
            false
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn execute(
    config: &Config,
    formatter: &dyn OutputFormatter,
    preset: Option<String>,
    file: Option<String>,
    chain_id: Option<String>,
    rpc_url: Option<String>,
    overwrite: bool,
    account: Option<&str>,
) -> Result<()> {
    // Validate that either preset or file is provided
    if preset.is_none() && file.is_none() {
        return Err(CliError::InvalidInput(
            "Session policies are required. Use --preset <name> to load a preset policy or --file <path> to provide a local policy JSON file".to_string(),
        ));
    }

    if let Some(name) = account {
        formatter.info(&format!("Using account: {name}"));
    }

    // Check if there's an active unexpired session before proceeding
    let storage_path = config.resolve_storage_path(account);
    let backend = FileSystemBackend::new(storage_path.clone());

    let controller_metadata = backend.controller().ok().flatten();
    if let Some(controller) = &controller_metadata {
        let session_key = format!(
            "@cartridge/session/0x{:x}/0x{:x}",
            controller.address, controller.chain_id
        );
        if let Ok(Some(metadata)) = backend.session(&session_key) {
            if !metadata.session.is_expired() && !overwrite {
                formatter.warning(
                    "An active session already exists. Authorizing a new session will replace it.",
                );
                eprint!("Continue? [y/N] ");
                let mut input = String::new();
                std::io::stdin()
                    .read_line(&mut input)
                    .map_err(|e| CliError::InvalidInput(format!("Failed to read input: {e}")))?;
                if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
                    formatter.info("Aborted.");
                    return Ok(());
                }
            }
        }
    }

    // Map chain_id to RPC URL if provided
    let resolved_rpc_url = if let Some(ref chain_id_str) = chain_id {
        match chain_id_str.as_str() {
            "SN_MAIN" => Some("https://api.cartridge.gg/x/starknet/mainnet".to_string()),
            "SN_SEPOLIA" => Some("https://api.cartridge.gg/x/starknet/sepolia".to_string()),
            _ => {
                return Err(CliError::InvalidInput(format!(
                    "Unsupported chain ID '{chain_id_str}'. Supported chains: SN_MAIN, SN_SEPOLIA. \
                     For Cartridge SLOT or other chains, use --rpc-url to specify your Katana endpoint."
                )));
            }
        }
    } else if rpc_url.is_some() {
        rpc_url.clone()
    } else if config.session.rpc_url_explicitly_set {
        Some(config.session.rpc_url.clone())
    } else {
        formatter.warning("No --chain-id or --rpc-url specified, using SN_SEPOLIA by default");
        Some(config.session.rpc_url.clone())
    };

    // Generate a new session keypair
    let signing_key = SigningKey::from_random();
    let verifying_key = signing_key.verifying_key();
    let public_key = format!("0x{:x}", verifying_key.scalar());
    let private_key = signing_key.secret_scalar();

    // Re-open storage as mutable for writes (ensure directory exists for named accounts)
    if account.is_some() {
        std::fs::create_dir_all(&storage_path)
            .map_err(|e| CliError::Storage(format!("Failed to create account directory: {e}")))?;
    }
    let mut backend = FileSystemBackend::new(storage_path.clone());

    // Store the keypair as session credentials
    let credentials = Credentials {
        private_key,
        authorization: vec![],
    };

    let credentials_json =
        serde_json::to_string(&credentials).map_err(|e| CliError::InvalidInput(e.to_string()))?;

    backend
        .set("session_signer", &StorageValue::String(credentials_json))
        .map_err(|e| CliError::Storage(e.to_string()))?;

    // Load policies from preset or file
    let policy_file: PolicyFile = if let Some(preset_name) = preset {
        // Fetch preset from GitHub
        let preset_config = presets::fetch_preset(&preset_name).await?;

        // Use resolved RPC URL or fall back to config default for preset chain detection
        let preset_rpc_url = resolved_rpc_url.as_ref().unwrap_or(&config.session.rpc_url);
        {
            let rpc_url_str = preset_rpc_url;
            let provider = starknet::providers::jsonrpc::JsonRpcClient::new(
                starknet::providers::jsonrpc::HttpTransport::new(
                    url::Url::parse(rpc_url_str)
                        .map_err(|e| CliError::InvalidInput(format!("Invalid RPC URL: {e}")))?,
                ),
            );

            let chain_id = starknet::providers::Provider::chain_id(&provider)
                .await
                .map_err(|e| {
                    CliError::InvalidInput(format!("Failed to query chain_id from RPC: {e}"))
                })?;

            let chain_name = starknet::core::utils::parse_cairo_short_string(&chain_id)
                .unwrap_or_else(|_| format!("0x{chain_id:x}"));

            // Extract chain-specific policies
            let chain_policies =
                presets::extract_chain_policies(&preset_config, &chain_name, &preset_name)?;

            // Convert to PolicyFile format
            let contracts: std::collections::HashMap<String, ContractPolicy> = chain_policies
                .contracts
                .into_iter()
                .map(|(addr, contract)| {
                    (
                        addr,
                        ContractPolicy {
                            name: Some(contract.name),
                            methods: contract
                                .methods
                                .into_iter()
                                .map(|m| MethodPolicy {
                                    name: m.name,
                                    entrypoint: m.entrypoint,
                                    description: m.description,
                                    amount: None,
                                    authorized: true,
                                })
                                .collect(),
                        },
                    )
                })
                .collect();

            PolicyFile {
                contracts,
                messages: chain_policies.messages,
            }
        }
    } else if let Some(file_path) = file {
        // Load from local file
        let policy_content = std::fs::read_to_string(&file_path)
            .map_err(|e| CliError::InvalidInput(format!("Failed to read policy file: {e}")))?;

        serde_json::from_str(&policy_content)
            .map_err(|e| CliError::InvalidInput(format!("Invalid policy file format: {e}")))?
    } else {
        unreachable!("Either preset or file must be provided");
    };

    let total_contracts = policy_file.contracts.len();
    let total_entrypoints: usize = policy_file
        .contracts
        .values()
        .map(|c| c.methods.len())
        .sum();
    formatter.info(&format!(
        "Policies loaded: {total_contracts} contracts, {total_entrypoints} entrypoints"
    ));

    // Convert to the format expected by the keychain
    let mut policies = serde_json::json!({
        "verified": false,
        "contracts": {}
    });

    // Also build Policy structures for storage
    // IMPORTANT: Sort contracts by address and methods by entrypoint name to match
    // the frontend's toWasmPolicies() canonical ordering. Without this, the Merkle
    // tree root will differ from what was registered on-chain, causing session/not-registered.
    let mut policy_vec = Vec::new();

    if let Some(contracts) = policies.as_object_mut() {
        if let Some(contracts_obj) = contracts.get_mut("contracts") {
            if let Some(contracts_map) = contracts_obj.as_object_mut() {
                // Sort contracts by address (case-insensitive) to match toWasmPolicies
                let mut sorted_contracts: Vec<_> = policy_file.contracts.iter().collect();
                sorted_contracts.sort_by(|(a, _), (b, _)| a.to_lowercase().cmp(&b.to_lowercase()));

                for (address, contract) in &sorted_contracts {
                    contracts_map.insert(
                        address.to_string(),
                        serde_json::json!({
                            "methods": &contract.methods
                        }),
                    );

                    // Parse address and create Policy for each method
                    let contract_address =
                        starknet::core::types::Felt::from_hex(address).map_err(|e| {
                            CliError::InvalidInput(format!(
                                "Invalid contract address {address}: {e}"
                            ))
                        })?;

                    // Sort methods by entrypoint name to match toWasmPolicies
                    let mut sorted_methods = contract.methods.clone();
                    sorted_methods.sort_by(|a, b| a.entrypoint.cmp(&b.entrypoint));

                    for method in &sorted_methods {
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
        .map_err(|e| CliError::InvalidInput(format!("Failed to serialize policies: {e}")))?;
    let parsed_policies = policy_vec;

    // Use CLI flag if provided, otherwise use config
    let effective_rpc_url = resolved_rpc_url.as_ref().unwrap_or(&config.session.rpc_url);

    // If --rpc-url or --chain-id was provided, validate it's a Cartridge RPC endpoint
    if let Some(ref url) = resolved_rpc_url {
        if !url.starts_with("https://api.cartridge.gg") {
            return Err(CliError::InvalidInput(
                "Only Cartridge RPC endpoints are supported. Use: https://api.cartridge.gg/x/starknet/mainnet or https://api.cartridge.gg/x/starknet/sepolia".to_string()
            ));
        }
    }

    // Query chain_id from the RPC endpoint to display in authorization URL
    let provider = starknet::providers::jsonrpc::JsonRpcClient::new(
        starknet::providers::jsonrpc::HttpTransport::new(
            url::Url::parse(effective_rpc_url)
                .map_err(|e| CliError::InvalidInput(format!("Invalid RPC URL: {e}")))?,
        ),
    );

    let detected_chain_name = match starknet::providers::Provider::chain_id(&provider).await {
        Ok(chain_id_felt) => {
            // Parse chain name for display
            let chain_name = starknet::core::utils::parse_cairo_short_string(&chain_id_felt)
                .unwrap_or_else(|_| format!("0x{chain_id_felt:x}"));
            Some(chain_name)
        }
        Err(e) => {
            // Only error out if --rpc-url or --chain-id was explicitly provided
            if resolved_rpc_url.is_some() {
                return Err(CliError::InvalidInput(format!(
                    "RPC endpoint not responding: {e}"
                )));
            }
            None
        }
    };

    // Build the authorization URL
    let mut url = Url::parse(&format!("{}/session", config.session.keychain_url))
        .map_err(|e| CliError::InvalidInput(format!("Invalid keychain URL: {e}")))?;

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
    try_open_authorization_url(formatter, display_url);

    let output = AuthorizeOutput {
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
        if let Some(chain_name) = detected_chain_name {
            formatter.info(&format!("Authorization URL ({chain_name}):"));
        } else {
            formatter.info("Authorization URL:");
        }
        println!("\n{display_url}\n");
        formatter.info("Waiting for authorization...");
    }

    // Calculate session_key_guid for long-polling query
    // GUID = poseidon_hash("Starknet Signer", public_key)
    let session_key_guid = {
        use starknet::macros::short_string;
        use starknet_crypto::poseidon_hash;

        let pubkey_felt = starknet::core::types::Felt::from_hex(&public_key)
            .map_err(|e| CliError::InvalidInput(format!("Invalid public key: {e}")))?;

        let guid = poseidon_hash(short_string!("Starknet Signer"), pubkey_felt);
        format!("0x{guid:x}")
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

                // Store policies for display in status command
                let policies_storage = PolicyStorage {
                    contracts: policy_file.contracts.clone(),
                };
                let policies_json = serde_json::to_string(&policies_storage)
                    .map_err(|e| CliError::Storage(format!("Failed to serialize policies: {e}")))?;
                backend
                    .set("session_policies", &StorageValue::String(policies_json))
                    .map_err(|e| CliError::Storage(e.to_string()))?;
                backend
                    .set(
                        "session_key_guid",
                        &StorageValue::String(session_key_guid.clone()),
                    )
                    .map_err(|e| CliError::Storage(e.to_string()))?;

                if config.cli.json_output {
                    formatter.success(&serde_json::json!({
                        "message": "Session authorized and stored successfully",
                        "public_key": public_key,
                        "chain_id": chain_id,
                    }));
                } else {
                    formatter.info("Session authorized and stored successfully.");
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
        .map_err(|e| CliError::InvalidInput(format!("Invalid public key: {e}")))?;

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
    .map_err(|e| CliError::InvalidSessionData(format!("Failed to create session: {e}")))?;

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
        rpc_url: "".to_string(),                       // Not used (CLI uses config.session.rpc_url)
        salt: starknet::core::types::Felt::ZERO,       // Not needed for execution
        owner: Owner::Account(starknet::core::types::Felt::ZERO), // Not needed for execution with authorization
        username: session_info.controller.account_id.clone(),     // Use account_id as username
    };

    // Store session and controller metadata using the correct key format
    // Key format: @cartridge/session/0x{address:x}/0x{chain_id:x}
    let session_key = format!("@cartridge/session/0x{address:x}/0x{chain_id:x}");

    backend
        .set_session(&session_key, session_metadata)
        .map_err(|e| CliError::Storage(e.to_string()))?;

    backend
        .set_controller(&chain_id, address, controller_metadata)
        .map_err(|e| CliError::Storage(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CliError;
    use std::cell::{Cell, RefCell};

    #[derive(Default)]
    struct TestFormatter {
        warnings: RefCell<Vec<String>>,
    }

    impl OutputFormatter for TestFormatter {
        fn success(&self, _data: &dyn erased_serde::Serialize) {}

        fn error(&self, _error: &CliError) {}

        fn info(&self, _message: &str) {}

        fn warning(&self, message: &str) {
            self.warnings.borrow_mut().push(message.to_string());
        }
    }

    #[test]
    fn opens_authorization_url_when_opener_succeeds() {
        let formatter = TestFormatter::default();
        let called = Cell::new(false);
        let target_url = "https://example.com/session";

        let opened = try_open_authorization_url_with(
            &formatter,
            target_url,
            |url| -> std::result::Result<(), &str> {
                called.set(url == target_url);
                Ok(())
            },
        );

        assert!(opened);
        assert!(called.get());
        assert!(formatter.warnings.borrow().is_empty());
    }

    #[test]
    fn warns_when_browser_open_fails() {
        let formatter = TestFormatter::default();
        let opened = try_open_authorization_url_with(
            &formatter,
            "https://example.com/session",
            |_url| -> std::result::Result<(), &str> { Err("mock failure") },
        );

        assert!(!opened);
        let warnings = formatter.warnings.borrow();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Could not open browser automatically: mock failure"));
    }
}
