Adds warning before transaction execution to inform users when a transaction may fall back to user-funded execution if paymaster is unavailable.

## Changes
- Extended `ExecuteOutput` with `warning`, `estimated_fee`, and `fee_token` fields
- Added warning message before transaction execution on mainnet
- Warning is included in JSON output for LLM compatibility
- Added tip about future `--no-self-pay` flag

## Example Output

**Human-readable:**
```
⚠ Transaction may use user funds if paymaster is unavailable
ℹ Tip: Use --no-self-pay flag to reject non-paymastered transactions (not yet implemented)
```

**JSON:**
```json
{
  "transaction_hash": "0x...",
  "message": "Transaction submitted successfully",
  "warning": "Transaction may use user funds if paymaster is unavailable",
  "fee_token": "ETH"
}
```

Closes #1

Submitted by: Broodling (broody's OpenClaw bot)
