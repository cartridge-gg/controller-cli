use cainome_cairo_serde::{ByteArray, Bytes31, CairoSerde};

use crate::error::{CliError, Result};
use starknet::core::{types::Felt, utils::cairo_short_string_to_felt};

/// Parse a calldata value, handling special prefixes (u256:, str:, bytearray:) and default felt
/// parsing.
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
    } else if let Some(ba_value) = value.strip_prefix("bytearray:") {
        // Parse Cairo ByteArray (multi-felt serialization)
        let byte_array = if ba_value.starts_with('[') && ba_value.ends_with(']') {
            // Raw bytes mode: bytearray:[0xa,0xd,0xff]
            let inner = &ba_value[1..ba_value.len() - 1];
            if inner.is_empty() {
                ByteArray::default()
            } else {
                let bytes: Vec<u8> = inner
                    .split(',')
                    .map(|b| {
                        let b = b.trim();
                        if let Some(hex) = b.strip_prefix("0x").or_else(|| b.strip_prefix("0X")) {
                            u8::from_str_radix(hex, 16).map_err(|e| {
                                CliError::InvalidInput(format!(
                                    "Invalid byte value '{b}' in bytearray: {e}"
                                ))
                            })
                        } else {
                            b.parse::<u8>().map_err(|e| {
                                CliError::InvalidInput(format!(
                                    "Invalid byte value '{b}' in bytearray: {e}"
                                ))
                            })
                        }
                    })
                    .collect::<Result<Vec<u8>>>()?;
                byte_array_from_bytes(&bytes)
                    .map_err(|e| CliError::InvalidInput(format!("Invalid bytearray: {e}")))?
            }
        } else {
            // String mode: bytearray:hello or bytearray:"hello world"
            // Strip surrounding double quotes if present
            let str_value =
                if ba_value.starts_with('"') && ba_value.ends_with('"') && ba_value.len() >= 2 {
                    &ba_value[1..ba_value.len() - 1]
                } else {
                    ba_value
                };
            ByteArray::from_string(str_value)
                .map_err(|e| CliError::InvalidInput(format!("Invalid bytearray string: {e}")))?
        };
        Ok(ByteArray::cairo_serialize(&byte_array))
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

/// Construct a `ByteArray` from raw bytes, chunking into 31-byte segments.
fn byte_array_from_bytes(
    bytes: &[u8],
) -> std::result::Result<ByteArray, cainome_cairo_serde::Error> {
    const MAX_WORD_LEN: usize = 31;
    let chunks: Vec<_> = bytes.chunks(MAX_WORD_LEN).collect();

    let remainder = if !bytes.len().is_multiple_of(MAX_WORD_LEN) {
        chunks.last().copied().map(|last| last.to_vec())
    } else {
        None
    };

    let full_chunks = if remainder.is_some() {
        &chunks[..chunks.len() - 1]
    } else {
        &chunks[..]
    };

    let (pending_word, pending_word_len) = if let Some(r) = remainder {
        let len = r.len();
        (Felt::from_bytes_be_slice(&r), len)
    } else {
        (Felt::ZERO, 0)
    };

    let mut data = Vec::new();
    for chunk in full_chunks {
        data.push(Bytes31::new(Felt::from_bytes_be_slice(chunk))?);
    }

    Ok(ByteArray {
        data,
        pending_word,
        pending_word_len,
    })
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

    #[test]
    fn test_parse_bytearray_short_string() {
        // "hello" is 5 bytes, fits in pending_word (no full chunks)
        let result = parse_calldata_value("bytearray:hello").unwrap();
        // Expected: [data_length=0, pending_word="hello", pending_word_len=5]
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], Felt::from(0_u128)); // data_length = 0
        assert_eq!(result[1], Felt::from_bytes_be_slice(b"hello")); // pending_word
        assert_eq!(result[2], Felt::from(5_u128)); // pending_word_len
    }

    #[test]
    fn test_parse_bytearray_empty_string() {
        let result = parse_calldata_value("bytearray:").unwrap();
        // Expected: [data_length=0, pending_word=0, pending_word_len=0]
        assert_eq!(
            result,
            vec![Felt::from(0_u128), Felt::ZERO, Felt::from(0_u128)]
        );
    }

    #[test]
    fn test_parse_bytearray_long_string() {
        // 35 bytes = 1 full chunk (31 bytes) + 4 bytes pending
        let s = "ABCDEFGHIJKLMNOPQRSTUVWXYZ12345ABCD";
        assert_eq!(s.len(), 35);
        let result = parse_calldata_value(&format!("bytearray:{s}")).unwrap();
        // Expected: [data_length=1, chunk0, pending_word="ABCD", pending_word_len=4]
        assert_eq!(result.len(), 4);
        assert_eq!(result[0], Felt::from(1_u128)); // data_length = 1
        assert_eq!(
            result[1],
            Felt::from_bytes_be_slice(b"ABCDEFGHIJKLMNOPQRSTUVWXYZ12345")
        ); // chunk
        assert_eq!(result[2], Felt::from_bytes_be_slice(b"ABCD")); // pending_word
        assert_eq!(result[3], Felt::from(4_u128)); // pending_word_len
    }

    #[test]
    fn test_parse_bytearray_raw_bytes() {
        // "Hello" = [0x48, 0x65, 0x6c, 0x6c, 0x6f]
        let result = parse_calldata_value("bytearray:[0x48,0x65,0x6c,0x6c,0x6f]").unwrap();
        let expected = parse_calldata_value("bytearray:Hello").unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_bytearray_raw_bytes_empty() {
        let result = parse_calldata_value("bytearray:[]").unwrap();
        assert_eq!(
            result,
            vec![Felt::from(0_u128), Felt::ZERO, Felt::from(0_u128)]
        );
    }

    #[test]
    fn test_parse_bytearray_quoted_string() {
        // bytearray:"hello world" should encode "hello world" (quotes stripped)
        let result = parse_calldata_value("bytearray:\"hello world\"").unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], Felt::from(0_u128)); // data_length = 0
        assert_eq!(result[1], Felt::from_bytes_be_slice(b"hello world")); // pending_word
        assert_eq!(result[2], Felt::from(11_u128)); // pending_word_len
    }

    #[test]
    fn test_parse_bytearray_quoted_empty() {
        let result = parse_calldata_value("bytearray:\"\"").unwrap();
        assert_eq!(
            result,
            vec![Felt::from(0_u128), Felt::ZERO, Felt::from(0_u128)]
        );
    }
}
