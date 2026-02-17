# LLM Usage Guide

Instructions for LLMs to install and use the Cartridge Controller CLI for executing Starknet transactions.

## Installation

### Step 1: Install the CLI binary

```bash
curl -fsSL https://raw.githubusercontent.com/cartridge-gg/controller-cli/main/install.sh | bash
```

If the installation directory is not in PATH, add it:

```bash
export PATH="$PATH:$HOME/.local/bin"
```

Verify:

```bash
controller --version
```

### Step 2: Install the skill (Recommended)

The skill provides structured tools with automatic JSON parsing and better error handling.

```bash
git clone https://github.com/cartridge-gg/controller-cli.git /tmp/controller-cli && \
  mkdir -p ~/.claude/skills && \
  ln -sf /tmp/controller-cli/.claude/skills/controller-skill ~/.claude/skills/controller-skill
```

Once installed, tools become available:
- `controller_session_auth` - Generate keypair and authorize a new session
- `controller_session_status` - Check session status
- `controller_session_list` - List active sessions
- `controller_session_clear` - Clear session data
- `controller_execute` - Execute transactions
- `controller_call` - Read-only contract calls
- `controller_transaction` - Get transaction status
- `controller_receipt` - Get transaction receipt
- `controller_balance` - Check token balances
- `controller_username` - Get account username
- `controller_lookup` - Look up usernames/addresses
- `controller_config` - Manage CLI configuration

**See:** [Skill Documentation](./.claude/skills/controller-skill/README.md)

---

## Workflow

### 1. Check Status

```bash
controller session status --json
```

**Status states:**
- `no_session` - No keypair exists
- `keypair_only` - Keypair exists but no registered session
- `active` - Session registered and not expired

Active session output:
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

### 2. Authorize Session

**Requirements:** Human user must authorize via browser. Specify either a preset or a local policy file, plus a network.

The `session auth` command combines keypair generation and session registration in a single step.

#### Option A: Use a Preset (Recommended)

For popular games/apps, use a preset from [cartridge-gg/presets](https://github.com/cartridge-gg/presets/tree/main/configs):

```bash
controller session auth \
  --preset loot-survivor \
  --chain-id SN_MAIN \
  --json
```

Available presets: loot-survivor, influence, realms, pistols, dope-wars, and more.

#### Option B: Use a Local Policy File

Create `policy.json`:
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

```bash
controller session auth \
  --file policy.json \
  --rpc-url https://api.cartridge.gg/x/starknet/sepolia \
  --json
```

#### Authorization Flow

JSON output:
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
3. The command waits automatically and stores the session when authorized (up to 6 minutes)

#### Background Execution

The `session auth` command blocks for up to 6 minutes while waiting for the user to authorize in the browser. To avoid blocking your main thread, run it as a background process:

1. Start `session auth` in the background
2. Capture and display the `short_url` to the user immediately (fall back to `authorization_url` if unavailable)
3. Poll the process for completion
4. Once it exits successfully, verify with `controller session status --json`

This keeps the agent responsive to other user requests while waiting for authorization.

### 3. Execute Transaction

**Single call (positional args: contract, entrypoint, calldata):**
```bash
controller execute \
  0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7 \
  transfer \
  0xRECIPIENT_ADDRESS,u256:1000000000000000000 \
  --json
```

**Multiple calls from file (`calls.json`):**
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

```bash
controller execute \
  --file calls.json \
  --json
```

Output:
```json
{
  "transaction_hash": "0x...",
  "message": "Transaction submitted successfully"
}
```

**Transaction Explorer Links:** Always use Voyager:
- **Mainnet:** `https://voyager.online/tx/0x...`
- **Sepolia:** `https://sepolia.voyager.online/tx/0x...`

### 4. Read-Only Call

Execute a read-only call to query contract state without submitting a transaction.

**Single call (positional args: contract, entrypoint, calldata):**
```bash
controller call \
  0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7 \
  balance_of \
  0xADDRESS \
  --chain-id SN_SEPOLIA \
  --json
```

**Query at a specific block:**
```bash
controller call \
  0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7 \
  balance_of \
  0xADDRESS \
  --chain-id SN_SEPOLIA \
  --block-id latest \
  --json
```

**Multiple calls from file:**
```bash
controller call --file calls.json --chain-id SN_SEPOLIA --json
```

**Note:** `call` does not require an active session. It only needs a network (via `--chain-id` or `--rpc-url`).

### 5. Get Transaction Status

Check the status and details of a submitted transaction.

```bash
controller transaction 0xTRANSACTION_HASH \
  --chain-id SN_SEPOLIA \
  --json
```

**Wait for confirmation:**
```bash
controller transaction 0xTRANSACTION_HASH \
  --chain-id SN_SEPOLIA \
  --wait \
  --timeout 300 \
  --json
```

### 6. Get Transaction Receipt

Get the full transaction receipt including execution status, fee, events, and messages.

```bash
controller receipt 0xTRANSACTION_HASH \
  --chain-id SN_SEPOLIA \
  --json
```

**Wait for receipt to be available:**
```bash
controller receipt 0xTRANSACTION_HASH \
  --chain-id SN_SEPOLIA \
  --wait \
  --timeout 300 \
  --json
```

### 7. Check Token Balances

Query ERC20 token balances for the active session account.

```bash
# All non-zero balances
controller balance --json

# Specific token
controller balance eth --json

# Query on mainnet
controller balance --chain-id SN_MAIN --json
```

Built-in tokens: ETH, STRK, USDC, USD.e, LORDS, SURVIVOR, WBTC. Add custom tokens:
```bash
controller config set token.MYTOKEN 0x123...
```

Output:
```json
[
  { "token": "ETH", "balance": "0.500000", "raw": "0x6f05b59d3b20000", "contract": "0x049d36..." },
  { "token": "STRK", "balance": "100.000000", "raw": "0x56bc75e2d63100000", "contract": "0x04718f..." }
]
```

### 8. Get Account Username

Display the Cartridge username for the active session account.

```bash
controller username --json
```

### 9. Look Up Usernames / Addresses

Resolve Cartridge controller usernames to addresses or vice versa:

```bash
# Look up addresses for usernames
controller lookup --usernames shinobi,sensei --json
```

```bash
# Look up usernames for addresses
controller lookup --addresses 0x123...,0x456... --json
```

Output:
```json
{
  "status": "success",
  "data": [
    "shinobi:0x123...",
    "sensei:0x456..."
  ]
}
```

Each entry is a `username:address` pair. You can combine both flags in a single call. See the [Cartridge Usernames API](https://docs.cartridge.gg/controller/usernames) for limits and rate-limiting details.

### 10. Session Management

**List active sessions:**
```bash
controller session list --json
controller session list --limit 20 --page 2 --json
```

**Clear all session data:**
```bash
controller session clear --yes
```

### 11. Configuration

Manage CLI settings without editing the config file directly.

```bash
# Set a value
controller config set rpc-url https://api.cartridge.gg/x/starknet/mainnet

# Get a value
controller config get rpc-url --json

# List all values
controller config list --json
```

Valid keys: `rpc-url`, `keychain-url`, `api-url`, `storage-path`, `json-output`, `colors`, `callback-timeout`, `token.<symbol>`.

---

## Calldata Formats

Calldata values support multiple formats:

| Format | Example | Description |
|--------|---------|-------------|
| Hex | `0x64` | Standard hex felt |
| Decimal | `100` | Decimal felt (auto-converted) |
| `u256:` | `u256:1000000000000000000` | Auto-splits into low/high 128-bit felts |
| `str:` | `str:hello` | Cairo short string encoding |

The `u256:` prefix is the recommended way to specify token amounts. It eliminates manual low/high splitting:

```bash
# Using u256: prefix (recommended)
controller execute 0x04718f... transfer 0xRECIPIENT,u256:1000000000000000000 --json

# Equivalent manual split
controller execute 0x04718f... transfer 0xRECIPIENT,0xDE0B6B3A7640000,0x0 --json
```

---

## Network Selection

**Always be explicit about network.** Never rely on defaults.

### Supported Networks

| Chain ID | RPC URL | Usage |
|----------|---------|-------|
| `SN_MAIN` | `https://api.cartridge.gg/x/starknet/mainnet` | Starknet Mainnet |
| `SN_SEPOLIA` | `https://api.cartridge.gg/x/starknet/sepolia` | Starknet Sepolia |

For SLOT or custom chains, use `--rpc-url` with your Katana endpoint.

### How to Specify Network

- **Session auth:** Use `--chain-id SN_MAIN` or `--chain-id SN_SEPOLIA` (simplest)
- **Execute/call/transaction:** Use `--chain-id` or `--rpc-url` (explicit)

### When Network is Ambiguous

1. Run `controller session status --json` to check the current session's `chain_id`
2. Use the same network, or ask the user

### Priority Order

1. `--chain-id` or `--rpc-url` flag (highest)
2. Explicit config/env (`config set rpc-url` or `CARTRIDGE_RPC_URL`)
3. Stored session RPC URL (from authorization)
4. Default (SN_SEPOLIA)

---

## Paymaster Control

By default, transactions use the paymaster (free execution). If the paymaster is unavailable, the transaction **fails** rather than falling back to user-funded execution.

Use `--no-paymaster` to bypass the paymaster and pay with user funds:

```bash
controller execute \
  0x... \
  transfer \
  0x... \
  --no-paymaster \
  --json
```

| Scenario | Flag | Behavior |
|----------|------|----------|
| Default | None | Free via paymaster, fails if unavailable |
| Urgent / self-pay | `--no-paymaster` | User pays fees directly |

---

## Error Handling

All errors return JSON:

```json
{
  "status": "error",
  "error_code": "ErrorType",
  "message": "Human-readable description",
  "recovery_hint": "Suggested action"
}
```

| Error Code | Cause | Recovery |
|------------|-------|----------|
| `NoSession` | No keypair found | Run `controller session auth --file policy.json --json` |
| `SessionExpired` | Session past expiry | Run `controller session auth` again |
| `ManualExecutionRequired` | No authorized session for this transaction | Authorize session with appropriate policies |
| `CallbackTimeout` | User didn't authorize within 360s | Retry `session auth`, ask user to authorize faster |
| `InvalidInput` (UnsupportedChainId) | Bad chain ID | Use `SN_MAIN` or `SN_SEPOLIA`, or `--rpc-url` for custom chains |
| `InvalidInput` (PresetNotFound) | Unknown preset name | Check [available presets](https://github.com/cartridge-gg/presets/tree/main/configs) |
| `InvalidInput` (PresetChainNotSupported) | Preset doesn't support requested chain | Use a supported chain or create a custom policy file |

---

## Use Cases

The lookup + execute commands combine to enable natural-language workflows. An LLM can resolve usernames to addresses transparently, then build the right transaction.

### Send tokens to a username

> "Send 1 STRK to broody"

1. `controller lookup --usernames broody --json` → resolves to `broody:0xABC...`
2. `controller execute 0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d transfer 0xABC...,u256:1000000000000000000 --json`

### Interact with a game using a player's username

> "Attack loaf's realm at grid 1,2"

1. `controller lookup --usernames loaf --json` → resolves to `loaf:0xDEF...`
2. `controller execute 0xGAME_CONTRACT attack 0xDEF...,0x1,0x2 --json`

### Check who owns an address

> "Who is 0x123...?"

1. `controller lookup --addresses 0x123... --json` → resolves to `shinobi:0x123...`

### Check account balance

> "How much ETH do I have?"

1. `controller balance eth --json` → returns balance with formatted and raw values

---

## Best Practices

1. **Always use `--json` flag** for machine-readable output
2. **Always be explicit about network** - use `--chain-id` or `--rpc-url`
3. **Check session status** before executing to verify session exists and isn't expired
4. **Prefer presets** for known games/apps - they're maintained by project teams
5. **Display authorization URLs clearly** and explain the human authorization step
6. **Handle errors** by checking `error_code` and following `recovery_hint`
7. **Validate addresses** (must be hex with 0x prefix)
8. **Always use Voyager** for transaction links, never Starkscan
9. **Use `u256:` prefix** for token amounts instead of manual low/high splitting
10. **Use `balance` command** instead of raw `call balance_of` for token balance queries

---

## Security

- Private keys stored locally in `~/.config/controller-cli/` with restricted permissions
- Sessions are scoped: only authorized contracts, methods, and time window
- Human browser authorization required for all sessions (cannot be automated)
- Expired sessions are automatically rejected

---

## Support

- Repository: https://github.com/cartridge-gg/controller-cli
- Issues: https://github.com/cartridge-gg/controller-cli/issues
- Skill: [.claude/skills/controller-skill](./.claude/skills/controller-skill)
