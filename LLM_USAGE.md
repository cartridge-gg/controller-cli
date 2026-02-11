# LLM Usage Guide

This guide provides instructions for LLMs (Large Language Models) to install and use the Cartridge Controller CLI for executing Starknet transactions.

## Quick Start: Use the Skill (Recommended)

**The easiest way to use the controller is through the MCP skill:**

```bash
# Install the skill
git clone https://github.com/cartridge-gg/controller-cli.git
ln -s "$(pwd)/controller-cli/.claude/skills/controller-skill" ~/.claude/skills/controller-skill
```

The skill provides 5 tools:
- `controller_generate_keypair` - Generate session keypair
- `controller_status` - Check session status
- `controller_register_session` - Register session (requires human auth)
- `controller_execute` - Execute transactions
- `controller_clear` - Clear session data

**See:** [Skill Documentation](./.claude/skills/controller-skill/README.md)

Once installed, you can simply ask:
- "Check my controller status"
- "Send 100 STRK to 0xabc123"
- "Execute a transaction on Starknet"

---

## Manual Usage (Alternative)

If you prefer to use the CLI directly without the skill, follow the instructions below.

## Installation

Run this single command to install:

```bash
curl -fsSL https://raw.githubusercontent.com/cartridge-gg/controller-cli/main/install.sh | bash
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
  "stored_at": "~/.config/controller-cli",
  "message": "Keypair generated successfully. Use this public key for session registration."
}
```

**Security Note:** The private key is stored locally. Even if compromised, the resulting session is strictly scoped to:
- Only the contracts you authorize (e.g., STRK token at 0x04718f5a...)
- Only the methods you authorize (e.g., `transfer` and `approve`)
- Only until the session expires (typically 7 days)

A leaked session key cannot access arbitrary contracts or call unauthorized methods.

### 2. Check Status

```bash
controller status --json
```

**Status States:**
- `no_session` - No keypair exists
- `keypair_only` - Keypair exists but no registered session
- `active` - Session registered and not expired

**Expected outputs:**

No keypair:
```json
{
  "status": "no_session",
  "session": null,
  "keypair": null
}
```

Keypair only (after `generate-keypair`):
```json
{
  "status": "keypair_only",
  "session": null,
  "keypair": {
    "public_key": "0x...",
    "has_private_key": true
  }
}
```

Active session (after `register-session`):
```json
{
  "status": "active",
  "session": {
    "address": "0x...",
    "chain_id": "SN_SEPOLIA",
    "expires_at": 1735689600,
    "expires_in_seconds": 3600,
    "expires_at_formatted": "2025-01-01 00:00:00 UTC",
    "is_expired": false
  },
  "keypair": {
    "public_key": "0x...",
    "has_private_key": true
  }
}
```

### 3. Register Session

**Requirements:**
- Either a preset name OR a local policy JSON file
- Human user to authorize via browser
- RPC URL to determine which network

#### Option A: Use a Preset (Recommended)

For popular games and applications, use a preset from [cartridge-gg/presets](https://github.com/cartridge-gg/presets):

**With --chain-id (simplest):**
```bash
controller register-session \
  --preset loot-survivor \
  --chain-id SN_MAIN \
  --json
```

**With --rpc-url (explicit):**
```bash
controller register-session \
  --preset loot-survivor \
  --rpc-url https://api.cartridge.gg/x/starknet/mainnet \
  --json
```

The CLI will:
1. Fetch the preset configuration from GitHub
2. Determine the chain (from --chain-id or by querying --rpc-url)
3. Extract chain-specific policies (mainnet contracts vs sepolia contracts)
4. Display a summary of contracts and entrypoints being authorized
5. Validate the RPC endpoint
6. Generate the authorization URL with chain identification

**Expected output (human-readable):**
```
ℹ Fetching preset 'loot-survivor'...
ℹ Determining chain from RPC URL...
ℹ Using policies for chain: SN_MAIN
ℹ Preset loaded: 6 contracts, 16 entrypoints
ℹ Validating RPC endpoint...
ℹ Authorization URL (SN_MAIN):

https://api.cartridge.gg/s/abc123

ℹ Waiting for authorization (timeout: 5 minutes)...
```

**Supported chain IDs:**
- `SN_MAIN` - Starknet Mainnet (auto-maps to https://api.cartridge.gg/x/starknet/mainnet)
- `SN_SEPOLIA` - Starknet Sepolia (auto-maps to https://api.cartridge.gg/x/starknet/sepolia)

**For Cartridge SLOT or custom chains:** Use `--rpc-url` instead of `--chain-id` to specify your Katana endpoint

**Available presets:** loot-survivor, influence, realms, pistols, dope-wars, and more. See https://github.com/cartridge-gg/presets/tree/main/configs

#### Option B: Use a Local Policy File

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
controller register-session \
  --file policy.json \
  --rpc-url https://api.cartridge.gg/x/starknet/sepolia \
  --json
```

**Expected output (human-readable):**
```
ℹ Validating RPC endpoint...
ℹ Authorization URL (SN_SEPOLIA):

https://api.cartridge.gg/s/abc123

ℹ Waiting for authorization (timeout: 5 minutes)...
```

**Expected output (JSON):**
```json
{
  "authorization_url": "https://x.cartridge.gg/session?public_key=0x...&policies=...",
  "short_url": "https://api.cartridge.gg/s/abc123",
  "public_key": "0x...",
  "message": "Open this URL in your browser to authorize the session. Waiting for authorization..."
}
```

**Important:**
1. Display the `short_url` (if present) to the user, otherwise fall back to `authorization_url`
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
  --rpc-url https://api.cartridge.gg/x/starknet/sepolia \
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
controller execute \
  --file calls.json \
  --rpc-url https://api.cartridge.gg/x/starknet/sepolia \
  --json
```

Expected output:
```json
{
  "transaction_hash": "0x...",
  "message": "Transaction submitted successfully"
}
```

**Important - Transaction Explorer Links:**
When displaying transaction hashes to users, **always use Voyager** as the block explorer:
- **Mainnet**: `https://voyager.online/tx/0x...`
- **Sepolia**: `https://sepolia.voyager.online/tx/0x...`

**Never use** Starkscan or other explorers. Voyager is the preferred explorer for Cartridge Controller transactions.

## Network Selection (Critical for LLMs)

**Always be intentional about network selection.** Never rely on defaults.

### When to Set Network

1. **User explicitly mentions network**: "send on mainnet", "deploy to sepolia"
   - Always add `--rpc-url` flag with the appropriate network

2. **Network is ambiguous**: User says "send 10 STRK to 0xabc"
   - First run `controller status --json` to check the current session's `chain_id`
   - Use the same network as the current session, or ask the user which network

3. **Registering a new session**: Always specify the network during registration
   - This determines which network the session will work on

### Network Detection Workflow

```bash
# Step 1: Check current session network
controller status --json
# Parse output: {"session": {"chain_id": "SN_SEPOLIA", ...}}

# Step 2: Execute on the same network or switch explicitly
controller execute \
  --contract 0x... \
  --entrypoint transfer \
  --calldata 0x... \
  --rpc-url https://api.cartridge.gg/x/starknet/sepolia \  # Match session network
  --json
```

### Specifying Network

Both `register-session` and `execute` commands support network selection via `--chain-id` (for presets) or `--rpc-url` (for explicit control).

**Option 1: Use --chain-id (simplest, for presets):**
```bash
controller register-session --preset loot-survivor --chain-id SN_MAIN
```

Supported chain IDs:
- `SN_MAIN` - Auto-maps to `https://api.cartridge.gg/x/starknet/mainnet`
- `SN_SEPOLIA` - Auto-maps to `https://api.cartridge.gg/x/starknet/sepolia`

**Option 2: Use --rpc-url (explicit control):**
```bash
controller register-session --file policy.json \
  --rpc-url https://api.cartridge.gg/x/starknet/mainnet
```

**Important:** Only Cartridge RPC endpoints are supported for mainnet/sepolia. For SLOT or custom chains, use `--rpc-url` with your Katana endpoint.

**Examples:**

Register with chain-id (preset):
```bash
controller register-session --preset loot-survivor --chain-id SN_MAIN --json
```

Register with rpc-url (file):
```bash
controller register-session --file policy.json \
  --rpc-url https://api.cartridge.gg/x/starknet/mainnet \
  --json
```

Execute transaction on mainnet:
```bash
controller execute \
  --contract 0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7 \
  --entrypoint transfer \
  --calldata 0xdeadbeef,0xa,0x0 \
  --rpc-url https://api.cartridge.gg/x/starknet/mainnet \
  --json
```

**User Request Examples:**
- "I want to play Loot Survivor on mainnet" → Use `--preset loot-survivor --chain-id SN_MAIN`
- "Register for this game on sepolia" → Use `--preset <game> --chain-id SN_SEPOLIA`
- "Send 10 STRK to 0xdeadbeef on mainnet" → Add `--rpc-url https://api.cartridge.gg/x/starknet/mainnet`
- "Execute this on sepolia" → Add `--rpc-url https://api.cartridge.gg/x/starknet/sepolia`

**Priority Order:**
1. `--rpc-url` flag (highest priority)
2. Stored session RPC URL (from registration)
3. Config.toml default RPC URL (lowest priority)

**Validation:**
- When `--rpc-url` is provided, the CLI will validate the endpoint by querying its chain_id
- For `execute`, the chain_id must match the session's registered chain_id
- If validation fails, an error message will indicate the issue

### 5. Wait for Confirmation (Optional)

Add `--wait` flag to wait for transaction confirmation:

```bash
controller execute \
  --file calls.json \
  --rpc-url https://api.cartridge.gg/x/starknet/sepolia \
  --wait \
  --json
```

This will poll until the transaction is confirmed (default 300 second timeout).

## Paymaster Control (Fee Payment)

By default, transactions attempt to use the paymaster (subsidized/free execution). If the paymaster is unavailable, the transaction **fails** rather than falling back to user-funded execution.

### Default Behavior

```bash
controller execute \
  --contract 0x... \
  --entrypoint transfer \
  --calldata 0x... \
  --json
```

- Uses paymaster (free execution)
- **Fails if paymaster unavailable** - no automatic fallback
- Error message suggests using `--no-paymaster`

**Error when paymaster fails:**
```json
{
  "error_code": "TransactionFailed",
  "message": "Paymaster execution failed: <error>. Use --no-paymaster to force self-pay"
}
```

### Force Self-Pay (--no-paymaster)

When you want to pay for the transaction yourself:

```bash
controller execute \
  --contract 0x... \
  --entrypoint transfer \
  --calldata 0x... \
  --no-paymaster \
  --json
```

- Bypasses paymaster entirely
- Estimates fee and executes with user funds
- Message: "Executing transaction on SN_SEPOLIA without paymaster..."

**Use cases for --no-paymaster:**
- Paymaster is unavailable but transaction is urgent
- User prefers to pay fees themselves
- Testing self-pay flow

### When to Use Each Mode

| Scenario | Flag | Behavior |
|----------|------|----------|
| Default (recommended) | None | Free via paymaster, fails if unavailable |
| Urgent transaction | `--no-paymaster` | User pays, always works |
| Testing | `--no-paymaster` | Verify self-pay works |

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

### UnsupportedChainId
```json
{
  "error_code": "InvalidInput",
  "message": "Unsupported chain ID 'SLOT'. Supported chains: SN_MAIN, SN_SEPOLIA. For Cartridge SLOT or other chains, use --rpc-url to specify your Katana endpoint."
}
```

**Recovery:** Use `SN_MAIN` or `SN_SEPOLIA` for standard chains, or use `--rpc-url` with a custom RPC endpoint for SLOT/Katana

### PresetNotFound
```json
{
  "error_code": "InvalidInput",
  "message": "Preset 'invalid-game' not found. Check available presets at: https://github.com/cartridge-gg/presets/tree/main/configs"
}
```

**Recovery:** Check available presets at https://github.com/cartridge-gg/presets/tree/main/configs or use `--file` with a local policy file

### PresetChainNotSupported
```json
{
  "error_code": "InvalidInput",
  "message": "Preset 'loot-survivor' does not support chain 'SN_SEPOLIA'. Available chains: SN_MAIN"
}
```

**Recovery:** Use a supported chain for the preset, or create a custom policy file with `--file`

## Complete Example Flow

```bash
# 1. Install
curl -fsSL https://raw.githubusercontent.com/cartridge-gg/controller-cli/main/install.sh | bash

# 2. Check status (no keypair yet)
controller status --json
# Output: {"status": "no_session", "session": null, "keypair": null}

# 3. Generate keypair
controller generate-keypair --json
# Output: {"public_key": "0x123...", ...}

# 4. Check status again (keypair exists, no session)
controller status --json
# Output: {"status": "keypair_only", "session": null, "keypair": {...}}

# 5. Create policy file
cat > policy.json << 'EOF'
{
  "contracts": {
    "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7": {
      "methods": [{"name": "transfer", "entrypoint": "transfer"}]
    }
  }
}
EOF

# 6. Register session using preset (user must authorize in browser)
controller register-session \
  --preset loot-survivor \
  --chain-id SN_MAIN \
  --json
# Output: {"authorization_url": "https://...", "short_url": "https://api.cartridge.gg/s/abc123", ...}
# User opens URL and authorizes
# Output: {"message": "Session registered and stored successfully", "chain_id": "SN_MAIN", ...}

# 7. Check status to see current network
controller status --json
# Output: {"status": "active", "session": {"chain_id": "SN_SEPOLIA", ...}, ...}

# 8. Execute transaction on the same network
controller execute \
  --contract 0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7 \
  --entrypoint transfer \
  --calldata 0xRECIPIENT,0x64,0x0 \
  --rpc-url https://api.cartridge.gg/x/starknet/sepolia \
  --json
# Output: {"transaction_hash": "0x789...", ...}
```

## Best Practices for LLMs

1. **Always use --json flag** for machine-readable output
2. **Parse JSON responses** to extract relevant fields
3. **Handle errors gracefully** by checking error_code and following recovery_hint
4. **Display authorization URLs clearly** when registering sessions
5. **Explain the human authorization step** - don't expect it to happen automatically
6. **Always be intentional with network selection**:
   - For presets, use `--chain-id SN_MAIN` or `--chain-id SN_SEPOLIA` (simplest)
   - For custom configs, use `--rpc-url https://api.cartridge.gg/x/starknet/mainnet` or sepolia
   - When user mentions "mainnet", use `--chain-id SN_MAIN` (presets) or `--rpc-url` (files)
   - When user mentions "sepolia" or "testnet", use `--chain-id SN_SEPOLIA` (presets) or `--rpc-url` (files)
   - If network is ambiguous, check `controller status --json` first to see the current `chain_id`
   - Never rely on config defaults - always be explicit about network intent
7. **Check session status** before executing transactions to verify session exists and is not expired
8. **Prefer presets for known games/apps**:
   - Use `--preset loot-survivor --chain-id SN_MAIN` for simplicity
   - Presets are maintained by project teams and always up-to-date
   - See available presets at https://github.com/cartridge-gg/presets/tree/main/configs
   - Only use `--file` for custom contracts or testing
   - For SLOT or custom chains, use `--preset <name> --rpc-url <katana-endpoint>`
9. **Validate addresses** before including in calldata (must be 32-byte hex with 0x prefix)
10. **Handle BigInt amounts** correctly (split into low/high for u256)
11. **Set appropriate timeouts** for `--wait` flag based on network conditions
12. **Always use Voyager for transaction links** - Format as `https://voyager.online/tx/0x...` (mainnet) or `https://sepolia.voyager.online/tx/0x...` (sepolia). Never use Starkscan.

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

On Sepolia (default):
```bash
controller execute \
  --contract 0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d \
  --entrypoint transfer \
  --calldata 0xRECIPIENT_ADDRESS,0xAMOUNT,0x0 \
  --json
```

On Mainnet:
```bash
controller execute \
  --contract 0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d \
  --entrypoint transfer \
  --calldata 0xRECIPIENT_ADDRESS,0xAMOUNT,0x0 \
  --rpc-url https://api.cartridge.gg/x/starknet/mainnet \
  --json
```

### Approve Token Spending
```bash
controller execute \
  --contract 0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7 \
  --entrypoint approve \
  --calldata 0xSPENDER_ADDRESS,0xAMOUNT,0x0 \
  --rpc-url https://api.cartridge.gg/x/starknet/sepolia \
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
controller execute \
  --file multicall.json \
  --rpc-url https://api.cartridge.gg/x/starknet/sepolia \
  --wait \
  --json
```

## Security Notes

**Session Key Protection:**
- Private keys are stored locally in `~/.config/controller-cli/` with restricted file permissions
- Even if a session key is compromised, damage is limited because:
  - **Contract scoping**: Only authorized contracts can be called (e.g., only STRK token at 0x04718f5a...)
  - **Method scoping**: Only authorized methods can be called (e.g., only `transfer` and `approve`)
  - **Time scoping**: Sessions expire (typically after 7 days) and must be re-authorized

**Authorization Model:**
- Human authorization required for all sessions (cannot be automated)
- Sessions must be registered via browser before use
- Expired sessions automatically rejected

**Best Practices:**
- Use specific policies (authorize only needed contracts/methods)
- Keep sessions short-lived when possible
- Re-authorize sessions when requirements change
- All transactions are automatically subsidized on Sepolia testnet

## Recommended: Use the Skill

For easier integration, use the MCP skill instead of manual CLI commands:
- **Skill Documentation:** [.claude/skills/controller-skill/README.md](./.claude/skills/controller-skill/README.md)
- **Installation:** `ln -s "$(pwd)/controller-cli/.claude/skills/controller-skill" ~/.claude/skills/controller-skill`
- **Benefits:** Structured tools, automatic JSON parsing, better error handling

## Support

- Repository: https://github.com/cartridge-gg/controller-cli
- Issues: https://github.com/cartridge-gg/controller-cli/issues
- Documentation: https://github.com/cartridge-gg/controller-cli
- Skill: [.claude/skills/controller-skill](./.claude/skills/controller-skill)
