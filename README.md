# Cartridge Controller CLI

Command-line interface for managing Cartridge Controller sessions on Starknet.

## Overview

Enables automated Starknet transaction execution through a human-in-the-loop workflow:

1. **Generate a keypair** — Creates session signing keys
2. **Register a session** — Creates authorization URL, human approves in browser, CLI auto-retrieves credentials
3. **Execute transactions** — Autonomously executes within authorized policies

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

### 1. Generate a Keypair

```bash
controller generate-keypair
```

Creates and stores a new session keypair. The private key is stored locally — even if compromised, the session is scoped to only the authorized contracts, methods, and time window.

### 2. Register a Session

```bash
controller register-session --file policies.json --chain-id SN_MAIN
```

Or use a preset for popular games/apps:

```bash
controller register-session --preset loot-survivor --chain-id SN_MAIN
```

The CLI generates an authorization URL, displays it, then automatically polls until you authorize in the browser and stores the session.

### 3. Execute Transactions

**Single call:**

```bash
controller execute \
  --contract 0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7 \
  --entrypoint transfer \
  --calldata 0xrecipient,0x100,0x0
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

### 4. Check Status

```bash
controller status
```

Returns `no_session`, `keypair_only`, or `active` with expiration details.

### 5. Look Up Usernames / Addresses

```bash
# Resolve usernames to addresses
controller lookup --usernames shinobi,sensei

# Resolve addresses to usernames
controller lookup --addresses 0x123...,0x456...
```

Returns `username:address` pairs. See the [Cartridge Usernames docs](https://docs.cartridge.gg/controller/usernames) for API details.

### 6. Clear Session

```bash
controller clear
```

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
controller status --json
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
default_chain_id = "SN_SEPOLIA"
default_rpc_url = "https://api.cartridge.gg/x/starknet/sepolia"
keychain_url = "https://x.cartridge.gg"
api_url = "https://api.cartridge.gg/query"

[cli]
json_output = false
use_colors = true
callback_timeout_seconds = 300
```

### Environment Variables

| Variable | Description |
|----------|-------------|
| `CARTRIDGE_STORAGE_PATH` | Override storage location |
| `CARTRIDGE_CHAIN_ID` | Default chain ID (`SN_MAIN` or `SN_SEPOLIA`) |
| `CARTRIDGE_RPC_URL` | Default RPC endpoint |
| `CARTRIDGE_API_URL` | Override API endpoint |
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
