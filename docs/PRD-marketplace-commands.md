# PRD: Marketplace Commands for Controller CLI

## Overview

Extend the Controller CLI to support Arcade marketplace operations, enabling users to purchase NFTs from listings without ambiguity in calldata formatting.

## Problem Statement

Currently, purchasing from the Arcade marketplace requires:
1. Manually constructing complex calldata with proper type encoding (u256, ContractAddress)
2. Understanding the marketplace contract interface and parameter ordering
3. Building multi-call transactions (approve + execute)
4. Knowing the correct contract addresses per network

This creates friction for CLI users and increases the risk of transaction failures due to malformed calldata.

## Goals

1. **Unambiguous purchases**: `controller marketplace buy` should "just work"
2. **Consistent UX**: Mirror the patterns established by `starterpack` commands
3. **Safety**: Validate session policies before attempting transactions
4. **Discoverability**: Query listings and orders before purchasing

## Non-Goals

- Full marketplace management (listing, offers, intents) - future PRs
- Collection browsing/search - use web UI
- Admin operations (pause, fees, roles)

## Marketplace Contract Interface

From `arcade/contracts/src/systems/marketplace.cairo`:

```cairo
fn execute(
    ref self: ContractState,
    order_id: u32,               // Order identifier
    collection: ContractAddress, // NFT collection address
    token_id: u256,              // Token ID in collection
    asset_id: u256,              // Specific asset (for ERC1155)
    quantity: u128,              // Amount to purchase
    royalties: bool,             // Pay creator royalties
    client_fee: u32,             // Client app fee (basis points)
    client_receiver: ContractAddress, // Client fee recipient
);
```

## Contract Addresses

| Network | Marketplace Contract |
|---------|---------------------|
| Mainnet | `0x057b4ca2f7b58e1b940eb89c4376d6e166abc640abf326512b0c77091f3f9652` |
| Sepolia | `0x057b4ca2f7b58e1b940eb89c4376d6e166abc640abf326512b0c77091f3f9652` |

## Proposed Commands

### `controller marketplace buy`

Purchase an NFT from an existing listing.

```bash
controller marketplace buy \
  --order-id 42 \
  --collection 0x123...abc \
  --token-id 1 \
  [--asset-id 0] \
  [--quantity 1] \
  [--no-royalties] \
  [--chain-id SN_MAIN|SN_SEPOLIA] \
  [--rpc-url URL] \
  [--wait] \
  [--no-paymaster] \
  [--json]
```

**Behavior:**
1. Query order details from Torii/marketplace to get price and currency
2. Validate order is still valid (not expired, not filled)
3. Build approve call for payment token
4. Build execute call with properly formatted calldata
5. Validate session has required policies
6. Execute multicall via Controller

**Required Session Policies:**
- `approve` on payment token (ERC20)
- `execute` on marketplace contract

### `controller marketplace info`

Query order/listing details before purchasing.

```bash
controller marketplace info \
  --order-id 42 \
  --collection 0x123...abc \
  --token-id 1 \
  [--chain-id SN_MAIN|SN_SEPOLIA] \
  [--json]
```

**Output:**
```json
{
  "order_id": 42,
  "collection": "0x123...abc",
  "token_id": "1",
  "price": "1.5",
  "currency": "STRK",
  "currency_address": "0x04718f5a...",
  "seller": "0xabc...def",
  "status": "active",
  "expires_at": "2026-03-01T00:00:00Z",
  "royalties_enabled": true
}
```

### `controller marketplace orders` (Future)

List active orders for a collection.

```bash
controller marketplace orders \
  --collection 0x123...abc \
  [--token-id 1] \
  [--status active|filled|cancelled] \
  [--limit 20] \
  [--json]
```

## Calldata Formatting

### u256 Encoding

u256 values are encoded as two felt252 (low, high):
```rust
fn encode_u256(value: U256) -> Vec<Felt> {
    vec![
        Felt::from(value.low),   // u128 low bits
        Felt::from(value.high),  // u128 high bits
    ]
}
```

### Execute Calldata

```rust
let calldata = vec![
    Felt::from(order_id),        // u32 -> felt
    collection,                   // ContractAddress
    token_id_low,                // u256 low
    token_id_high,               // u256 high
    asset_id_low,                // u256 low (usually 0)
    asset_id_high,               // u256 high (usually 0)
    Felt::from(quantity),        // u128 -> felt
    Felt::from(royalties as u8), // bool -> felt (0 or 1)
    Felt::from(client_fee),      // u32 -> felt (0 for no client fee)
    Felt::ZERO,                  // client_receiver (zero address)
];
```

## TDD Test Specifications

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    // Test: u256 encoding for token IDs
    #[test]
    fn test_encode_u256_small_value() {
        let token_id = U256::from(42u64);
        let encoded = encode_u256(token_id);
        assert_eq!(encoded.len(), 2);
        assert_eq!(encoded[0], Felt::from(42u64));  // low
        assert_eq!(encoded[1], Felt::ZERO);         // high
    }
    
    #[test]
    fn test_encode_u256_large_value() {
        // Value larger than u128::MAX
        let token_id = U256::from_str("0x1ffffffffffffffffffffffffffffffff").unwrap();
        let encoded = encode_u256(token_id);
        assert_eq!(encoded.len(), 2);
        assert_eq!(encoded[0], Felt::from(u128::MAX));  // low saturated
        assert_eq!(encoded[1], Felt::from(1u64));       // high = 1
    }
    
    // Test: Execute calldata building
    #[test]
    fn test_build_execute_calldata() {
        let order_id = 42u32;
        let collection = Felt::from_hex("0x123").unwrap();
        let token_id = U256::from(1u64);
        let quantity = 1u128;
        let royalties = true;
        
        let calldata = build_execute_calldata(
            order_id,
            collection,
            token_id,
            U256::ZERO,  // asset_id
            quantity,
            royalties,
            0,           // client_fee
            Felt::ZERO,  // client_receiver
        );
        
        assert_eq!(calldata.len(), 10);
        assert_eq!(calldata[0], Felt::from(42u32));      // order_id
        assert_eq!(calldata[1], collection);             // collection
        assert_eq!(calldata[2], Felt::from(1u64));       // token_id low
        assert_eq!(calldata[3], Felt::ZERO);             // token_id high
        assert_eq!(calldata[4], Felt::ZERO);             // asset_id low
        assert_eq!(calldata[5], Felt::ZERO);             // asset_id high
        assert_eq!(calldata[6], Felt::from(1u128));      // quantity
        assert_eq!(calldata[7], Felt::from(1u8));        // royalties = true
        assert_eq!(calldata[8], Felt::ZERO);             // client_fee
        assert_eq!(calldata[9], Felt::ZERO);             // client_receiver
    }
    
    // Test: Policy validation
    #[test]
    fn test_validate_policies_missing_approve() {
        let policies = PolicyStorage { contracts: vec![] };
        let payment_token = Felt::from_hex("0x04718f5a...").unwrap();
        
        let result = validate_marketplace_policies(&Some(policies), payment_token);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("approve"));
    }
    
    #[test]
    fn test_validate_policies_missing_execute() {
        let policies = PolicyStorage {
            contracts: vec![(
                "0x04718f5a...".to_string(),
                ContractPolicy { methods: vec![MethodPolicy { entrypoint: "approve".to_string() }] },
            )],
        };
        let payment_token = Felt::from_hex("0x04718f5a...").unwrap();
        
        let result = validate_marketplace_policies(&Some(policies), payment_token);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("execute"));
    }
    
    #[test]
    fn test_validate_policies_complete() {
        let policies = PolicyStorage {
            contracts: vec![
                (
                    "0x04718f5a...".to_string(),
                    ContractPolicy { methods: vec![MethodPolicy { entrypoint: "approve".to_string() }] },
                ),
                (
                    MARKETPLACE_CONTRACT.to_string(),
                    ContractPolicy { methods: vec![MethodPolicy { entrypoint: "execute".to_string() }] },
                ),
            ],
        };
        let payment_token = Felt::from_hex("0x04718f5a...").unwrap();
        
        let result = validate_marketplace_policies(&Some(policies), payment_token);
        assert!(result.is_ok());
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_marketplace_info_valid_order() {
    // Setup: Create a test listing on Sepolia
    // Query: controller marketplace info --order-id 1 --collection 0x... --token-id 1
    // Assert: Returns valid order details
}

#[tokio::test]
async fn test_marketplace_info_invalid_order() {
    // Query non-existent order
    // Assert: Returns appropriate error
}

#[tokio::test]
async fn test_marketplace_buy_insufficient_balance() {
    // Setup: Session with insufficient payment token balance
    // Execute: marketplace buy
    // Assert: Fails with balance error before transaction
}

#[tokio::test]
async fn test_marketplace_buy_expired_order() {
    // Setup: Order that has expired
    // Execute: marketplace buy
    // Assert: Fails with order expired error
}
```

## File Structure

```
src/commands/
├── marketplace/
│   ├── mod.rs           # Module exports, shared utilities
│   ├── buy.rs           # Purchase command implementation
│   ├── info.rs          # Order info query
│   └── types.rs         # Shared types (OrderInfo, etc.)
├── mod.rs               # Add marketplace module
```

## Implementation Plan

### Phase 1: Core Infrastructure
1. Add `marketplace` module scaffold
2. Implement u256 encoding utilities
3. Add marketplace contract addresses to constants

### Phase 2: Info Command
1. Implement order query via Torii GraphQL
2. Parse and display order details
3. Add validity checking

### Phase 3: Buy Command
1. Implement quote/price fetching
2. Build approve + execute multicall
3. Add policy validation
4. Execute transaction
5. Handle wait/receipt

### Phase 4: Polish
1. Add comprehensive error messages
2. Update CLI help text
3. Write documentation
4. Add to LLM_USAGE.md

## Success Metrics

1. **Zero calldata ambiguity**: Users never need to manually encode u256/addresses
2. **< 3 commands to purchase**: Info → Buy → Done
3. **Clear error messages**: Policy issues, balance problems, expired orders

## Security Considerations

1. **Policy validation**: Refuse to execute without proper session policies
2. **Order validation**: Check order validity before building transaction
3. **Slippage protection**: Future - add max price parameter

## Open Questions

1. Should we query Torii or the contract directly for order info?
   - Recommendation: Torii for speed, contract `get_validity` for confirmation
2. Client fee handling - should CLI pass 0 or allow configuration?
   - Recommendation: Default to 0, add optional `--client-fee` flag later

## Appendix: Arcade Marketplace GraphQL

```graphql
query GetOrder($orderId: Int!, $collection: String!, $tokenId: String!) {
  arcadeMarketplaceOrderModels(
    where: {
      order_id: { eq: $orderId }
      collection: { eq: $collection }
      token_id: { eq: $tokenId }
    }
  ) {
    edges {
      node {
        order_id
        offerer
        collection
        token_id
        price
        currency
        quantity
        expiration
        status { value }
        category { value }
      }
    }
  }
}
```
