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

Once installed, 5 tools become available:
- `controller_generate_keypair` - Generate session keypair
- `controller_status` - Check session status
- `controller_register_session` - Register session (requires human auth)
- `controller_execute` - Execute transactions
- `controller_clear` - Clear session data

**See:** [Skill Documentation](./.claude/skills/controller-skill/README.md)

---

## Workflow

### 1. Generate Keypair

```bash
controller generate-keypair --json
```

Output:
```json
{
  "public_key": "0x...",
  "stored_at": "~/.config/controller-cli",
  "message": "Keypair generated successfully. Use this public key for session registration."
}
```

**Security Note:** The private key is stored locally. Even if compromised, the session is scoped to only the authorized contracts, methods, and expiry window (typically 7 days).

### 2. Check Status

```bash
controller status --json
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

### 3. Register Session

**Requirements:** Human user must authorize via browser. Specify either a preset or a local policy file, plus a network.

#### Option A: Use a Preset (Recommended)

For popular games/apps, use a preset from [cartridge-gg/presets](https://github.com/cartridge-gg/presets/tree/main/configs):

```bash
controller register-session \
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
controller register-session \
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
  --rpc-url https://api.cartridge.gg/x/starknet/sepolia \
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

### 5. Wait for Confirmation (Optional)

Add `--wait` to wait for transaction confirmation (default 300 second timeout):

```bash
controller execute \
  --file calls.json \
  --rpc-url https://api.cartridge.gg/x/starknet/sepolia \
  --wait \
  --json
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

- **Presets:** Use `--chain-id SN_MAIN` or `--chain-id SN_SEPOLIA` (simplest)
- **Policy files / execute:** Use `--rpc-url <url>` (explicit)

### When Network is Ambiguous

1. Run `controller status --json` to check the current session's `chain_id`
2. Use the same network, or ask the user

### Priority Order

1. `--rpc-url` flag (highest)
2. Stored session RPC URL (from registration)
3. Config.toml default (lowest)

---

## Paymaster Control

By default, transactions use the paymaster (free execution). If the paymaster is unavailable, the transaction **fails** rather than falling back to user-funded execution.

Use `--no-paymaster` to bypass the paymaster and pay with user funds:

```bash
controller execute \
  --contract 0x... \
  --entrypoint transfer \
  --calldata 0x... \
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
| `NoSession` | No keypair found | Run `controller generate-keypair --json` |
| `SessionExpired` | Session past expiry | Run `controller register-session` again |
| `ManualExecutionRequired` | No authorized session for this transaction | Register session with appropriate policies |
| `CallbackTimeout` | User didn't authorize within 360s | Retry `register-session`, ask user to authorize faster |
| `InvalidInput` (UnsupportedChainId) | Bad chain ID | Use `SN_MAIN` or `SN_SEPOLIA`, or `--rpc-url` for custom chains |
| `InvalidInput` (PresetNotFound) | Unknown preset name | Check [available presets](https://github.com/cartridge-gg/presets/tree/main/configs) |
| `InvalidInput` (PresetChainNotSupported) | Preset doesn't support requested chain | Use a supported chain or create a custom policy file |

---

## Transaction Amounts (u256)

Starknet uses u256 for token amounts, split into low/high u128:

```
calldata: ["0xrecipient", "0x64", "0x0"]
                           ^^^^   ^^^^
                           low    high
```

For amounts that fit in u128 (most cases), set high to `0x0`.

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
