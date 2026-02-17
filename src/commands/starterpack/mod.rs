pub mod info;
pub mod purchase;
pub mod quote;

use cainome_cairo_serde::{ByteArray, CairoSerde};
use starknet::core::types::{BlockId, BlockTag, Felt, FunctionCall};
use starknet::providers::{jsonrpc::HttpTransport, JsonRpcClient, Provider};

/// Hardcoded starterpack contract address
pub const STARTERPACK_CONTRACT: Felt =
    Felt::from_hex_unchecked("0x3eb03b8f2be0ec2aafd186d72f6d8f3dd320dbc89f2b6802bca7465f6ccaa43");

/// Token info queried on-chain from the ERC20 contract
pub struct TokenInfo {
    pub symbol: String,
    pub decimals: u8,
}

/// Query ERC20 symbol and decimals from the token contract
pub async fn query_token_info(
    provider: &JsonRpcClient<HttpTransport>,
    token_address: Felt,
) -> crate::error::Result<TokenInfo> {
    let symbol = query_token_symbol(provider, token_address).await?;
    let decimals = query_token_decimals(provider, token_address).await?;
    Ok(TokenInfo { symbol, decimals })
}

async fn query_token_symbol(
    provider: &JsonRpcClient<HttpTransport>,
    token_address: Felt,
) -> crate::error::Result<String> {
    let selector = starknet::core::utils::get_selector_from_name("symbol")
        .map_err(|e| crate::error::CliError::InvalidInput(format!("Invalid entrypoint: {e}")))?;

    let result = provider
        .call(
            FunctionCall {
                contract_address: token_address,
                entry_point_selector: selector,
                calldata: vec![],
            },
            BlockId::Tag(BlockTag::Latest),
        )
        .await
        .map_err(|e| {
            crate::error::CliError::TransactionFailed(format!("Failed to query token symbol: {e}"))
        })?;

    // Try ByteArray deserialization first (newer tokens), fall back to short string (felt)
    if let Ok(byte_array) = ByteArray::cairo_deserialize(&result, 0) {
        if let Ok(s) = byte_array.to_string() {
            return Ok(s);
        }
    }

    if let Some(felt) = result.first() {
        if let Ok(s) = starknet::core::utils::parse_cairo_short_string(felt) {
            return Ok(s);
        }
    }

    Ok(format!("0x{token_address:x}"))
}

async fn query_token_decimals(
    provider: &JsonRpcClient<HttpTransport>,
    token_address: Felt,
) -> crate::error::Result<u8> {
    let selector = starknet::core::utils::get_selector_from_name("decimals")
        .map_err(|e| crate::error::CliError::InvalidInput(format!("Invalid entrypoint: {e}")))?;

    let result = provider
        .call(
            FunctionCall {
                contract_address: token_address,
                entry_point_selector: selector,
                calldata: vec![],
            },
            BlockId::Tag(BlockTag::Latest),
        )
        .await
        .map_err(|e| {
            crate::error::CliError::TransactionFailed(format!(
                "Failed to query token decimals: {e}"
            ))
        })?;

    let felt = result.first().ok_or_else(|| {
        crate::error::CliError::InvalidInput("Empty decimals response".to_string())
    })?;

    let bytes = felt.to_bytes_be();
    Ok(bytes[31])
}

pub fn format_token_amount(amount: u128, decimals: u8) -> String {
    let display_decimals = std::cmp::min(decimals as usize, 6);
    let divisor = 10u128.pow(decimals as u32);
    let whole = amount / divisor;
    let remainder = amount % divisor;
    let padded = format!("{:0>width$}", remainder, width = decimals as usize);
    let truncated = &padded[..display_decimals];
    format!("{whole}.{truncated}")
}

/// Parsed starterpack quote result
#[allow(dead_code)]
pub struct StarterpackQuote {
    pub base_price_low: Felt,
    pub base_price_high: Felt,
    pub referral_fee_low: Felt,
    pub referral_fee_high: Felt,
    pub protocol_fee_low: Felt,
    pub protocol_fee_high: Felt,
    pub total_cost_low: Felt,
    pub total_cost_high: Felt,
    pub payment_token: Felt,
}

impl StarterpackQuote {
    /// Parse the raw felt array returned by the quote entrypoint
    pub fn from_felts(result: &[Felt]) -> crate::error::Result<Self> {
        if result.len() < 9 {
            return Err(crate::error::CliError::InvalidInput(format!(
                "Unexpected quote response: expected 9 values, got {}",
                result.len()
            )));
        }
        Ok(Self {
            base_price_low: result[0],
            base_price_high: result[1],
            referral_fee_low: result[2],
            referral_fee_high: result[3],
            protocol_fee_low: result[4],
            protocol_fee_high: result[5],
            total_cost_low: result[6],
            total_cost_high: result[7],
            payment_token: result[8],
        })
    }
}

/// Extract u128 from the low part of a u256 felt pair
pub fn felt_to_u128(felt: Felt) -> u128 {
    let bytes = felt.to_bytes_be();
    u128::from_be_bytes(bytes[16..32].try_into().unwrap())
}

/// Parse a starterpack ID from string (supports decimal and hex)
pub fn parse_starterpack_id(id: &str) -> crate::error::Result<Felt> {
    if id.starts_with("0x") || id.starts_with("0X") {
        Felt::from_hex(id)
    } else {
        Felt::from_dec_str(id)
    }
    .map_err(|e| {
        crate::error::CliError::InvalidInput(format!("Invalid starterpack ID '{id}': {e}"))
    })
}

/// Resolve RPC URL from chain_id, explicit rpc_url, or config
pub fn resolve_rpc_url(
    chain_id: Option<String>,
    rpc_url: Option<String>,
    config: &crate::config::Config,
    formatter: &dyn crate::output::OutputFormatter,
) -> crate::error::Result<String> {
    if let Some(url) = rpc_url {
        return Ok(url);
    }

    if let Some(chain) = chain_id {
        match chain.as_str() {
            "SN_MAIN" => Ok("https://api.cartridge.gg/x/starknet/mainnet".to_string()),
            "SN_SEPOLIA" => Ok("https://api.cartridge.gg/x/starknet/sepolia".to_string()),
            _ => Err(crate::error::CliError::InvalidInput(format!(
                "Unsupported chain ID '{chain}'. Supported chains: SN_MAIN, SN_SEPOLIA"
            ))),
        }
    } else if !config.session.rpc_url.is_empty() {
        Ok(config.session.rpc_url.clone())
    } else {
        formatter.warning("No --chain-id or --rpc-url specified, using SN_SEPOLIA by default");
        Ok("https://api.cartridge.gg/x/starknet/sepolia".to_string())
    }
}
