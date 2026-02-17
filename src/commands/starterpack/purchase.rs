use crate::commands::session::authorize::PolicyStorage;
use crate::config::Config;
use crate::error::{CliError, Result};
use crate::output::OutputFormatter;
use account_sdk::{
    controller::Controller,
    signers::{Owner, Signer},
    storage::{filestorage::FileSystemBackend, StorageBackend, StorageValue},
};
use serde::Serialize;
use starknet::core::types::{BlockId, BlockTag, Call, Felt, FunctionCall};
use starknet::providers::{jsonrpc::HttpTransport, JsonRpcClient, Provider};
use std::path::PathBuf;

use super::{
    felt_to_u128, format_token_amount, parse_starterpack_id, query_token_info, StarterpackQuote,
    STARTERPACK_CONTRACT,
};

#[derive(Serialize)]
struct PurchaseOutput {
    transaction_hash: String,
    message: String,
}

#[allow(clippy::too_many_arguments)]
pub async fn execute(
    config: &Config,
    formatter: &dyn OutputFormatter,
    id: String,
    recipient: Option<String>,
    quantity: u32,
    _ui: bool,
    direct: bool,
    chain_id: Option<String>,
    rpc_url: Option<String>,
    wait: bool,
    timeout: u64,
    no_paymaster: bool,
) -> Result<()> {
    if direct {
        return execute_direct(
            config,
            formatter,
            &id,
            recipient,
            quantity,
            chain_id,
            rpc_url,
            wait,
            timeout,
            no_paymaster,
        )
        .await;
    }

    // Default to UI mode
    execute_ui(config, formatter, &id, chain_id, rpc_url).await
}

/// Open the starterpack purchase UI in the browser
async fn execute_ui(
    config: &Config,
    formatter: &dyn OutputFormatter,
    id: &str,
    chain_id: Option<String>,
    rpc_url: Option<String>,
) -> Result<()> {
    // Determine chain_id string for URL
    let chain_id_str = resolve_chain_id_string(chain_id, rpc_url, config, formatter).await?;

    let url = format!("https://x.cartridge.gg/starterpack/{id}/{chain_id_str}");

    formatter.info("Opening starterpack purchase page...");

    match webbrowser::open(&url) {
        Ok(()) => {
            formatter.info(&format!("Purchase URL: {url}"));
        }
        Err(e) => {
            formatter.warning(&format!(
                "Could not open browser automatically: {e}. Please open the URL manually."
            ));
            println!("\n{url}\n");
        }
    }

    Ok(())
}

/// Execute the purchase directly via the Controller wallet
#[allow(clippy::too_many_arguments)]
async fn execute_direct(
    config: &Config,
    formatter: &dyn OutputFormatter,
    id: &str,
    recipient: Option<String>,
    quantity: u32,
    chain_id: Option<String>,
    rpc_url: Option<String>,
    wait: bool,
    timeout: u64,
    no_paymaster: bool,
) -> Result<()> {
    let id_felt = parse_starterpack_id(id)?;
    let quantity_felt = Felt::from(quantity);

    // Resolve --chain-id to RPC URL
    let rpc_url = resolve_chain_id_to_rpc(chain_id, rpc_url)?;

    // Load controller metadata
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

    // Resolve recipient: explicit flag or default to controller address
    let recipient_felt = match recipient {
        Some(addr) => Felt::from_hex(&addr)
            .map_err(|e| CliError::InvalidInput(format!("Invalid recipient address: {e}")))?,
        None => controller_metadata.address,
    };

    let session_key = format!(
        "@cartridge/session/0x{:x}/0x{:x}",
        controller_metadata.address, controller_metadata.chain_id
    );

    let session_metadata = backend
        .session(&session_key)
        .map_err(|e| CliError::Storage(e.to_string()))?
        .ok_or(CliError::NoSession)?;

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

    let signing_key = starknet::signers::SigningKey::from_secret_scalar(credentials.private_key);
    let owner = Owner::Signer(Signer::Starknet(signing_key));

    // Resolve effective RPC URL: CLI flag > config (explicit) > stored session > config default
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

    // Validate Cartridge RPC endpoint
    if let Some(ref url) = rpc_url {
        if !url.starts_with("https://api.cartridge.gg") {
            return Err(CliError::InvalidInput(
                "Only Cartridge RPC endpoints are supported. Use: https://api.cartridge.gg/x/starknet/mainnet or https://api.cartridge.gg/x/starknet/sepolia".to_string()
            ));
        }
    }

    let rpc_parsed = url::Url::parse(&effective_rpc_url)
        .map_err(|e| CliError::InvalidInput(format!("Invalid RPC URL: {e}")))?;

    // First, get the quote to know the payment token and amount
    let provider = JsonRpcClient::new(HttpTransport::new(rpc_parsed.clone()));

    formatter.info("Fetching quote...");

    let quote_selector = starknet::core::utils::get_selector_from_name("quote")
        .map_err(|e| CliError::InvalidInput(format!("Invalid entrypoint: {e}")))?;

    let quote_result = provider
        .call(
            FunctionCall {
                contract_address: STARTERPACK_CONTRACT,
                entry_point_selector: quote_selector,
                calldata: vec![id_felt, quantity_felt, Felt::ZERO],
            },
            BlockId::Tag(BlockTag::Latest),
        )
        .await
        .map_err(|e| CliError::TransactionFailed(format!("Quote call failed: {e}")))?;

    let quote = StarterpackQuote::from_felts(&quote_result)?;

    // Display quote info
    let total_cost_val = felt_to_u128(quote.total_cost_low);
    let token_info = query_token_info(&provider, quote.payment_token).await?;
    let amount_display = format!(
        "{} {}",
        format_token_amount(total_cost_val, token_info.decimals),
        token_info.symbol
    );
    formatter.info(&format!("Total cost: {amount_display}"));

    // Check session policies for required entrypoints
    let stored_policies: Option<PolicyStorage> = backend
        .get("session_policies")
        .ok()
        .flatten()
        .and_then(|v| match v {
            StorageValue::String(json) => serde_json::from_str(&json).ok(),
            _ => None,
        });

    validate_purchase_policies(&stored_policies, quote.payment_token)?;

    // Build multicall: approve + issue
    let approve_selector = starknet::core::utils::get_selector_from_name("approve")
        .map_err(|e| CliError::InvalidInput(format!("Invalid entrypoint: {e}")))?;
    let issue_selector = starknet::core::utils::get_selector_from_name("issue")
        .map_err(|e| CliError::InvalidInput(format!("Invalid entrypoint: {e}")))?;

    let calls = vec![
        // approve(spender, total_cost)
        Call {
            to: quote.payment_token,
            selector: approve_selector,
            calldata: vec![
                STARTERPACK_CONTRACT,
                quote.total_cost_low,
                quote.total_cost_high,
            ],
        },
        // issue(recipient, starterpack_id, quantity, referrer=None, referrer_group=None)
        Call {
            to: STARTERPACK_CONTRACT,
            selector: issue_selector,
            calldata: vec![
                recipient_felt,
                id_felt,
                quantity_felt,
                Felt::ONE, // Option::None for referrer
                Felt::ONE, // Option::None for referrer_group
            ],
        },
    ];

    // Create controller
    let mut controller = Controller::new(
        controller_metadata.username.clone(),
        controller_metadata.class_hash,
        rpc_parsed,
        owner,
        controller_metadata.address,
        Some(backend),
    )
    .await
    .map_err(|e| CliError::Storage(format!("Failed to create controller: {e}")))?;

    let chain_name = match controller.provider.chain_id().await {
        Ok(felt) => starknet::core::utils::parse_cairo_short_string(&felt)
            .unwrap_or_else(|_| format!("0x{felt:x}")),
        Err(_) => starknet::core::utils::parse_cairo_short_string(&controller_metadata.chain_id)
            .unwrap_or_else(|_| format!("0x{:x}", controller_metadata.chain_id)),
    };
    let is_mainnet = chain_name == "SN_MAIN";

    // Execute
    let result = if no_paymaster {
        formatter.info(&format!(
            "Purchasing starterpack #{id} on {chain_name} without paymaster..."
        ));
        let estimate = controller
            .estimate_invoke_fee(calls.clone())
            .await
            .map_err(|e| CliError::TransactionFailed(format!("Fee estimation failed: {e}")))?;
        controller
            .execute(calls, Some(estimate), None)
            .await
            .map_err(|e| CliError::TransactionFailed(format!("Transaction failed: {e}")))?
    } else {
        formatter.info(&format!("Purchasing starterpack #{id} on {chain_name}..."));
        controller
            .execute_from_outside_v3(calls, None)
            .await
            .map_err(|e| {
                CliError::TransactionFailed(format!(
                    "Paymaster execution failed: {e}\nUse --no-paymaster to force self-pay"
                ))
            })?
    };

    let transaction_hash = format!("0x{:x}", result.transaction_hash);
    let voyager_subdomain = if is_mainnet { "" } else { "sepolia." };

    if config.cli.json_output {
        formatter.success(&PurchaseOutput {
            transaction_hash: transaction_hash.clone(),
            message: "Starterpack purchased successfully".to_string(),
        });
    } else {
        formatter.info(&format!(
            "Transaction: https://{voyager_subdomain}voyager.online/tx/{transaction_hash}"
        ));
    }

    // Wait for confirmation if requested
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

            match controller
                .provider
                .get_transaction_receipt(result.transaction_hash)
                .await
            {
                Ok(_) => {
                    formatter.info("Transaction confirmed!");
                    break;
                }
                Err(_) => {
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                }
            }
        }
    }

    Ok(())
}

/// Validate that the session policies include `approve` on the payment token
/// and `issue` on the starterpack contract. Returns an error if any are missing.
fn validate_purchase_policies(policies: &Option<PolicyStorage>, payment_token: Felt) -> Result<()> {
    let mut missing = Vec::new();

    match policies {
        None => {
            missing.push(format!("approve on payment token (0x{payment_token:x})"));
            missing.push(format!(
                "issue on starterpack contract (0x{STARTERPACK_CONTRACT:x})"
            ));
        }
        Some(policies) => {
            let has_approve = policies.contracts.iter().any(|(addr, policy)| {
                Felt::from_hex(addr).ok() == Some(payment_token)
                    && policy.methods.iter().any(|m| m.entrypoint == "approve")
            });
            if !has_approve {
                missing.push(format!("approve on payment token (0x{payment_token:x})"));
            }

            let has_issue = policies.contracts.iter().any(|(addr, policy)| {
                Felt::from_hex(addr).ok() == Some(STARTERPACK_CONTRACT)
                    && policy.methods.iter().any(|m| m.entrypoint == "issue")
            });
            if !has_issue {
                missing.push(format!(
                    "issue on starterpack contract (0x{STARTERPACK_CONTRACT:x})"
                ));
            }
        }
    }

    if !missing.is_empty() {
        return Err(CliError::InvalidInput(format!(
            "Current session is missing required policies for direct purchase: {}. \
             Register a new session with policies that include these entrypoints.",
            missing.join(", ")
        )));
    }

    Ok(())
}

/// Resolve chain_id to a human-readable string for the UI URL
async fn resolve_chain_id_string(
    chain_id: Option<String>,
    rpc_url: Option<String>,
    config: &Config,
    formatter: &dyn OutputFormatter,
) -> Result<String> {
    if let Some(chain) = chain_id {
        return match chain.as_str() {
            "SN_MAIN" | "SN_SEPOLIA" => Ok(chain),
            _ => Err(CliError::InvalidInput(format!(
                "Unsupported chain ID '{chain}'. Supported chains: SN_MAIN, SN_SEPOLIA"
            ))),
        };
    }

    let rpc_url = if let Some(url) = rpc_url {
        url
    } else if !config.session.rpc_url.is_empty() {
        config.session.rpc_url.clone()
    } else {
        formatter.warning("No --chain-id or --rpc-url specified, using SN_SEPOLIA by default");
        return Ok("SN_SEPOLIA".to_string());
    };

    let url = url::Url::parse(&rpc_url)
        .map_err(|e| CliError::InvalidInput(format!("Invalid RPC URL: {e}")))?;
    let provider = JsonRpcClient::new(HttpTransport::new(url));

    let chain_felt = provider
        .chain_id()
        .await
        .map_err(|e| CliError::Network(format!("Failed to get chain ID: {e}")))?;

    starknet::core::utils::parse_cairo_short_string(&chain_felt)
        .map_err(|e| CliError::InvalidInput(format!("Failed to parse chain ID: {e}")))
}

/// Resolve --chain-id to an RPC URL, or pass through --rpc-url as-is
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
