use crate::config::Config;
use crate::error::{CliError, Result};
use crate::output::OutputFormatter;
use serde::Serialize;
use starknet::core::types::Felt;
use starknet::providers::{jsonrpc::HttpTransport, JsonRpcClient, Provider};

/// Get transaction receipt
pub async fn execute(
    config: &Config,
    formatter: &dyn OutputFormatter,
    hash: String,
    chain_id: Option<String>,
    rpc_url: Option<String>,
    wait: bool,
    timeout: u64,
) -> Result<()> {
    let rpc_url = resolve_rpc_url(chain_id, rpc_url, config, formatter)?;

    let url = url::Url::parse(&rpc_url)
        .map_err(|e| CliError::InvalidInput(format!("Invalid RPC URL: {e}")))?;
    let provider = JsonRpcClient::new(HttpTransport::new(url));

    let tx_hash = Felt::from_hex(&hash)
        .map_err(|e| CliError::InvalidInput(format!("Invalid transaction hash: {e}")))?;

    if wait {
        formatter.info(&format!(
            "Waiting for transaction {hash} receipt (timeout: {timeout}s)..."
        ));

        let start = std::time::Instant::now();
        let timeout_duration = std::time::Duration::from_secs(timeout);

        loop {
            if start.elapsed() > timeout_duration {
                return Err(CliError::TimeoutError(format!(
                    "Transaction {hash} not confirmed within {timeout} seconds"
                )));
            }

            match get_receipt(&provider, tx_hash).await {
                Ok(Some(output)) => {
                    formatter.success(&output);
                    return Ok(());
                }
                Ok(None) => {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    match get_receipt(&provider, tx_hash).await? {
        Some(output) => {
            formatter.success(&output);
            Ok(())
        }
        None => Err(CliError::NotFoundError(format!(
            "Transaction {hash} not found"
        ))),
    }
}

async fn get_receipt(
    provider: &JsonRpcClient<HttpTransport>,
    tx_hash: Felt,
) -> Result<Option<ReceiptOutput>> {
    let result = provider.get_transaction_receipt(tx_hash).await;

    match result {
        Ok(receipt_with_block) => {
            let receipt = &receipt_with_block.receipt;

            let r#type = match receipt {
                starknet::core::types::TransactionReceipt::Invoke(_) => "INVOKE",
                starknet::core::types::TransactionReceipt::Declare(_) => "DECLARE",
                starknet::core::types::TransactionReceipt::Deploy(_) => "DEPLOY",
                starknet::core::types::TransactionReceipt::DeployAccount(_) => "DEPLOY_ACCOUNT",
                starknet::core::types::TransactionReceipt::L1Handler(_) => "L1_HANDLER",
            };

            let actual_fee = {
                let fee = match receipt {
                    starknet::core::types::TransactionReceipt::Invoke(r) => &r.actual_fee,
                    starknet::core::types::TransactionReceipt::Declare(r) => &r.actual_fee,
                    starknet::core::types::TransactionReceipt::Deploy(r) => &r.actual_fee,
                    starknet::core::types::TransactionReceipt::DeployAccount(r) => &r.actual_fee,
                    starknet::core::types::TransactionReceipt::L1Handler(r) => &r.actual_fee,
                };
                FeeOutput {
                    amount: format!("0x{:x}", fee.amount),
                    unit: match fee.unit {
                        starknet::core::types::PriceUnit::Wei => "WEI".to_string(),
                        starknet::core::types::PriceUnit::Fri => "FRI".to_string(),
                    },
                }
            };

            let finality_status = match receipt.finality_status() {
                starknet::core::types::TransactionFinalityStatus::AcceptedOnL2 => "ACCEPTED_ON_L2",
                starknet::core::types::TransactionFinalityStatus::AcceptedOnL1 => "ACCEPTED_ON_L1",
                starknet::core::types::TransactionFinalityStatus::PreConfirmed => "PRE_CONFIRMED",
            }
            .to_string();

            let messages_sent: Vec<MessageOutput> = match receipt {
                starknet::core::types::TransactionReceipt::Invoke(r) => &r.messages_sent,
                starknet::core::types::TransactionReceipt::Declare(r) => &r.messages_sent,
                starknet::core::types::TransactionReceipt::Deploy(r) => &r.messages_sent,
                starknet::core::types::TransactionReceipt::DeployAccount(r) => &r.messages_sent,
                starknet::core::types::TransactionReceipt::L1Handler(r) => &r.messages_sent,
            }
            .iter()
            .map(|m| MessageOutput {
                from_address: format!("0x{:x}", m.from_address),
                to_address: format!("0x{:x}", m.to_address),
                payload: m.payload.iter().map(|f| format!("0x{f:x}")).collect(),
            })
            .collect();

            let events: Vec<EventOutput> = receipt
                .events()
                .iter()
                .map(|e| EventOutput {
                    from_address: format!("0x{:x}", e.from_address),
                    keys: e.keys.iter().map(|f| format!("0x{f:x}")).collect(),
                    data: e.data.iter().map(|f| format!("0x{f:x}")).collect(),
                })
                .collect();

            let execution_resources = {
                let res = match receipt {
                    starknet::core::types::TransactionReceipt::Invoke(r) => &r.execution_resources,
                    starknet::core::types::TransactionReceipt::Declare(r) => &r.execution_resources,
                    starknet::core::types::TransactionReceipt::Deploy(r) => &r.execution_resources,
                    starknet::core::types::TransactionReceipt::DeployAccount(r) => {
                        &r.execution_resources
                    }
                    starknet::core::types::TransactionReceipt::L1Handler(r) => {
                        &r.execution_resources
                    }
                };
                ExecutionResourcesOutput {
                    l1_gas: res.l1_gas,
                    l1_data_gas: res.l1_data_gas,
                    l2_gas: res.l2_gas,
                }
            };

            let execution_status = match receipt.execution_result() {
                starknet::core::types::ExecutionResult::Succeeded => "SUCCEEDED".to_string(),
                starknet::core::types::ExecutionResult::Reverted { reason } => {
                    format!("REVERTED: {reason}")
                }
            };

            let (block_hash, block_number) = match receipt_with_block.block {
                starknet::core::types::ReceiptBlock::Block {
                    block_hash,
                    block_number,
                } => (Some(format!("0x{block_hash:x}")), Some(block_number)),
                starknet::core::types::ReceiptBlock::PreConfirmed { block_number } => {
                    (None, Some(block_number))
                }
            };

            Ok(Some(ReceiptOutput {
                r#type: r#type.to_string(),
                transaction_hash: format!("0x{tx_hash:x}"),
                actual_fee,
                finality_status,
                messages_sent,
                events,
                execution_resources,
                execution_status,
                block_hash,
                block_number,
            }))
        }
        Err(starknet::providers::ProviderError::StarknetError(
            starknet::core::types::StarknetError::TransactionHashNotFound,
        )) => Ok(None),
        Err(e) => Err(CliError::ApiError(format!(
            "Failed to get transaction receipt: {e}"
        ))),
    }
}

#[derive(Debug, Serialize)]
struct ReceiptOutput {
    r#type: String,
    transaction_hash: String,
    actual_fee: FeeOutput,
    finality_status: String,
    messages_sent: Vec<MessageOutput>,
    events: Vec<EventOutput>,
    execution_resources: ExecutionResourcesOutput,
    execution_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    block_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    block_number: Option<u64>,
}

#[derive(Debug, Serialize)]
struct FeeOutput {
    amount: String,
    unit: String,
}

#[derive(Debug, Serialize)]
struct MessageOutput {
    from_address: String,
    to_address: String,
    payload: Vec<String>,
}

#[derive(Debug, Serialize)]
struct EventOutput {
    from_address: String,
    keys: Vec<String>,
    data: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ExecutionResourcesOutput {
    l1_gas: u64,
    l1_data_gas: u64,
    l2_gas: u64,
}

/// Resolve RPC URL from chain_id, explicit rpc_url, or config
fn resolve_rpc_url(
    chain_id: Option<String>,
    rpc_url: Option<String>,
    config: &Config,
    formatter: &dyn OutputFormatter,
) -> Result<String> {
    if let Some(url) = rpc_url {
        return Ok(url);
    }

    if let Some(chain) = chain_id {
        match chain.as_str() {
            "SN_MAIN" => Ok("https://api.cartridge.gg/x/starknet/mainnet".to_string()),
            "SN_SEPOLIA" => Ok("https://api.cartridge.gg/x/starknet/sepolia".to_string()),
            _ => Err(CliError::InvalidInput(format!(
                "Unsupported chain ID '{chain}'. Supported chains: SN_MAIN, SN_SEPOLIA"
            ))),
        }
    } else if !config.session.default_rpc_url.is_empty() {
        Ok(config.session.default_rpc_url.clone())
    } else {
        formatter.warning("No --chain-id or --rpc-url specified, using SN_SEPOLIA by default");
        Ok("https://api.cartridge.gg/x/starknet/sepolia".to_string())
    }
}
