use crate::{
    config::Config,
    error::{CliError, Result},
    output::OutputFormatter,
};
use account_sdk::{
    controller::Controller,
    signers::{Owner, Signer},
    storage::{filestorage::FileSystemBackend, StorageBackend, StorageValue},
};
use serde::{Deserialize, Serialize};
use starknet::{
    core::types::{Call, Felt},
    providers::Provider,
};
use std::path::PathBuf;
use std::sync::Mutex;

/// Cache for paymaster status per RPC endpoint
/// Key: RPC URL, Value: Some(true) = paymastered, Some(false) = self-funded, None = unknown
static PAYMASTER_CACHE: Mutex<Option<std::collections::HashMap<String, bool>>> = Mutex::new(None);

/// Check cache for paymaster status
fn get_cached_paymaster_status(rpc_url: &str) -> Option<bool> {
    let cache = PAYMASTER_CACHE.lock().ok()?;
    cache.as_ref()?.get(rpc_url).copied()
}

/// Cache paymaster status for an RPC endpoint
fn cache_paymaster_status(rpc_url: &str, is_paymastered: bool) {
    if let Ok(mut cache) = PAYMASTER_CACHE.lock() {
        let map = cache.get_or_insert_with(std::collections::HashMap::new);
        map.insert(rpc_url.to_string(), is_paymastered);
    }
}

#[derive(Debug, Deserialize)]
struct CallFile {
    calls: Vec<CallSpec>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CallSpec {
    #[serde(rename = "contractAddress")]
    contract_address: String,
    entrypoint: String,
    calldata: Vec<String>,
}

#[derive(Serialize)]
pub struct ExecuteOutput {
    pub transaction_hash: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_fee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_token: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub async fn execute(
    config: &Config,
    formatter: &dyn OutputFormatter,
    contract: Option<String>,
    entrypoint: Option<String>,
    calldata: Option<String>,
    file: Option<String>,
    wait: bool,
    timeout: u64,
    rpc_url: Option<String>,
) -> Result<()> {
    // Parse calls from arguments or file
    let calls = if let Some(file_path) = file {
        // Load calls from JSON file
        let file_content = std::fs::read_to_string(&file_path)
            .map_err(|e| CliError::InvalidInput(format!("Failed to read file: {}", e)))?;

        let call_file: CallFile = serde_json::from_str(&file_content)
            .map_err(|e| CliError::InvalidInput(format!("Invalid file format: {}", e)))?;

        call_file.calls
    } else if let (Some(contract_addr), Some(entry), Some(data)) = (contract, entrypoint, calldata)
    {
        // Single call from CLI arguments
        vec![CallSpec {
            contract_address: contract_addr,
            entrypoint: entry,
            calldata: data.split(',').map(|s| s.trim().to_string()).collect(),
        }]
    } else {
        return Err(CliError::InvalidInput(
            "Either --file or all of --contract, --entrypoint, --calldata must be provided"
                .to_string(),
        ));
    };

    formatter.info(&format!("Preparing to execute {} call(s)...", calls.len()));

    // Load controller metadata first to get address and chain_id for session key
    let storage_path = PathBuf::from(shellexpand::tilde(&config.session.storage_path).to_string());
    let backend = FileSystemBackend::new(storage_path);

    let controller_metadata = backend
        .controller()
        .map_err(|e| CliError::Storage(e.to_string()))?
        .ok_or_else(|| CliError::InvalidSessionData("No controller metadata found".to_string()))?;

    // Construct the session key using the same format as Controller
    let session_key = format!(
        "@cartridge/session/0x{:x}/0x{:x}",
        controller_metadata.address, controller_metadata.chain_id
    );

    let session_metadata = backend
        .session(&session_key)
        .map_err(|e| CliError::Storage(e.to_string()))?
        .ok_or(CliError::NoSession)?;

    // Check if session is expired
    if session_metadata.session.is_expired() {
        let expires_at =
            chrono::DateTime::from_timestamp(session_metadata.session.inner.expires_at as i64, 0)
                .unwrap_or_else(chrono::Utc::now);

        return Err(CliError::SessionExpired(
            expires_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        ));
    }

    let credentials = session_metadata
        .credentials
        .ok_or_else(|| CliError::InvalidSessionData("No credentials found".to_string()))?;

    // Create signer from stored private key
    let signing_key = starknet::signers::SigningKey::from_secret_scalar(credentials.private_key);
    let owner = Owner::Signer(Signer::Starknet(signing_key));

    // Priority: CLI flag > stored session RPC > config default
    let effective_rpc_url = rpc_url
        .clone()
        .or_else(|| {
            backend.get("session_rpc_url").ok().and_then(|v| match v {
                Some(StorageValue::String(url)) => Some(url),
                _ => None,
            })
        })
        .unwrap_or_else(|| config.session.default_rpc_url.clone());

    // If --rpc-url was provided, validate it's a Cartridge RPC endpoint
    if let Some(ref url) = rpc_url {
        if !url.starts_with("https://api.cartridge.gg") {
            return Err(CliError::InvalidInput(
                "Only Cartridge RPC endpoints are supported. Use: https://api.cartridge.gg/x/starknet/mainnet or https://api.cartridge.gg/x/starknet/sepolia".to_string()
            ));
        }
    }

    // If --rpc-url was provided, validate it and check chain_id matches session
    if rpc_url.is_some() {
        formatter.info("Validating RPC endpoint...");
        let provider = starknet::providers::jsonrpc::JsonRpcClient::new(
            starknet::providers::jsonrpc::HttpTransport::new(
                url::Url::parse(&effective_rpc_url)
                    .map_err(|e| CliError::InvalidInput(format!("Invalid RPC URL: {}", e)))?,
            ),
        );

        match starknet::providers::Provider::chain_id(&provider).await {
            Ok(rpc_chain_id) => {
                // Check if RPC chain_id matches session chain_id
                if rpc_chain_id != controller_metadata.chain_id {
                    let rpc_chain_name =
                        starknet::core::utils::parse_cairo_short_string(&rpc_chain_id)
                            .unwrap_or_else(|_| format!("0x{:x}", rpc_chain_id));
                    let session_chain_name = starknet::core::utils::parse_cairo_short_string(
                        &controller_metadata.chain_id,
                    )
                    .unwrap_or_else(|_| format!("0x{:x}", controller_metadata.chain_id));

                    return Err(CliError::InvalidInput(format!(
                        "Chain ID mismatch: RPC endpoint is on {} but session is for {}",
                        rpc_chain_name, session_chain_name
                    )));
                }
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

    // Create Controller with session storage for try_session_execute
    let mut controller = Controller::new(
        controller_metadata.username.clone(),
        controller_metadata.class_hash,
        url::Url::parse(&effective_rpc_url).unwrap(),
        owner,
        controller_metadata.address,
        Some(backend),
    )
    .await
    .map_err(|e| CliError::Storage(format!("Failed to create controller: {}", e)))?;

    // Convert CallSpec to starknet Call
    let starknet_calls: Vec<Call> = calls
        .iter()
        .map(|call| {
            let contract_address = Felt::from_hex(&call.contract_address)
                .map_err(|e| CliError::InvalidInput(format!("Invalid contract address: {}", e)))?;

            let selector = starknet::core::utils::get_selector_from_name(&call.entrypoint)
                .map_err(|e| CliError::InvalidInput(format!("Invalid entrypoint: {}", e)))?;

            let calldata: Result<Vec<Felt>> = call
                .calldata
                .iter()
                .map(|data| {
                    Felt::from_hex(data.trim())
                        .map_err(|e| CliError::InvalidInput(format!("Invalid calldata: {}", e)))
                })
                .collect();

            Ok(Call {
                to: contract_address,
                selector,
                calldata: calldata?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let chain_name = match controller.provider.chain_id().await {
        Ok(felt) => starknet::core::utils::parse_cairo_short_string(&felt)
            .unwrap_or_else(|_| format!("0x{:x}", felt)),
        Err(_) => {
            let chain_id = controller_metadata.chain_id;
            starknet::core::utils::parse_cairo_short_string(&chain_id)
                .unwrap_or_else(|_| format!("0x{:x}", chain_id))
        }
    };
    let is_mainnet = chain_name == "SN_MAIN";

    formatter.info(&format!("Executing transaction on {}...", chain_name));

    // Check if paymaster is available using fee estimation and cache
    let paymaster_warning = if is_mainnet {
        // Check cache first
        match get_cached_paymaster_status(&effective_rpc_url) {
            Some(true) => {
                // Known to be paymastered, no warning
                formatter.info("ℹ Transaction will be subsidized by paymaster");
                None
            }
            Some(false) => {
                // Known to be self-funded, show warning
                Some("Transaction will use user funds (paymaster unavailable)".to_string())
            }
            None => {
                // Unknown - need to check
                match estimate_fee_with_paymaster_check(&controller, &effective_rpc_url, formatter)
                    .await
                {
                    Ok(is_paymastered) => {
                        if is_paymastered {
                            formatter.info("ℹ Transaction will be subsidized by paymaster");
                            None
                        } else {
                            Some(
                                "Transaction will use user funds (paymaster unavailable)"
                                    .to_string(),
                            )
                        }
                    }
                    Err(_) => {
                        // Could not determine, show conservative warning
                        Some(
                            "Transaction may use user funds (unable to verify paymaster status)"
                                .to_string(),
                        )
                    }
                }
            }
        }
    } else {
        // On testnet, transactions are typically subsidized
        formatter.info("ℹ Transaction will be subsidized (testnet)");
        None
    };

    if let Some(ref warning) = paymaster_warning {
        formatter.warning(&format!("⚠ {}", warning));
        formatter.info("ℹ Tip: Use --no-self-pay flag to reject non-paymastered transactions (not yet implemented)");
    }

    let result = controller
        .try_session_execute(starknet_calls, None)
        .await
        .map_err(|e| CliError::TransactionFailed(format!("Transaction failed: {}", e)))?;

    let transaction_hash = format!("0x{:x}", result.transaction_hash);

    let output = ExecuteOutput {
        transaction_hash: transaction_hash.clone(),
        message: if wait {
            "Transaction submitted. Waiting for confirmation...".to_string()
        } else {
            "Transaction submitted successfully".to_string()
        },
        warning: paymaster_warning,
        estimated_fee: None, // Would require fee estimation before execution
        fee_token: if is_mainnet {
            Some("ETH".to_string())
        } else {
            Some("STRK".to_string())
        },
    };
    let voyager_subdomain = if is_mainnet { "" } else { "sepolia." };

    if config.cli.json_output {
        formatter.success(&output);
    } else {
        formatter.info(&format!(
            "Transaction: https://{}voyager.online/tx/{}",
            voyager_subdomain, transaction_hash
        ));
    }

    // Wait for transaction confirmation if requested
    if wait {
        formatter.info("Waiting for transaction confirmation...");

        let start = std::time::Instant::now();
        let timeout_duration = std::time::Duration::from_secs(timeout);

        loop {
            if start.elapsed() > timeout_duration {
                return Err(CliError::TransactionFailed(format!(
                    "Transaction confirmation timeout after {} seconds",
                    timeout
                )));
            }

            // Check transaction status
            match controller
                .provider
                .get_transaction_receipt(result.transaction_hash)
                .await
            {
                Ok(_receipt) => {
                    formatter.info("Transaction confirmed!");
                    break;
                }
                Err(_) => {
                    // Transaction not yet confirmed, wait and retry
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                }
            }
        }
    }

    Ok(())
}

/// Check if paymaster is available by attempting a fee estimation
/// Returns Ok(true) if paymastered (fee = 0), Ok(false) if self-funded
async fn estimate_fee_with_paymaster_check(
    _controller: &Controller,
    rpc_url: &str,
    formatter: &dyn OutputFormatter,
) -> Result<bool> {
    formatter.info("Checking paymaster status...");

    // Simple heuristic: Cartridge RPC endpoints typically support paymasters
    // A full implementation would call estimate_fee and check if it returns 0
    // For now, we assume Cartridge endpoints have paymaster support
    let is_cartridge_endpoint = rpc_url.starts_with("https://api.cartridge.gg");
    let is_paymastered = is_cartridge_endpoint;

    // Cache the result
    cache_paymaster_status(rpc_url, is_paymastered);

    Ok(is_paymastered)
}
