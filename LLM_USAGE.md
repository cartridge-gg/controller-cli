# LLM Usage Guide

This guide provides instructions for LLMs (Large Language Models) to install and use the Cartridge Controller CLI for executing Starknet transactions.

## Installation

Run this single command to install:

```bash
curl -fsSL https://raw.githubusercontent.com/broody/controller/main/install.sh | bash
```

If the installation directory is not in PATH, add it:

```bash
export PATH="$PATH:$HOME/.local/bin"
```

Verify installation:

```bash
controller --version
```

## Quick Start Workflow

### 1. Generate Keypair

```bash
controller generate-keypair --json
```

Expected output:
```json
{
  "public_key": "0x...",
  "stored_at": "~/.config/cartridge",
  "message": "Keypair generated successfully. Use this public key for session registration."
}
```

### 2. Check Status

```bash
controller status --json
```

Expected output (no session):
```json
{
  "status": "no_session",
  "session": null,
  "keypair": {
    "public_key": "0x...",
    "has_private_key": true
  }
}
```

### 3. Register Session

**Requirements:**
- A policy JSON file defining allowed contracts and methods
- Human user to authorize via browser

Create a policy file `policy.json`:
```json
{
  "contracts": {
    "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7": {
      "name": "STRK Token",
      "methods": [
        {
          "name": "transfer",
          "entrypoint": "transfer",
          "description": "Transfer STRK tokens"
        }
      ]
    }
  }
}
```

Register the session:
```bash
controller register-session policy.json --json
```

Expected output:
```json
{
  "authorization_url": "https://x.cartridge.gg/session?public_key=0x...&policies=...",
  "public_key": "0x...",
  "message": "Open this URL in your browser to authorize the session. Waiting for authorization..."
}
```

**Important:**
1. Display the `authorization_url` to the user
2. Ask them to open it in their browser and authorize
3. The command will automatically wait and store the session when authorized (up to 6 minutes)

When authorization completes:
```json
{
  "message": "Session registered and stored successfully",
  "public_key": "0x..."
}
```

### 4. Execute Transaction

**Single call:**
```bash
controller execute \
  --contract 0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7 \
  --entrypoint transfer \
  --calldata 0xRECIPIENT_ADDRESS,0xAMOUNT_LOW,0xAMOUNT_HIGH \
  --json
```

**Multiple calls from file:**

Create `calls.json`:
```json
{
  "calls": [
    {
      "contractAddress": "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7",
      "entrypoint": "transfer",
      "calldata": ["0xRECIPIENT", "0x100", "0x0"]
    }
  ]
}
```

Execute:
```bash
controller execute --file calls.json --json
```

Expected output:
```json
{
  "transaction_hash": "0x...",
  "message": "Transaction submitted successfully"
}
```

### 5. Wait for Confirmation (Optional)

Add `--wait` flag to wait for transaction confirmation:

```bash
controller execute --file calls.json --wait --json
```

This will poll until the transaction is confirmed (default 300 second timeout).

## Error Handling

All errors return JSON with this structure:

```json
{
  "status": "error",
  "error_code": "ErrorType",
  "message": "Human-readable description",
  "recovery_hint": "Suggested action"
}
```

Common errors:

### NoSession
```json
{
  "error_code": "NoSession",
  "message": "No keypair found",
  "recovery_hint": "Run 'controller generate-keypair' to create a keypair"
}
```

**Recovery:** Run `controller generate-keypair --json`

### SessionExpired
```json
{
  "error_code": "SessionExpired",
  "message": "Session expired at 2025-01-01 00:00:00 UTC",
  "recovery_hint": "Run 'controller register-session' to create a new session"
}
```

**Recovery:** Run `controller register-session policy.json --json`

### ManualExecutionRequired
```json
{
  "error_code": "ManualExecutionRequired",
  "message": "No authorized session found for this transaction"
}
```

**Recovery:** Register a session with appropriate policies

### CallbackTimeout
```json
{
  "error_code": "CallbackTimeout",
  "message": "Authorization timeout after 360 seconds"
}
```

**Recovery:** Retry `register-session` and ask user to authorize more quickly

## Complete Example Flow

```bash
# 1. Install
curl -fsSL https://raw.githubusercontent.com/broody/controller/main/install.sh | bash

# 2. Check status
controller status --json
# Output: {"status": "no_session", ...}

# 3. Generate keypair
controller generate-keypair --json
# Output: {"public_key": "0x123...", ...}

# 4. Create policy file
cat > policy.json << 'EOF'
{
  "contracts": {
    "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7": {
      "methods": [{"name": "transfer", "entrypoint": "transfer"}]
    }
  }
}
EOF

# 5. Register session (user must authorize in browser)
controller register-session policy.json --json
# Output: {"authorization_url": "https://...", ...}
# User opens URL and authorizes
# Output: {"message": "Session registered and stored successfully", ...}

# 6. Execute transaction
controller execute \
  --contract 0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7 \
  --entrypoint transfer \
  --calldata 0xRECIPIENT,0x64,0x0 \
  --json
# Output: {"transaction_hash": "0x789...", ...}
```

## Best Practices for LLMs

1. **Always use --json flag** for machine-readable output
2. **Parse JSON responses** to extract relevant fields
3. **Handle errors gracefully** by checking error_code and following recovery_hint
4. **Display authorization URLs clearly** when registering sessions
5. **Explain the human authorization step** - don't expect it to happen automatically
6. **Check session status** before executing transactions
7. **Use descriptive policy files** so users understand what they're authorizing
8. **Validate addresses** before including in calldata (must be 32-byte hex with 0x prefix)
9. **Handle BigInt amounts** correctly (split into low/high for u256)
10. **Set appropriate timeouts** for `--wait` flag based on network conditions

## Transaction Amounts (u256 handling)

Starknet uses u256 for large amounts (like token transfers). These must be split into low/high:

```javascript
// For amount = 100 (fits in u128)
"calldata": ["0xrecipient", "0x64", "0x0"]
//                           ^^^^^  ^^^^
//                           low    high

// For large amounts, split manually or use a library
```

## Common Use Cases

### Transfer STRK Tokens
```bash
controller execute \
  --contract 0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d \
  --entrypoint transfer \
  --calldata 0xRECIPIENT_ADDRESS,0xAMOUNT,0x0 \
  --json
```

### Approve Token Spending
```bash
controller execute \
  --contract 0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7 \
  --entrypoint approve \
  --calldata 0xSPENDER_ADDRESS,0xAMOUNT,0x0 \
  --json
```

### Multi-call Transaction
Create `multicall.json`:
```json
{
  "calls": [
    {
      "contractAddress": "0x049d36...",
      "entrypoint": "approve",
      "calldata": ["0xSPENDER", "0xFFFFFFFF", "0xFFFFFFFF"]
    },
    {
      "contractAddress": "0x123abc...",
      "entrypoint": "swap",
      "calldata": ["0x100", "0x0", "0x1"]
    }
  ]
}
```

Execute:
```bash
controller execute --file multicall.json --wait --json
```

## Security Notes

- Private keys stored in `~/.config/cartridge/` with restricted permissions
- Sessions expire and must be renewed
- Policies limit which contracts/methods can be called
- Human authorization required for all sessions (cannot be automated)
- All transactions are automatically subsidized on Sepolia testnet

## Support

- Repository: https://github.com/broody/controller
- Issues: https://github.com/broody/controller/issues
- Documentation: https://github.com/broody/controller
