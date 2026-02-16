use crate::{
    commands::{calldata::parse_calldata_value, session::authorize::PolicyStorage},
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
    chain_id: Option<String>,
    rpc_url: Option<String>,
    no_paymaster: bool,
) -> Result<()> {
    // Resolve --chain-id to RPC URL
    let rpc_url = resolve_chain_id_to_rpc(chain_id, rpc_url)?;
    // Parse calls from arguments or file
    let calls = if let Some(file_path) = file {
        // Load calls from JSON file
        let file_content = std::fs::read_to_string(&file_path)
            .map_err(|e| CliError::InvalidInput(format!("Failed to read file: {e}")))?;

        let call_file: CallFile = serde_json::from_str(&file_content)
            .map_err(|e| CliError::InvalidInput(format!("Invalid file format: {e}")))?;

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
            "Either --file or all of contract, entrypoint, calldata arguments must be provided"
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
        .ok_or_else(|| {
            CliError::InvalidSessionData(
                "No controller metadata found. Run 'controller session auth' to create a session."
                    .to_string(),
            )
        })?;

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

    // Priority: CLI flag > config > stored session RPC
    let effective_rpc_url = rpc_url
        .clone()
        .or_else(|| {
            if config.session.rpc_url_explicitly_set {
                Some(config.session.rpc_url.clone())
            } else {
                None
            }
        })
        .or_else(|| {
            backend.get("session_rpc_url").ok().and_then(|v| match v {
                Some(StorageValue::String(url)) => Some(url),
                _ => None,
            })
        })
        .unwrap_or_else(|| config.session.rpc_url.clone());

    // Load stored policies for pre-execution validation
    let stored_policies: Option<PolicyStorage> = backend
        .get("session_policies")
        .ok()
        .flatten()
        .and_then(|v| match v {
            StorageValue::String(json) => serde_json::from_str(&json).ok(),
            _ => None,
        });

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
                    .map_err(|e| CliError::InvalidInput(format!("Invalid RPC URL: {e}")))?,
            ),
        );

        match starknet::providers::Provider::chain_id(&provider).await {
            Ok(rpc_chain_id) => {
                // Check if RPC chain_id matches session chain_id
                if rpc_chain_id != controller_metadata.chain_id {
                    let rpc_chain_name =
                        starknet::core::utils::parse_cairo_short_string(&rpc_chain_id)
                            .unwrap_or_else(|_| format!("0x{rpc_chain_id:x}"));
                    let session_chain_name = starknet::core::utils::parse_cairo_short_string(
                        &controller_metadata.chain_id,
                    )
                    .unwrap_or_else(|_| format!("0x{:x}", controller_metadata.chain_id));

                    return Err(CliError::InvalidInput(format!(
                        "Chain ID mismatch: RPC endpoint is on {rpc_chain_name} but session is for {session_chain_name}"
                    )));
                }
                // Validation successful, continue
            }
            Err(e) => {
                return Err(CliError::InvalidInput(format!(
                    "RPC endpoint not responding: {e}"
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
    .map_err(|e| CliError::Storage(format!("Failed to create controller: {e}")))?;

    // Convert CallSpec to starknet Call
    let starknet_calls: Vec<Call> = calls
        .iter()
        .map(|call| {
            let contract_address = Felt::from_hex(&call.contract_address)
                .map_err(|e| CliError::InvalidInput(format!("Invalid contract address: {e}")))?;

            let selector = starknet::core::utils::get_selector_from_name(&call.entrypoint)
                .map_err(|e| CliError::InvalidInput(format!("Invalid entrypoint: {e}")))?;

            let calldata: Vec<Felt> = call
                .calldata
                .iter()
                .map(|data| parse_calldata_value(data.trim()))
                .collect::<Result<Vec<Vec<Felt>>>>()?
                .into_iter()
                .flatten()
                .collect();

            Ok(Call {
                to: contract_address,
                selector,
                calldata,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    // Validate calls against registered session policies
    if let Some(ref policies) = stored_policies {
        validate_calls_against_policies(&calls, policies)?;
    }

    let chain_name = match controller.provider.chain_id().await {
        Ok(felt) => starknet::core::utils::parse_cairo_short_string(&felt)
            .unwrap_or_else(|_| format!("0x{felt:x}")),
        Err(_) => {
            let chain_id = controller_metadata.chain_id;
            starknet::core::utils::parse_cairo_short_string(&chain_id)
                .unwrap_or_else(|_| format!("0x{chain_id:x}"))
        }
    };
    let is_mainnet = chain_name == "SN_MAIN";

    // Execute based on paymaster preference
    let result = if no_paymaster {
        // Force self-pay: estimate fee and execute directly
        formatter.info(&format!(
            "Executing transaction on {chain_name} without paymaster..."
        ));
        let estimate = controller
            .estimate_invoke_fee(starknet_calls.clone())
            .await
            .map_err(|e| CliError::TransactionFailed(format!("Fee estimation failed: {e}")))?;
        controller
            .execute(starknet_calls, Some(estimate), None)
            .await
            .map_err(|e| CliError::TransactionFailed(format!("Transaction failed: {e}")))?
    } else {
        // Try paymaster first, fail if unavailable (no fallback)
        formatter.info(&format!("Executing transaction on {chain_name}..."));
        match controller
            .execute_from_outside_v3(starknet_calls, None)
            .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(CliError::TransactionFailed(format!(
                    "Paymaster execution failed: {e}\nUse --no-paymaster to force self-pay"
                )));
            }
        }
    };

    let transaction_hash = format!("0x{:x}", result.transaction_hash);

    let output = ExecuteOutput {
        transaction_hash: transaction_hash.clone(),
        message: if wait {
            "Transaction submitted. Waiting for confirmation...".to_string()
        } else {
            "Transaction submitted successfully".to_string()
        },
    };
    let voyager_subdomain = if is_mainnet { "" } else { "sepolia." };

    if config.cli.json_output {
        formatter.success(&output);
    } else {
        formatter.info(&format!(
            "Transaction: https://{voyager_subdomain}voyager.online/tx/{transaction_hash}"
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
                    "Transaction confirmation timeout after {timeout} seconds"
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

/// Validates that all calls are permitted by the stored session policies.
/// Checks both contract address (normalized to handle leading zeros) and entrypoint.
fn validate_calls_against_policies(calls: &[CallSpec], policies: &PolicyStorage) -> Result<()> {
    if calls.is_empty() {
        return Err(CliError::InvalidInput(
            "No calls provided to execute.".to_string(),
        ));
    }

    for call in calls {
        // Normalize by parsing as Felt to handle leading zeros (0x06f... == 0x6f...)
        let call_felt = Felt::from_hex(&call.contract_address).ok();
        let matching_contract = policies.contracts.iter().find(|(addr, _)| {
            match (call_felt, Felt::from_hex(addr).ok()) {
                (Some(a), Some(b)) => a == b,
                _ => addr.to_lowercase() == call.contract_address.to_lowercase(),
            }
        });

        match matching_contract {
            None => {
                return Err(CliError::InvalidInput(format!(
                    "Contract {} is not authorized by the current session policies. \
                     Register a new session with policies that include this contract.",
                    call.contract_address
                )));
            }
            Some((_, contract_policy)) => {
                let entrypoint_allowed = contract_policy
                    .methods
                    .iter()
                    .any(|m| m.entrypoint == call.entrypoint);

                if !entrypoint_allowed {
                    let allowed: Vec<&str> = contract_policy
                        .methods
                        .iter()
                        .map(|m| m.entrypoint.as_str())
                        .collect();
                    return Err(CliError::InvalidInput(format!(
                        "Entrypoint '{}' on contract {} is not authorized by the current session. \
                         Allowed entrypoints: [{}]",
                        call.entrypoint,
                        call.contract_address,
                        allowed.join(", ")
                    )));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::session::authorize::{ContractPolicy, MethodPolicy, PolicyStorage};
    use std::collections::HashMap;

    fn make_policies(contracts: Vec<(&str, Vec<&str>)>) -> PolicyStorage {
        let mut map = HashMap::new();
        for (addr, methods) in contracts {
            map.insert(
                addr.to_string(),
                ContractPolicy {
                    name: None,
                    methods: methods
                        .into_iter()
                        .map(|e| MethodPolicy {
                            name: e.to_string(),
                            entrypoint: e.to_string(),
                            description: None,
                            amount: None,
                            authorized: true,
                        })
                        .collect(),
                },
            );
        }
        PolicyStorage { contracts: map }
    }

    fn make_call(contract: &str, entrypoint: &str) -> CallSpec {
        CallSpec {
            contract_address: contract.to_string(),
            entrypoint: entrypoint.to_string(),
            calldata: vec![],
        }
    }

    #[test]
    fn test_allowed_call_passes() {
        let policies = make_policies(vec![(
            "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7",
            vec!["transfer", "approve"],
        )]);
        let calls = vec![make_call(
            "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7",
            "transfer",
        )];
        assert!(validate_calls_against_policies(&calls, &policies).is_ok());
    }

    #[test]
    fn test_unauthorized_contract_rejected() {
        let policies = make_policies(vec![(
            "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7",
            vec!["transfer"],
        )]);
        let calls = vec![make_call("0xdeadbeef", "transfer")];
        let err = validate_calls_against_policies(&calls, &policies).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not authorized"), "got: {}", msg);
        assert!(msg.contains("0xdeadbeef"));
    }

    #[test]
    fn test_unauthorized_entrypoint_rejected() {
        let policies = make_policies(vec![(
            "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7",
            vec!["transfer"],
        )]);
        let calls = vec![make_call(
            "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7",
            "mint",
        )];
        let err = validate_calls_against_policies(&calls, &policies).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("'mint'"), "got: {}", msg);
        assert!(
            msg.contains("Allowed entrypoints: [transfer]"),
            "got: {}",
            msg
        );
    }

    #[test]
    fn test_leading_zero_normalization() {
        // Policy has leading zero, call does not
        let policies = make_policies(vec![(
            "0x06f7c4350d6d5ee926b3ac4fa0c9c351055456e75c92227468d84232fc493a9c",
            vec!["start_game"],
        )]);
        let calls = vec![make_call(
            "0x6f7c4350d6d5ee926b3ac4fa0c9c351055456e75c92227468d84232fc493a9c",
            "start_game",
        )];
        assert!(validate_calls_against_policies(&calls, &policies).is_ok());
    }

    #[test]
    fn test_leading_zero_normalization_reversed() {
        // Policy has no leading zero, call has leading zero
        let policies = make_policies(vec![(
            "0x6f7c4350d6d5ee926b3ac4fa0c9c351055456e75c92227468d84232fc493a9c",
            vec!["start_game"],
        )]);
        let calls = vec![make_call(
            "0x06f7c4350d6d5ee926b3ac4fa0c9c351055456e75c92227468d84232fc493a9c",
            "start_game",
        )];
        assert!(validate_calls_against_policies(&calls, &policies).is_ok());
    }

    #[test]
    fn test_case_insensitive_address_fallback() {
        let policies = make_policies(vec![("0xABCDEF1234567890", vec!["transfer"])]);
        let calls = vec![make_call("0xabcdef1234567890", "transfer")];
        assert!(validate_calls_against_policies(&calls, &policies).is_ok());
    }

    #[test]
    fn test_multiple_contracts_multiple_calls() {
        let policies = make_policies(vec![
            ("0xaaa", vec!["transfer", "approve"]),
            ("0xbbb", vec!["swap"]),
        ]);
        let calls = vec![make_call("0xaaa", "approve"), make_call("0xbbb", "swap")];
        assert!(validate_calls_against_policies(&calls, &policies).is_ok());
    }

    #[test]
    fn test_second_call_fails_validation() {
        let policies = make_policies(vec![("0xaaa", vec!["transfer"]), ("0xbbb", vec!["swap"])]);
        let calls = vec![
            make_call("0xaaa", "transfer"),
            make_call("0xbbb", "mint"), // not allowed
        ];
        let err = validate_calls_against_policies(&calls, &policies).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("'mint'"), "got: {}", msg);
        assert!(msg.contains("0xbbb"), "got: {}", msg);
    }

    #[test]
    fn test_empty_calls_rejected() {
        let policies = make_policies(vec![("0xaaa", vec!["transfer"])]);
        let calls: Vec<CallSpec> = vec![];
        let err = validate_calls_against_policies(&calls, &policies).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("No calls"), "got: {}", msg);
    }
}

/// Resolve --chain-id to an RPC URL, or pass through --rpc-url as-is.
fn resolve_chain_id_to_rpc(
    chain_id: Option<String>,
    rpc_url: Option<String>,
) -> Result<Option<String>> {
    match chain_id {
        Some(chain) => match chain.as_str() {
            "SN_MAIN" => Ok(Some(
                "https://api.cartridge.gg/x/starknet/mainnet".to_string(),
            )),
            "SN_SEPOLIA" => Ok(Some(
                "https://api.cartridge.gg/x/starknet/sepolia".to_string(),
            )),
            _ => Err(CliError::InvalidInput(format!(
                "Unsupported chain ID '{chain}'. Supported chains: SN_MAIN, SN_SEPOLIA"
            ))),
        },
        None => Ok(rpc_url),
    }
}
