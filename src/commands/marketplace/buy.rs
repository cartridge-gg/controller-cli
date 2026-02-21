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

use super::{build_execute_calldata, encode_u256, resolve_chain_id_to_rpc, MARKETPLACE_CONTRACT};

#[derive(Serialize)]
struct BuyOutput {
    transaction_hash: String,
    message: String,
}

#[allow(clippy::too_many_arguments)]
pub async fn execute(
    config: &Config,
    formatter: &dyn OutputFormatter,
    order_id: u32,
    collection: String,
    token_id: String,
    asset_id: Option<String>,
    quantity: u128,
    no_royalties: bool,
    chain_id: Option<String>,
    rpc_url: Option<String>,
    wait: bool,
    timeout: u64,
    no_paymaster: bool,
    account: Option<&str>,
) -> Result<()> {
    // Parse addresses and IDs
    let collection_felt = Felt::from_hex(&collection)
        .map_err(|e| CliError::InvalidInput(format!("Invalid collection address: {}", e)))?;
    let (token_id_low, token_id_high) = encode_u256(&token_id)?;
    let (asset_id_low, asset_id_high) = match asset_id {
        Some(ref id) => encode_u256(id)?,
        None => (Felt::ZERO, Felt::ZERO),
    };

    // Resolve RPC URL
    let rpc_url = resolve_chain_id_to_rpc(chain_id, rpc_url.clone())?;

    // Load controller metadata
    let storage_path = config.resolve_storage_path(account);
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

    // Resolve effective RPC URL
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
        .map_err(|e| CliError::InvalidInput(format!("Invalid RPC URL: {}", e)))?;

    let provider = JsonRpcClient::new(HttpTransport::new(rpc_parsed.clone()));

    // First, check order validity
    formatter.info("Checking order validity...");

    let validity_selector = starknet::core::utils::get_selector_from_name("get_validity")
        .map_err(|e| CliError::InvalidInput(format!("Invalid entrypoint: {}", e)))?;

    let validity_result = provider
        .call(
            FunctionCall {
                contract_address: MARKETPLACE_CONTRACT,
                entry_point_selector: validity_selector,
                calldata: vec![
                    Felt::from(order_id),
                    collection_felt,
                    token_id_low,
                    token_id_high,
                ],
            },
            BlockId::Tag(BlockTag::Latest),
        )
        .await
        .map_err(|e| CliError::TransactionFailed(format!("Validity check failed: {}", e)))?;

    let is_valid = validity_result
        .first()
        .map(|f| *f != Felt::ZERO)
        .unwrap_or(false);

    if !is_valid {
        let reason_felt = validity_result.get(1).copied().unwrap_or(Felt::ZERO);
        let reason = starknet::core::utils::parse_cairo_short_string(&reason_felt)
            .unwrap_or_else(|_| format!("0x{:x}", reason_felt));
        return Err(CliError::InvalidInput(format!(
            "Order #{} is not valid: {}",
            order_id, reason
        )));
    }

    formatter.info("Order is valid ✓");

    // TODO: Query order details from Torii to get price and payment token
    // For now, we'll need the user to provide payment token via session policies
    // This is a simplified implementation - production would query Torii

    // Check session policies
    let stored_policies: Option<PolicyStorage> = backend
        .get("session_policies")
        .ok()
        .flatten()
        .and_then(|v| match v {
            StorageValue::String(json) => serde_json::from_str(&json).ok(),
            _ => None,
        });

    validate_marketplace_policies(&stored_policies)?;

    // Build execute call
    let execute_selector = starknet::core::utils::get_selector_from_name("execute")
        .map_err(|e| CliError::InvalidInput(format!("Invalid entrypoint: {}", e)))?;

    let execute_calldata = build_execute_calldata(
        order_id,
        collection_felt,
        token_id_low,
        token_id_high,
        asset_id_low,
        asset_id_high,
        quantity,
        !no_royalties,
        0,          // client_fee = 0
        Felt::ZERO, // client_receiver = zero address
    );

    let calls = vec![Call {
        to: MARKETPLACE_CONTRACT,
        selector: execute_selector,
        calldata: execute_calldata,
    }];

    // Note: In a full implementation, we would also prepend an approve call
    // for the payment token. This requires querying the order to get the
    // price and currency first.

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
    .map_err(|e| CliError::Storage(format!("Failed to create controller: {}", e)))?;

    let chain_name = match controller.provider.chain_id().await {
        Ok(felt) => starknet::core::utils::parse_cairo_short_string(&felt)
            .unwrap_or_else(|_| format!("0x{:x}", felt)),
        Err(_) => starknet::core::utils::parse_cairo_short_string(&controller_metadata.chain_id)
            .unwrap_or_else(|_| format!("0x{:x}", controller_metadata.chain_id)),
    };
    let is_mainnet = chain_name == "SN_MAIN";

    // Execute
    let result = if no_paymaster {
        formatter.info(&format!(
            "Purchasing order #{} on {} without paymaster...",
            order_id, chain_name
        ));
        let estimate = controller
            .estimate_invoke_fee(calls.clone())
            .await
            .map_err(|e| CliError::TransactionFailed(format!("Fee estimation failed: {}", e)))?;
        controller
            .execute(calls, Some(estimate), None)
            .await
            .map_err(|e| CliError::TransactionFailed(format!("Transaction failed: {}", e)))?
    } else {
        formatter.info(&format!(
            "Purchasing order #{} on {}...",
            order_id, chain_name
        ));
        controller
            .execute_from_outside_v3(calls, None)
            .await
            .map_err(|e| {
                CliError::TransactionFailed(format!(
                    "Paymaster execution failed: {}\nUse --no-paymaster to force self-pay",
                    e
                ))
            })?
    };

    let transaction_hash = format!("0x{:x}", result.transaction_hash);
    let voyager_subdomain = if is_mainnet { "" } else { "sepolia." };

    if config.cli.json_output {
        formatter.success(&BuyOutput {
            transaction_hash: transaction_hash.clone(),
            message: "Marketplace purchase executed successfully".to_string(),
        });
    } else {
        formatter.info(&format!(
            "Transaction: https://{}voyager.online/tx/{}",
            voyager_subdomain, transaction_hash
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
                    "Transaction confirmation timeout after {} seconds",
                    timeout
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

/// Validate that the session policies include `execute` on the marketplace contract
fn validate_marketplace_policies(policies: &Option<PolicyStorage>) -> Result<()> {
    let mut missing = Vec::new();

    match policies {
        None => {
            missing.push(format!(
                "execute on marketplace contract (0x{:x})",
                MARKETPLACE_CONTRACT
            ));
        }
        Some(policies) => {
            let has_execute = policies.contracts.iter().any(|(addr, policy)| {
                Felt::from_hex(addr).ok() == Some(MARKETPLACE_CONTRACT)
                    && policy.methods.iter().any(|m| m.entrypoint == "execute")
            });
            if !has_execute {
                missing.push(format!(
                    "execute on marketplace contract (0x{:x})",
                    MARKETPLACE_CONTRACT
                ));
            }
        }
    }

    if !missing.is_empty() {
        return Err(CliError::InvalidInput(format!(
            "Current session is missing required policies for marketplace purchase: {}. \
             Register a new session with policies that include these entrypoints.",
            missing.join(", ")
        )));
    }

    Ok(())
}
