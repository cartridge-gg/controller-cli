use crate::config::Config;
use crate::error::{CliError, Result};
use crate::output::OutputFormatter;
use serde::Serialize;
use starknet::core::types::{BlockId, BlockTag, Felt, FunctionCall};
use starknet::providers::{jsonrpc::HttpTransport, JsonRpcClient, Provider};

use super::{
    felt_to_u128, format_token_amount, parse_starterpack_id, query_token_info, resolve_rpc_url,
    StarterpackQuote, STARTERPACK_CONTRACT,
};

#[derive(Serialize)]
struct QuoteOutput {
    starterpack_id: String,
    chain_id: String,
    payment_token: String,
    base_price: String,
    referral_fee: String,
    protocol_fee: String,
    total_cost: String,
}

pub async fn execute(
    config: &Config,
    formatter: &dyn OutputFormatter,
    id: String,
    quantity: u32,
    chain_id: Option<String>,
    rpc_url: Option<String>,
) -> Result<()> {
    let rpc_url = resolve_rpc_url(chain_id, rpc_url, config, formatter)?;

    let url = url::Url::parse(&rpc_url)
        .map_err(|e| CliError::InvalidInput(format!("Invalid RPC URL: {e}")))?;
    let provider = JsonRpcClient::new(HttpTransport::new(url));

    let id_felt = parse_starterpack_id(&id)?;
    let quantity_felt = Felt::from(quantity);

    let selector = starknet::core::utils::get_selector_from_name("quote")
        .map_err(|e| CliError::InvalidInput(format!("Invalid entrypoint: {e}")))?;

    let chain_name = provider
        .chain_id()
        .await
        .map_err(|e| CliError::Network(format!("Failed to get chain ID: {e}")))
        .and_then(|felt| {
            starknet::core::utils::parse_cairo_short_string(&felt)
                .map_err(|e| CliError::InvalidInput(format!("Failed to parse chain ID: {e}")))
        })?;

    formatter.info("Fetching quote...");

    let result = provider
        .call(
            FunctionCall {
                contract_address: STARTERPACK_CONTRACT,
                entry_point_selector: selector,
                calldata: vec![id_felt, quantity_felt, Felt::ZERO],
            },
            BlockId::Tag(BlockTag::Latest),
        )
        .await
        .map_err(|e| CliError::TransactionFailed(format!("Quote call failed: {e}")))?;

    let quote = StarterpackQuote::from_felts(&result)?;

    let token_info = query_token_info(&provider, quote.payment_token).await?;

    let fmt_amount = |low: Felt| -> String {
        let val = felt_to_u128(low);
        format_token_amount(val, token_info.decimals)
    };

    let base_price = fmt_amount(quote.base_price_low);
    let referral_fee = fmt_amount(quote.referral_fee_low);
    let protocol_fee = fmt_amount(quote.protocol_fee_low);
    let total_cost = fmt_amount(quote.total_cost_low);

    if config.cli.json_output {
        formatter.success(&QuoteOutput {
            starterpack_id: id,
            chain_id: chain_name,
            payment_token: format!("0x{:x}", quote.payment_token),
            base_price,
            referral_fee,
            protocol_fee,
            total_cost,
        });
    } else {
        let token_display = format!("{} (0x{:x})", token_info.symbol, quote.payment_token);

        formatter.info(&format!("Starterpack #{id} quote ({chain_name}):"));
        println!("  Token:        {token_display}");
        println!("  Base price:   {base_price} {}", token_info.symbol);
        println!("  Referral fee: {referral_fee} {}", token_info.symbol);
        println!("  Protocol fee: {protocol_fee} {}", token_info.symbol);
        println!("  Total cost:   {total_cost} {}", token_info.symbol);
    }

    Ok(())
}
