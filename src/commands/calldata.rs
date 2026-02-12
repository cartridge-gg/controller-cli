use crate::error::{CliError, Result};
use starknet::core::{types::Felt, utils::cairo_short_string_to_felt};

/// Parse a calldata value, handling special prefixes (u256:, str:) and default felt parsing.
pub fn parse_calldata_value(value: &str) -> Result<Vec<Felt>> {
    if let Some(u256_str) = value.strip_prefix("u256:") {
        // Parse u256 value and split into low/high felts
        let normalized = if u256_str.starts_with("0X") {
            u256_str.to_lowercase()
        } else {
            u256_str.to_string()
        };

        let felt = normalized
            .parse::<Felt>()
            .map_err(|e| CliError::InvalidInput(format!("Invalid u256 value '{value}': {e}")))?;

        // Split into low and high 128-bit parts
        let felt_bytes = felt.to_bytes_be();
        let high_bytes = &felt_bytes[0..16];
        let low_bytes = &felt_bytes[16..32];

        let low = Felt::from_bytes_be_slice(low_bytes);
        let high = Felt::from_bytes_be_slice(high_bytes);

        Ok(vec![low, high])
    } else if let Some(str_value) = value.strip_prefix("str:") {
        // Parse Cairo short string
        let felt = cairo_short_string_to_felt(str_value)
            .map_err(|e| CliError::InvalidInput(format!("Invalid short string '{value}': {e}")))?;
        Ok(vec![felt])
    } else {
        // Default: parse as felt
        let normalized = if value.starts_with("0X") {
            value.to_lowercase()
        } else {
            value.to_string()
        };
        let felt = normalized
            .parse::<Felt>()
            .map_err(|e| CliError::InvalidInput(format!("Invalid felt value '{value}': {e}")))?;
        Ok(vec![felt])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_felt_hex() {
        let result = parse_calldata_value("0x123").unwrap();
        assert_eq!(result, vec![Felt::from(0x123_u128)]);
    }

    #[test]
    fn test_parse_felt_hex_uppercase() {
        let result = parse_calldata_value("0XABC").unwrap();
        assert_eq!(result, vec![Felt::from(0xABC_u128)]);
    }

    #[test]
    fn test_parse_felt_decimal() {
        let result = parse_calldata_value("1000000000000000000").unwrap();
        assert_eq!(result, vec![Felt::from(1000000000000000000_u128)]);
    }

    #[test]
    fn test_parse_felt_decimal_zero() {
        let result = parse_calldata_value("0").unwrap();
        assert_eq!(result, vec![Felt::from(0_u128)]);
    }

    #[test]
    fn test_parse_felt_hex_large() {
        // 1 STRK = 10^18 = 0xDE0B6B3A7640000
        let result = parse_calldata_value("0xDE0B6B3A7640000").unwrap();
        assert_eq!(result, vec![Felt::from(1000000000000000000_u128)]);
    }

    #[test]
    fn test_parse_felt_invalid_hex() {
        let result = parse_calldata_value("0xGGGG");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_felt_invalid_decimal() {
        let result = parse_calldata_value("not_a_number");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_felt_empty() {
        let result = parse_calldata_value("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_u256_decimal() {
        let result = parse_calldata_value("u256:1000000000000000000").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], Felt::from(1000000000000000000_u128));
        assert_eq!(result[1], Felt::from(0_u128));
    }

    #[test]
    fn test_parse_u256_hex() {
        let result = parse_calldata_value("u256:0xDE0B6B3A7640000").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], Felt::from(0xDE0B6B3A7640000_u128));
        assert_eq!(result[1], Felt::from(0_u128));
    }

    #[test]
    fn test_parse_u256_large() {
        // 2^128 + 1 = 340282366920938463463374607431768211457
        let result = parse_calldata_value("u256:340282366920938463463374607431768211457").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], Felt::from(1_u128));
        assert_eq!(result[1], Felt::from(1_u128));
    }

    #[test]
    fn test_parse_str_short() {
        let result = parse_calldata_value("str:hello").unwrap();
        assert_eq!(result.len(), 1);
        let expected = cairo_short_string_to_felt("hello").unwrap();
        assert_eq!(result[0], expected);
    }

    #[test]
    fn test_parse_str_empty() {
        let result = parse_calldata_value("str:").unwrap();
        assert_eq!(result, vec![Felt::from(0_u128)]);
    }
}
