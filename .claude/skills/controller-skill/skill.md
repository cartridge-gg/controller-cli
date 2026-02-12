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
- Check transaction status

## Tools

### controller_generate_keypair

Generate a new session keypair for signing transactions.

**When to use:** First step in setting up a new session, or if no keypair exists.

**Input Schema:**
```json
{
  "type": "object",
  "properties": {}
}
```

**Output:** Public key and storage location

**Example:**
```bash
controller generate-keypair --json
```

---

### controller_status

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
controller status --json
```

---

### controller_register_session

Register a new session with specific contract/method policies. Requires human to authorize via browser.

**When to use:** After generating keypair, or when session expires, or when needing access to new contracts.

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "policy_file": {
      "type": "string",
      "description": "Path to JSON policy file defining allowed contracts and methods"
    }
  },
  "required": ["policy_file"]
}
```

**Important:** This command will output an authorization URL. Display this URL to the user and explain they need to open it in their browser to authorize. The command will automatically wait (up to 6 minutes) for authorization and store the session.

**Example:**
```bash
controller register-session policy.json --json
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
      "description": "Comma-separated calldata values (positional, hex with 0x prefix)"
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
  0xRECIPIENT_ADDRESS,0x64,0x0 \
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

### controller_clear

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
controller clear --yes
```

---

## Common Workflows

### First-Time Setup

1. Check status: `controller_status`
2. If no keypair: `controller_generate_keypair`
3. Create policy file with desired contracts/methods
4. Register session: `controller_register_session` (user must authorize in browser)
5. Execute transactions: `controller_execute`

### Transfer Tokens

```bash
# Check session is active
controller status --json

# Transfer 100 tokens (amount in u256: low, high)
controller execute \
  0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7 \
  transfer \
  0xRECIPIENT_ADDRESS,0x64,0x0 \
  --json
```

### Handle Expired Session

If status shows expired:
1. Create/update policy file if needed
2. Run `controller_register_session policy.json --json`
3. User authorizes in browser
4. Retry the transaction

## Error Handling

### NoSession
- **Cause:** No keypair found
- **Fix:** Run `controller_generate_keypair`

### SessionExpired
- **Cause:** Session expired
- **Fix:** Run `controller_register_session policy.json` (user must re-authorize)

### ManualExecutionRequired
- **Cause:** No authorized session for this transaction
- **Fix:** Register session with appropriate policies

### PolicyViolation
- **Cause:** Transaction not allowed by current session policies
- **Fix:** Register new session with expanded policies

## Important Notes

1. **Human Authorization Required:** Sessions require browser authorization. The LLM cannot bypass this - always prompt the user to open the URL.

2. **Session Expiration:** Sessions expire. Always check status before transactions.

3. **U256 Amounts:** Starknet uses u256 for amounts. Split into low/high:
   - For 100: `0x64,0x0`
   - For large amounts: calculate proper low/high split

4. **Subsidized Transactions:** On Sepolia testnet, transactions are automatically subsidized (no ETH needed for gas).

5. **Contract Addresses:** Must be 32-byte hex with 0x prefix.

6. **Always Use --json Flag:** For machine-readable output that's easy to parse.

## Common Contracts (Sepolia Testnet)

| Token | Address |
|-------|---------|
| STRK | `0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d` |
| ETH | `0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7` |

## Example Conversation

```
User: "Send 100 STRK to 0xabc123"

Agent: [Checks status]
> controller status --json
> Result: {"status": "no_session"}

Agent: "I need to set up a session first. Let me generate a keypair..."
> controller generate-keypair --json
> Result: {"public_key": "0x123..."}

Agent: [Creates policy file for STRK transfers]
> Creates policy.json with STRK contract and transfer method

Agent: "Now I need you to authorize this session. Please open this URL:"
> controller register-session policy.json --json
> Result: {"authorization_url": "https://x.cartridge.gg/session?..."}

Agent: "Please open the URL above and authorize the session. I'll wait..."

[User authorizes]

> Result: {"message": "Session registered successfully"}

Agent: "Great! Now executing the transfer..."
> controller execute 0x04718f5... transfer 0xabc123,0x64,0x0 --json
> Result: {"transaction_hash": "0x789..."}

Agent: "✅ Transfer submitted! Transaction hash: 0x789..."
```

## Security

- Private keys stored securely in `~/.config/controller-cli/`
- Sessions limit what contracts/methods can be called
- Human authorization required for all sessions
- Sessions expire automatically
- Transactions on Sepolia testnet only (currently)
