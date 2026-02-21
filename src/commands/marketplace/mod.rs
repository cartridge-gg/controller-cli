pub mod buy;
pub mod info;

use crate::error::{CliError, Result};
use starknet::core::types::Felt;

/// Marketplace contract address (same on mainnet and sepolia)
pub const MARKETPLACE_CONTRACT: Felt =
    Felt::from_hex_unchecked("0x057b4ca2f7b58e1b940eb89c4376d6e166abc640abf326512b0c77091f3f9652");

/// STRK token address (for reference)
pub const STRK_TOKEN: Felt =
    Felt::from_hex_unchecked("0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d");

/// Encode a u256 value as two felt252 values (low, high)
pub fn encode_u256(value: &str) -> Result<(Felt, Felt)> {
    // Parse as hex or decimal
    let parsed = if value.starts_with("0x") || value.starts_with("0X") {
        primitive_types::U256::from_str_radix(&value[2..], 16)
    } else {
        primitive_types::U256::from_dec_str(value)
    }
    .map_err(|e| CliError::InvalidInput(format!("Invalid u256 value '{}': {}", value, e)))?;

    // Split into low and high u128
    let low = parsed.low_u128();
    let high = (parsed >> 128).low_u128();

    Ok((Felt::from(low), Felt::from(high)))
}

/// Build the calldata for marketplace execute
pub fn build_execute_calldata(
    order_id: u32,
    collection: Felt,
    token_id_low: Felt,
    token_id_high: Felt,
    asset_id_low: Felt,
    asset_id_high: Felt,
    quantity: u128,
    royalties: bool,
    client_fee: u32,
    client_receiver: Felt,
) -> Vec<Felt> {
    vec![
        Felt::from(order_id),
        collection,
        token_id_low,
        token_id_high,
        asset_id_low,
        asset_id_high,
        Felt::from(quantity),
        Felt::from(royalties as u8),
        Felt::from(client_fee),
        client_receiver,
    ]
}

/// Resolve chain_id to an RPC URL, or pass through rpc_url as-is
pub fn resolve_chain_id_to_rpc(
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
                "Unsupported chain ID '{}'. Supported chains: SN_MAIN, SN_SEPOLIA",
                chain
            ))),
        },
        None => Ok(rpc_url),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_u256_small_value() {
        let (low, high) = encode_u256("42").unwrap();
        assert_eq!(low, Felt::from(42u64));
        assert_eq!(high, Felt::ZERO);
    }

    #[test]
    fn test_encode_u256_hex_value() {
        let (low, high) = encode_u256("0x2a").unwrap();
        assert_eq!(low, Felt::from(42u64));
        assert_eq!(high, Felt::ZERO);
    }

    #[test]
    fn test_encode_u256_large_value() {
        // Value that needs high bits: 2^128 + 1
        let (low, high) = encode_u256("340282366920938463463374607431768211457").unwrap();
        assert_eq!(low, Felt::from(1u64));
        assert_eq!(high, Felt::from(1u64));
    }

    #[test]
    fn test_build_execute_calldata() {
        let calldata = build_execute_calldata(
            42,                    // order_id
            Felt::from(0x123u64),  // collection
            Felt::from(1u64),      // token_id_low
            Felt::ZERO,            // token_id_high
            Felt::ZERO,            // asset_id_low
            Felt::ZERO,            // asset_id_high
            1,                     // quantity
            true,                  // royalties
            0,                     // client_fee
            Felt::ZERO,            // client_receiver
        );

        assert_eq!(calldata.len(), 10);
        assert_eq!(calldata[0], Felt::from(42u32));
        assert_eq!(calldata[7], Felt::from(1u8)); // royalties = true
    }

    #[test]
    fn test_resolve_chain_id_mainnet() {
        let result = resolve_chain_id_to_rpc(Some("SN_MAIN".to_string()), None).unwrap();
        assert_eq!(
            result,
            Some("https://api.cartridge.gg/x/starknet/mainnet".to_string())
        );
    }

    #[test]
    fn test_resolve_chain_id_sepolia() {
        let result = resolve_chain_id_to_rpc(Some("SN_SEPOLIA".to_string()), None).unwrap();
        assert_eq!(
            result,
            Some("https://api.cartridge.gg/x/starknet/sepolia".to_string())
        );
    }

    #[test]
    fn test_resolve_chain_id_invalid() {
        let result = resolve_chain_id_to_rpc(Some("INVALID".to_string()), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_rpc_url_passthrough() {
        let result =
            resolve_chain_id_to_rpc(None, Some("https://custom.rpc".to_string())).unwrap();
        assert_eq!(result, Some("https://custom.rpc".to_string()));
    }
}
