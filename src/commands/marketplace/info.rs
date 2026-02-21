use crate::config::Config;
use crate::error::{CliError, Result};
use crate::output::OutputFormatter;
use serde::Serialize;
use starknet::core::types::{BlockId, BlockTag, Felt, FunctionCall};
use starknet::providers::{jsonrpc::HttpTransport, JsonRpcClient, Provider};

use super::{resolve_chain_id_to_rpc, MARKETPLACE_CONTRACT};

#[derive(Serialize)]
pub struct OrderInfo {
    pub order_id: u32,
    pub collection: String,
    pub token_id: String,
    pub is_valid: bool,
    pub validity_reason: String,
}

#[derive(Serialize)]
struct InfoOutput {
    order: OrderInfo,
}

pub async fn execute(
    config: &Config,
    formatter: &dyn OutputFormatter,
    order_id: u32,
    collection: String,
    token_id: String,
    chain_id: Option<String>,
    rpc_url: Option<String>,
) -> Result<()> {
    // Resolve RPC URL
    let rpc_url = resolve_chain_id_to_rpc(chain_id.clone(), rpc_url)?
        .or_else(|| {
            if !config.session.rpc_url.is_empty() {
                Some(config.session.rpc_url.clone())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "https://api.cartridge.gg/x/starknet/sepolia".to_string());

    let url = url::Url::parse(&rpc_url)
        .map_err(|e| CliError::InvalidInput(format!("Invalid RPC URL: {}", e)))?;
    let provider = JsonRpcClient::new(HttpTransport::new(url));

    // Parse collection address
    let collection_felt = Felt::from_hex(&collection)
        .map_err(|e| CliError::InvalidInput(format!("Invalid collection address: {}", e)))?;

    // Parse token_id as u256 (low, high)
    let (token_id_low, token_id_high) = super::encode_u256(&token_id)?;

    formatter.info(&format!(
        "Querying order #{} for collection {} token {}...",
        order_id, collection, token_id
    ));

    // Call get_validity on the marketplace contract
    let selector = starknet::core::utils::get_selector_from_name("get_validity")
        .map_err(|e| CliError::InvalidInput(format!("Invalid entrypoint: {}", e)))?;

    let result = provider
        .call(
            FunctionCall {
                contract_address: MARKETPLACE_CONTRACT,
                entry_point_selector: selector,
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
        .map_err(|e| CliError::TransactionFailed(format!("get_validity call failed: {}", e)))?;

    // Parse result: (bool, felt252)
    let is_valid = result
        .first()
        .map(|f| *f != Felt::ZERO)
        .unwrap_or(false);
    let reason_felt = result.get(1).copied().unwrap_or(Felt::ZERO);
    let validity_reason = starknet::core::utils::parse_cairo_short_string(&reason_felt)
        .unwrap_or_else(|_| format!("0x{:x}", reason_felt));

    let order_info = OrderInfo {
        order_id,
        collection,
        token_id,
        is_valid,
        validity_reason: if is_valid {
            "Order is valid".to_string()
        } else {
            validity_reason
        },
    };

    if config.cli.json_output {
        formatter.success(&InfoOutput { order: order_info });
    } else {
        if is_valid {
            formatter.info(&format!("✅ Order #{} is valid and can be purchased", order_id));
        } else {
            formatter.warning(&format!(
                "❌ Order #{} is not valid: {}",
                order_id, order_info.validity_reason
            ));
        }
    }

    Ok(())
}
