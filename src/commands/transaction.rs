use crate::config::Config;
use crate::error::{CliError, Result};
use crate::output::OutputFormatter;
use serde::Serialize;
use starknet::core::types::Felt;
use starknet::providers::{jsonrpc::HttpTransport, JsonRpcClient, Provider};

/// Get transaction status and details
pub async fn execute(
    config: &Config,
    formatter: &dyn OutputFormatter,
    hash: String,
    chain_id: Option<String>,
    rpc_url: Option<String>,
    wait: bool,
    timeout: u64,
) -> Result<()> {
    // Determine RPC URL
    let rpc_url = resolve_rpc_url(chain_id, rpc_url, config)?;

    // Build the provider
    let url = url::Url::parse(&rpc_url)
        .map_err(|e| CliError::InvalidInput(format!("Invalid RPC URL: {}", e)))?;
    let provider = JsonRpcClient::new(HttpTransport::new(url));

    // Validate transaction hash
    let tx_hash = Felt::from_hex(&hash)
        .map_err(|e| CliError::InvalidInput(format!("Invalid transaction hash: {}", e)))?;

    // Wait for confirmation if requested
    if wait {
        formatter.info(&format!(
            "Waiting for transaction {} to be confirmed (timeout: {}s)...",
            hash, timeout
        ));

        let start = std::time::Instant::now();
        let timeout_duration = std::time::Duration::from_secs(timeout);

        loop {
            if start.elapsed() > timeout_duration {
                return Err(CliError::TimeoutError(format!(
                    "Transaction {} not confirmed within {} seconds",
                    hash, timeout
                )));
            }

            match get_transaction(&provider, tx_hash).await {
                Ok(Some(output)) => {
                    formatter.success(&output);
                    return Ok(());
                }
                Ok(None) => {
                    // Transaction not found yet, keep waiting
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    // Single check
    match get_transaction(&provider, tx_hash).await? {
        Some(output) => {
            formatter.success(&output);
            Ok(())
        }
        None => Err(CliError::NotFoundError(format!(
            "Transaction {} not found",
            hash
        ))),
    }
}

async fn get_transaction(
    provider: &JsonRpcClient<HttpTransport>,
    tx_hash: Felt,
) -> Result<Option<TransactionOutput>> {
    // Get transaction by hash
    let tx_result = provider.get_transaction_by_hash(tx_hash).await;

    match tx_result {
        Ok(tx) => {
            let output = match tx {
                starknet::core::types::Transaction::Invoke(invoke) => match invoke {
                    starknet::core::types::InvokeTransaction::V3(invoke_v3) => TransactionOutput {
                        transaction_hash: format!("0x{:x}", tx_hash),
                        r#type: "INVOKE".to_string(),
                        sender_address: Some(format!("0x{:x}", invoke_v3.sender_address)),
                        calldata: invoke_v3
                            .calldata
                            .iter()
                            .map(|f| format!("0x{:x}", f))
                            .collect(),
                        version: "0x3".to_string(),
                        signature: invoke_v3
                            .signature
                            .iter()
                            .map(|f| format!("0x{:x}", f))
                            .collect(),
                        nonce: format!("0x{:x}", invoke_v3.nonce),
                        resource_bounds: Some(ResourceBounds {
                            l1_gas: GasBounds {
                                max_amount: format!(
                                    "0x{:x}",
                                    invoke_v3.resource_bounds.l1_gas.max_amount
                                ),
                                max_price_per_unit: format!(
                                    "0x{:x}",
                                    invoke_v3.resource_bounds.l1_gas.max_price_per_unit
                                ),
                            },
                            l1_data_gas: GasBounds {
                                max_amount: format!(
                                    "0x{:x}",
                                    invoke_v3.resource_bounds.l1_data_gas.max_amount
                                ),
                                max_price_per_unit: format!(
                                    "0x{:x}",
                                    invoke_v3.resource_bounds.l1_data_gas.max_price_per_unit
                                ),
                            },
                            l2_gas: GasBounds {
                                max_amount: format!(
                                    "0x{:x}",
                                    invoke_v3.resource_bounds.l2_gas.max_amount
                                ),
                                max_price_per_unit: format!(
                                    "0x{:x}",
                                    invoke_v3.resource_bounds.l2_gas.max_price_per_unit
                                ),
                            },
                        }),
                        tip: format!("0x{:x}", invoke_v3.tip),
                        paymaster_data: invoke_v3
                            .paymaster_data
                            .iter()
                            .map(|f| format!("0x{:x}", f))
                            .collect(),
                        account_deployment_data: invoke_v3
                            .account_deployment_data
                            .iter()
                            .map(|f| format!("0x{:x}", f))
                            .collect(),
                        nonce_data_availability_mode: format!(
                            "{:?}",
                            invoke_v3.nonce_data_availability_mode
                        ),
                        fee_data_availability_mode: format!(
                            "{:?}",
                            invoke_v3.fee_data_availability_mode
                        ),
                    },
                    _ => TransactionOutput {
                        transaction_hash: format!("0x{:x}", tx_hash),
                        r#type: "INVOKE".to_string(),
                        sender_address: None,
                        calldata: vec![],
                        version: "0x1".to_string(),
                        signature: vec![],
                        nonce: "0x0".to_string(),
                        resource_bounds: None,
                        tip: "0x0".to_string(),
                        paymaster_data: vec![],
                        account_deployment_data: vec![],
                        nonce_data_availability_mode: "L1".to_string(),
                        fee_data_availability_mode: "L1".to_string(),
                    },
                },
                starknet::core::types::Transaction::Declare(_) => TransactionOutput {
                    transaction_hash: format!("0x{:x}", tx_hash),
                    r#type: "DECLARE".to_string(),
                    sender_address: None,
                    calldata: vec![],
                    version: "0x3".to_string(),
                    signature: vec![],
                    nonce: "0x0".to_string(),
                    resource_bounds: None,
                    tip: "0x0".to_string(),
                    paymaster_data: vec![],
                    account_deployment_data: vec![],
                    nonce_data_availability_mode: "L1".to_string(),
                    fee_data_availability_mode: "L1".to_string(),
                },
                starknet::core::types::Transaction::DeployAccount(_) => TransactionOutput {
                    transaction_hash: format!("0x{:x}", tx_hash),
                    r#type: "DEPLOY_ACCOUNT".to_string(),
                    sender_address: None,
                    calldata: vec![],
                    version: "0x3".to_string(),
                    signature: vec![],
                    nonce: "0x0".to_string(),
                    resource_bounds: None,
                    tip: "0x0".to_string(),
                    paymaster_data: vec![],
                    account_deployment_data: vec![],
                    nonce_data_availability_mode: "L1".to_string(),
                    fee_data_availability_mode: "L1".to_string(),
                },
                starknet::core::types::Transaction::L1Handler(_) => TransactionOutput {
                    transaction_hash: format!("0x{:x}", tx_hash),
                    r#type: "L1_HANDLER".to_string(),
                    sender_address: None,
                    calldata: vec![],
                    version: "0x0".to_string(),
                    signature: vec![],
                    nonce: "0x0".to_string(),
                    resource_bounds: None,
                    tip: "0x0".to_string(),
                    paymaster_data: vec![],
                    account_deployment_data: vec![],
                    nonce_data_availability_mode: "L1".to_string(),
                    fee_data_availability_mode: "L1".to_string(),
                },
                starknet::core::types::Transaction::Deploy(_) => TransactionOutput {
                    transaction_hash: format!("0x{:x}", tx_hash),
                    r#type: "DEPLOY".to_string(),
                    sender_address: None,
                    calldata: vec![],
                    version: "0x0".to_string(),
                    signature: vec![],
                    nonce: "0x0".to_string(),
                    resource_bounds: None,
                    tip: "0x0".to_string(),
                    paymaster_data: vec![],
                    account_deployment_data: vec![],
                    nonce_data_availability_mode: "L1".to_string(),
                    fee_data_availability_mode: "L1".to_string(),
                },
            };
            Ok(Some(output))
        }
        Err(starknet::providers::ProviderError::StarknetError(
            starknet::core::types::StarknetError::TransactionHashNotFound,
        )) => Ok(None),
        Err(e) => Err(CliError::ApiError(format!(
            "Failed to get transaction: {}",
            e
        ))),
    }
}

#[derive(Debug, Serialize)]
struct TransactionOutput {
    #[serde(rename = "transaction_hash")]
    transaction_hash: String,
    #[serde(rename = "type")]
    r#type: String,
    #[serde(rename = "sender_address")]
    sender_address: Option<String>,
    calldata: Vec<String>,
    version: String,
    signature: Vec<String>,
    nonce: String,
    #[serde(rename = "resource_bounds")]
    resource_bounds: Option<ResourceBounds>,
    tip: String,
    #[serde(rename = "paymaster_data")]
    paymaster_data: Vec<String>,
    #[serde(rename = "account_deployment_data")]
    account_deployment_data: Vec<String>,
    #[serde(rename = "nonce_data_availability_mode")]
    nonce_data_availability_mode: String,
    #[serde(rename = "fee_data_availability_mode")]
    fee_data_availability_mode: String,
}

#[derive(Debug, Serialize)]
struct ResourceBounds {
    #[serde(rename = "l1_gas")]
    l1_gas: GasBounds,
    #[serde(rename = "l1_data_gas")]
    l1_data_gas: GasBounds,
    #[serde(rename = "l2_gas")]
    l2_gas: GasBounds,
}

#[derive(Debug, Serialize)]
struct GasBounds {
    #[serde(rename = "max_amount")]
    max_amount: String,
    #[serde(rename = "max_price_per_unit")]
    max_price_per_unit: String,
}

/// Resolve RPC URL from chain_id, explicit rpc_url, or config
fn resolve_rpc_url(
    chain_id: Option<String>,
    rpc_url: Option<String>,
    config: &Config,
) -> Result<String> {
    // If explicit RPC URL provided, use it
    if let Some(url) = rpc_url {
        return Ok(url);
    }

    // If chain_id provided, map to known RPC URL
    if let Some(chain) = chain_id {
        match chain.as_str() {
            "SN_MAIN" => Ok("https://api.cartridge.gg/x/starknet/mainnet".to_string()),
            "SN_SEPOLIA" => Ok("https://api.cartridge.gg/x/starknet/sepolia".to_string()),
            _ => Err(CliError::InvalidInput(format!(
                "Unsupported chain ID '{}'. Supported chains: SN_MAIN, SN_SEPOLIA",
                chain
            ))),
        }
    } else {
        // Fall back to config default
        Ok(config.session.default_rpc_url.clone())
    }
}
