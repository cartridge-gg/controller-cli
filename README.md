# Cartridge Controller CLI

Command-line interface for managing Cartridge Controller sessions on Starknet.

## Overview

Enables automated Starknet transaction execution through a human-in-the-loop workflow:

1. **Authorize a session** — Generates keypair, creates authorization URL, human approves in browser, CLI auto-retrieves credentials
2. **Execute transactions** — Autonomously executes within authorized policies

The human operator maintains full control by authorizing specific contracts and methods through the browser.

**For LLMs/AI Agents:** See [LLM_USAGE.md](LLM_USAGE.md) for a complete integration guide.

## Installation

### Quick Install (Recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/cartridge-gg/controller-cli/main/install.sh | bash
```

Downloads the appropriate binary for your platform (Linux/macOS, x86_64/ARM64) and installs to `~/.local/bin`.

### From Source

```bash
cargo install --git https://github.com/cartridge-gg/controller-cli
```

## Usage

### 1. Authorize a Session

```bash
controller session auth --file policies.json --chain-id SN_MAIN
```

Or use a preset for popular games/apps:

```bash
controller session auth --preset loot-survivor --chain-id SN_MAIN
```

This generates a new keypair, creates an authorization URL, and automatically polls until you authorize in the browser and stores the session.

### 2. Execute Transactions

**Single call (positional args):**

```bash
controller execute \
  0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7 \
  transfer \
  0xrecipient,u256:1000000000000000000
```

**Multiple calls from file:**

```bash
controller execute --file examples/calls.json
```

**Wait for confirmation:**

```bash
controller execute --file calls.json --wait --timeout 300
```

Transactions are auto-subsidized via paymaster when possible. Use `--no-paymaster` to pay with user funds directly.

### 3. Read-Only Calls

```bash
controller call \
  0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7 \
  balance_of \
  0xaddress
```

Use `--block-id` to query at a specific block (`latest`, `pending`, a block number, or block hash).

### Calldata Formats

Calldata values support multiple formats:

| Format | Example | Description |
|--------|---------|-------------|
| Hex | `0x64` | Standard hex felt |
| Decimal | `100` | Decimal felt |
| `u256:` | `u256:1000000000000000000` | Auto-splits into low/high 128-bit felts |
| `str:` | `str:hello` | Cairo short string |

The `u256:` prefix eliminates the need to manually split token amounts into low/high parts.

### 4. Get Transaction Status

```bash
controller transaction 0xTRANSACTION_HASH --chain-id SN_SEPOLIA
```

Add `--wait` to poll until the transaction is confirmed.

### 5. Get Transaction Receipt

```bash
controller receipt 0xTRANSACTION_HASH --chain-id SN_SEPOLIA
```

Returns the full receipt including execution status, fee, events, and messages. Add `--wait` to poll until available.

### 6. Check Balances

```bash
# Query all token balances for the active session account
controller balance

# Query a specific token
controller balance eth
```

Queries ERC20 balances for the active session account. Built-in tokens: ETH, STRK, USDC, USD.e, LORDS, SURVIVOR, WBTC. Custom tokens can be added via `config set token.<SYMBOL> <address>`.

### 7. Look Up Usernames / Addresses

```bash
# Resolve usernames to addresses
controller lookup --usernames shinobi,sensei

# Resolve addresses to usernames
controller lookup --addresses 0x123...,0x456...
```

Returns `username:address` pairs. See the [Cartridge Usernames docs](https://docs.cartridge.gg/controller/usernames) for API details.

### 8. Get Account Username

```bash
controller username
```

Displays the Cartridge username associated with the active session account.

### 9. Session Management

```bash
# Check session status (no_session, keypair_only, or active with expiration)
controller session status

# List all active sessions with pagination
controller session list
controller session list --limit 20 --page 2

# Clear all stored session data
controller session clear
```

### 10. Configuration

```bash
# Set a config value
controller config set rpc-url https://api.cartridge.gg/x/starknet/mainnet

# Get a config value
controller config get rpc-url

# List all config values
controller config list

# Add a custom token for balance tracking
controller config set token.MYTOKEN 0x123...
```

Valid keys: `rpc-url`, `keychain-url`, `api-url`, `storage-path`, `json-output`, `colors`, `callback-timeout`, `token.<symbol>`.

## Session Policies

Policies define which contracts and methods the session can access:

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

Available presets: `loot-survivor`, `influence`, `realms`, `pistols`, `dope-wars`, and [more](https://github.com/cartridge-gg/presets/tree/main/configs).

## JSON Output

All commands support `--json` for machine-readable output:

```bash
controller session status --json
```

```json
{
  "data": {
    "status": "active",
    "session": {
      "address": "0x...",
      "expires_at": 1735689600,
      "expires_in_seconds": 3600,
      "is_expired": false
    },
    "keypair": { "public_key": "0x...", "has_private_key": true }
  },
  "status": "success"
}
```

Errors include `error_code`, `message`, and `recovery_hint` for programmatic handling.

## Configuration

### Config File

`~/.config/controller-cli/config.toml`:

```toml
[session]
storage_path = "~/.config/controller-cli"
rpc_url = "https://api.cartridge.gg/x/starknet/sepolia"
keychain_url = "https://x.cartridge.gg"
api_url = "https://api.cartridge.gg/query"

[cli]
json_output = false
use_colors = true
callback_timeout_seconds = 300

[tokens]
MYTOKEN = "0x123..."
```

### Environment Variables

| Variable | Description |
|----------|-------------|
| `CARTRIDGE_STORAGE_PATH` | Override storage location |
| `CARTRIDGE_RPC_URL` | Default RPC endpoint |
| `CARTRIDGE_JSON_OUTPUT` | Default to JSON output |

## Architecture

Built on [`account_sdk`](https://github.com/cartridge-gg/controller-rs) which provides session management, transaction execution, policy validation, and file-based storage. The CLI is a thin wrapper optimized for automation and scripting.

## Security

- **Scoped sessions** — Limited to authorized contracts, methods, and time window (typically 7 days)
- **Human authorization required** — Every session must be approved via browser
- **Local key storage** — Private keys stored in `~/.config/controller-cli/` with restricted permissions
- **No credential logging** — Sensitive data never written to logs

## License

MIT
