# Controller CLI Skill

Execute Starknet transactions using Cartridge Controller sessions.

## Description

This skill enables LLMs to manage Cartridge Controller sessions and execute Starknet transactions through a secure human-in-the-loop workflow. The controller uses session-based authentication where humans authorize specific contracts and methods via browser, then the LLM can execute transactions autonomously within those constraints.

## Prerequisites

- Controller CLI installed: `curl -fsSL https://raw.githubusercontent.com/cartridge-gg/controller-cli/main/install.sh | bash`
- User must authorize sessions via browser (one-time setup per session)

## When to Use

Use this skill when the user wants to:
- Execute Starknet smart contract transactions
- Transfer tokens on Starknet
- Interact with gaming contracts
- Manage Starknet sessions
- Check transaction status or receipts
- Query token balances
- Look up usernames or addresses

## Tools

### controller_session_auth

Generate a keypair and authorize a new session in a single step.

**When to use:** To set up a new session, or when the current session has expired.

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "policy_file": {
      "type": "string",
      "description": "Path to JSON policy file defining allowed contracts and methods"
    },
    "preset": {
      "type": "string",
      "description": "Preset name (e.g., 'loot-survivor'). Alternative to policy_file."
    },
    "chain_id": {
      "type": "string",
      "description": "Chain ID (e.g., 'SN_MAIN' or 'SN_SEPOLIA')"
    },
    "rpc_url": {
      "type": "string",
      "description": "RPC URL (overrides config, conflicts with chain_id)"
    }
  }
}
```

**Important:** This command will output an authorization URL. Display this URL to the user and explain they need to open it in their browser to authorize. The command will automatically wait (up to 6 minutes) for authorization and store the session.

**Example (preset):**
```bash
controller session auth --preset loot-survivor --chain-id SN_MAIN --json
```

**Example (policy file):**
```bash
controller session auth --file policy.json --json
```

**Policy file format:**
```json
{
  "contracts": {
    "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7": {
      "name": "ETH Token",
      "methods": [
        {
          "name": "transfer",
          "entrypoint": "transfer",
          "description": "Transfer ETH tokens"
        },
        {
          "name": "approve",
          "entrypoint": "approve",
          "description": "Approve token spending"
        }
      ]
    }
  }
}
```

---

### controller_session_status

Check current session status, expiration, and keypair information.

**When to use:** Before executing transactions to verify session is active, or to diagnose issues.

**Input Schema:**
```json
{
  "type": "object",
  "properties": {}
}
```

**Output:** Session status, expiration time, keypair info

**Example:**
```bash
controller session status --json
```

---

### controller_session_list

List all active sessions with pagination.

**When to use:** To see all sessions registered for the account, check which is current, or view expiration times.

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "chain_id": {
      "type": "string",
      "description": "Chain ID to filter sessions (defaults to session chain)"
    },
    "limit": {
      "type": "number",
      "description": "Sessions per page (default: 10)",
      "default": 10
    },
    "page": {
      "type": "number",
      "description": "Page number starting from 1 (default: 1)",
      "default": 1
    }
  }
}
```

**Example:**
```bash
controller session list --json
controller session list --limit 20 --page 2 --json
```

---

### controller_session_clear

Clear all stored session data and keypairs.

**When to use:** To reset and start fresh, or when troubleshooting session issues.

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "yes": {
      "type": "boolean",
      "description": "Skip confirmation prompt",
      "default": true
    }
  }
}
```

**Example:**
```bash
controller session clear --yes
```

---

### controller_execute

Execute a Starknet transaction using the active session.

**When to use:** To execute any smart contract call within authorized policies.

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "contract": {
      "type": "string",
      "description": "Contract address (positional, hex with 0x prefix)"
    },
    "entrypoint": {
      "type": "string",
      "description": "Function name to call (positional)"
    },
    "calldata": {
      "type": "string",
      "description": "Comma-separated calldata values (positional). Supports hex, decimal, u256:, and str: prefixes."
    },
    "file": {
      "type": "string",
      "description": "Path to JSON file with multiple calls (alternative to positional args)"
    },
    "wait": {
      "type": "boolean",
      "description": "Wait for transaction confirmation (default: false)",
      "default": false
    },
    "timeout": {
      "type": "number",
      "description": "Timeout in seconds when waiting (default: 300)",
      "default": 300
    }
  }
}
```

**Note:** Either provide positional `contract` `entrypoint` `calldata` for a single call, OR provide `--file` for multiple calls.

**Example (single call — positional args):**
```bash
controller execute \
  0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7 \
  transfer \
  0xRECIPIENT_ADDRESS,u256:1000000000000000000 \
  --json
```

**Example (multiple calls from file):**
```bash
controller execute --file calls.json --json
```

**Calls file format:**
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

---

### controller_call

Execute a read-only call to a contract (no session required).

**When to use:** To query contract state such as balances, allowances, or game state without submitting a transaction.

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "contract": {
      "type": "string",
      "description": "Contract address (positional, hex with 0x prefix)"
    },
    "entrypoint": {
      "type": "string",
      "description": "Function name to call (positional)"
    },
    "calldata": {
      "type": "string",
      "description": "Comma-separated calldata values (positional, hex with 0x prefix)"
    },
    "file": {
      "type": "string",
      "description": "Path to JSON file with multiple calls (alternative to positional args)"
    },
    "chain_id": {
      "type": "string",
      "description": "Chain ID (e.g., 'SN_MAIN' or 'SN_SEPOLIA')"
    },
    "rpc_url": {
      "type": "string",
      "description": "RPC URL (overrides config, conflicts with chain_id)"
    },
    "block_id": {
      "type": "string",
      "description": "Block ID to query (latest, pending, block number, or block hash)"
    }
  }
}
```

**Note:** Does not require an active session. Only needs a network via `--chain-id` or `--rpc-url`.

**Example (positional args):**
```bash
controller call \
  0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7 \
  balance_of \
  0xADDRESS \
  --chain-id SN_SEPOLIA \
  --json
```

**Example (from file):**
```bash
controller call --file calls.json --chain-id SN_SEPOLIA --json
```

---

### controller_transaction

Get transaction status and details.

**When to use:** To check whether a previously submitted transaction has been confirmed, or to wait for confirmation.

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "hash": {
      "type": "string",
      "description": "Transaction hash (positional)"
    },
    "chain_id": {
      "type": "string",
      "description": "Chain ID (e.g., 'SN_MAIN' or 'SN_SEPOLIA')"
    },
    "rpc_url": {
      "type": "string",
      "description": "RPC URL (overrides config, conflicts with chain_id)"
    },
    "wait": {
      "type": "boolean",
      "description": "Wait for transaction to be confirmed (default: false)",
      "default": false
    },
    "timeout": {
      "type": "number",
      "description": "Timeout in seconds when waiting (default: 300)",
      "default": 300
    }
  },
  "required": ["hash"]
}
```

**Example:**
```bash
controller transaction 0xTRANSACTION_HASH --chain-id SN_SEPOLIA --json
```

**Example (wait for confirmation):**
```bash
controller transaction 0xTRANSACTION_HASH --chain-id SN_SEPOLIA --wait --json
```

---

### controller_receipt

Get the full transaction receipt including execution status, fee, events, and messages.

**When to use:** To get detailed information about a confirmed transaction, including events emitted and execution resources used.

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "hash": {
      "type": "string",
      "description": "Transaction hash (positional)"
    },
    "chain_id": {
      "type": "string",
      "description": "Chain ID (e.g., 'SN_MAIN' or 'SN_SEPOLIA')"
    },
    "rpc_url": {
      "type": "string",
      "description": "RPC URL (overrides config, conflicts with chain_id)"
    },
    "wait": {
      "type": "boolean",
      "description": "Wait for receipt to be available (default: false)",
      "default": false
    },
    "timeout": {
      "type": "number",
      "description": "Timeout in seconds when waiting (default: 300)",
      "default": 300
    }
  },
  "required": ["hash"]
}
```

**Example:**
```bash
controller receipt 0xTRANSACTION_HASH --chain-id SN_SEPOLIA --json
```

**Example (wait for receipt):**
```bash
controller receipt 0xTRANSACTION_HASH --chain-id SN_SEPOLIA --wait --json
```

---

### controller_balance

Query ERC20 token balances for the active session account.

**When to use:** To check token balances. Prefer this over raw `call balance_of` for common tokens.

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "symbol": {
      "type": "string",
      "description": "Token symbol (e.g., 'eth', 'strk'). If omitted, queries all known tokens."
    },
    "chain_id": {
      "type": "string",
      "description": "Chain ID (e.g., 'SN_MAIN' or 'SN_SEPOLIA')"
    },
    "rpc_url": {
      "type": "string",
      "description": "RPC URL (overrides config, conflicts with chain_id)"
    }
  }
}
```

**Built-in tokens:** ETH, STRK, USDC, USD.e, LORDS, SURVIVOR, WBTC. Custom tokens can be added via `controller config set token.<SYMBOL> <address>`.

**Example:**
```bash
controller balance --json
controller balance eth --json
controller balance --chain-id SN_MAIN --json
```

---

### controller_username

Display the Cartridge username associated with the active session account.

**When to use:** To find out the username for the currently active account.

**Input Schema:**
```json
{
  "type": "object",
  "properties": {}
}
```

**Example:**
```bash
controller username --json
```

---

### controller_lookup

Look up Cartridge controller addresses by usernames or usernames by addresses.

**When to use:** To resolve a username to an on-chain address, or find the username associated with an address.

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "usernames": {
      "type": "string",
      "description": "Comma-separated usernames to resolve (e.g., 'shinobi,sensei')"
    },
    "addresses": {
      "type": "string",
      "description": "Comma-separated addresses to resolve (e.g., '0x123...,0x456...')"
    }
  }
}
```

**Note:** Provide at least one of `usernames` or `addresses`. Both can be used in the same call.

**Output:** Array of `username:address` pairs

**Example (by username):**
```bash
controller lookup --usernames shinobi,sensei --json
```

**Example (by address):**
```bash
controller lookup --addresses 0x123...,0x456... --json
```

---

### controller_config

Manage CLI configuration values.

**When to use:** To set, get, or list configuration values (e.g., default RPC URL, custom tokens).

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "action": {
      "type": "string",
      "enum": ["set", "get", "list"],
      "description": "Config action to perform"
    },
    "key": {
      "type": "string",
      "description": "Config key (required for set/get). Valid: rpc-url, keychain-url, api-url, storage-path, json-output, colors, callback-timeout, token.<symbol>"
    },
    "value": {
      "type": "string",
      "description": "Value to set (required for set action)"
    }
  },
  "required": ["action"]
}
```

**Example:**
```bash
controller config set rpc-url https://api.cartridge.gg/x/starknet/mainnet
controller config get rpc-url --json
controller config list --json
controller config set token.MYTOKEN 0x123...
```

---

## Calldata Formats

Calldata values support multiple formats:

| Format | Example | Description |
|--------|---------|-------------|
| Hex | `0x64` | Standard hex felt |
| Decimal | `100` | Decimal felt (auto-converted) |
| `u256:` | `u256:1000000000000000000` | Auto-splits into low/high 128-bit felts |
| `str:` | `str:hello` | Cairo short string encoding |

The `u256:` prefix is the recommended way to specify token amounts. It eliminates manual low/high splitting.

## Common Workflows

### First-Time Setup

1. Check status: `controller session status --json`
2. Create policy file with desired contracts/methods
3. Authorize session: `controller session auth --file policy.json --json` (user must authorize in browser)
4. Execute transactions: `controller execute ...`

### Transfer Tokens

```bash
# Check session is active
controller session status --json

# Transfer 1 STRK using u256: prefix
controller execute \
  0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d \
  transfer \
  0xRECIPIENT_ADDRESS,u256:1000000000000000000 \
  --json
```

### Handle Expired Session

If status shows expired:
1. Create/update policy file if needed
2. Run `controller session auth --file policy.json --json`
3. User authorizes in browser
4. Retry the transaction

## Error Handling

### NoSession
- **Cause:** No keypair found
- **Fix:** Run `controller session auth --file policy.json`

### SessionExpired
- **Cause:** Session expired
- **Fix:** Run `controller session auth --file policy.json` (user must re-authorize)

### ManualExecutionRequired
- **Cause:** No authorized session for this transaction
- **Fix:** Authorize session with appropriate policies

### PolicyViolation
- **Cause:** Transaction not allowed by current session policies
- **Fix:** Authorize new session with expanded policies

## Important Notes

1. **Human Authorization Required:** Sessions require browser authorization. The LLM cannot bypass this - always prompt the user to open the URL.

2. **Session Expiration:** Sessions expire. Always check status before transactions.

3. **Calldata Prefixes:** Use `u256:` for token amounts instead of manual low/high splitting. Use `str:` for Cairo short strings. Decimal values are supported without any prefix.

4. **Subsidized Transactions:** On Sepolia testnet, transactions are automatically subsidized (no ETH needed for gas).

5. **Contract Addresses:** Must be 32-byte hex with 0x prefix.

6. **Always Use --json Flag:** For machine-readable output that's easy to parse.

7. **Use `balance` command:** Prefer `controller balance` over raw `call balance_of` for token balance queries.

## Common Contracts (Sepolia Testnet)

| Token | Address |
|-------|---------|
| STRK | `0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d` |
| ETH | `0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7` |

## Example Conversation

```
User: "Send 100 STRK to 0xabc123"

Agent: [Checks status]
> controller session status --json
> Result: {"status": "no_session"}

Agent: "I need to set up a session first. Let me authorize one..."
> [Creates policy.json with STRK contract and transfer method]

Agent: "Now I need you to authorize this session. Please open this URL:"
> controller session auth --file policy.json --json
> Result: {"authorization_url": "https://x.cartridge.gg/session?...", "short_url": "https://api.cartridge.gg/s/abc123"}

Agent: "Please open the URL above and authorize the session. I'll wait..."

[User authorizes]

> Result: {"message": "Session authorized and stored successfully"}

Agent: "Great! Now executing the transfer..."
> controller execute 0x04718f5... transfer 0xabc123,u256:100000000000000000000 --json
> Result: {"transaction_hash": "0x789..."}

Agent: "Transfer submitted! Transaction hash: 0x789..."
```

## Security

- Private keys stored securely in `~/.config/controller-cli/`
- Sessions limit what contracts/methods can be called
- Human authorization required for all sessions
- Sessions expire automatically
