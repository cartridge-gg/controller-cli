---
name: controller-cli
description: Execute Starknet transactions using Cartridge Controller sessions with human-authorized policies. Use when the user wants to execute Starknet smart contract transactions, transfer tokens on Starknet, interact with gaming contracts, query contract state, look up Cartridge usernames/addresses, or manage session-based authentication.
---

# Controller CLI

Manage Cartridge Controller sessions and execute Starknet transactions through a secure human-in-the-loop workflow.

## Prerequisites

Controller CLI must be installed:

```bash
curl -fsSL https://raw.githubusercontent.com/cartridge-gg/controller-cli/main/install.sh | bash
```

## Session Workflow

Sessions use keypair-based auth where humans authorize specific contracts/methods via browser, then the agent executes transactions within those constraints.

1. **Check status** — `controller status --json`
2. **Generate keypair** (if needed) — `controller generate --json`
3. **Create policy file** — Define allowed contracts and methods
4. **Register session** — `controller register --file policy.json --json` (user must authorize via browser URL)
5. **Execute transactions** — `controller execute <contract> <entrypoint> <calldata> --json`

## Commands

### Status & Setup

```bash
controller status --json          # Check session status and expiration
controller generate --json        # Generate new session keypair
controller clear --yes            # Clear all session data
```

### Register Session

```bash
controller register --file policy.json --json
```

Outputs an authorization URL. Display it to the user and wait — the command polls for up to 6 minutes until the user authorizes in their browser.

### Execute Transaction

Single call (positional args):

```bash
controller execute <contract> <entrypoint> <calldata> [--wait] [--timeout <secs>] --json
```

Multiple calls from file:

```bash
controller execute --file calls.json [--wait] --json
```

### Read-Only Call (no session required)

```bash
controller call <contract> <entrypoint> <calldata> --chain-id SN_SEPOLIA --json
controller call --file calls.json --chain-id SN_SEPOLIA --json
```

### Transaction Status

```bash
controller transaction <hash> --chain-id SN_SEPOLIA [--wait] --json
```

### Username/Address Lookup

```bash
controller lookup --usernames shinobi,sensei --json
controller lookup --addresses 0x123...,0x456... --json
```

## Calldata Format

- Values are comma-separated, hex with `0x` prefix by default
- Decimal values: use `u256:100` prefix for automatic u256 encoding
- String values: use `str:hello` prefix for automatic felt encoding
- U256 manual encoding: split into low,high — e.g., 100 tokens = `0x64,0x0`

## Policy File Format

See [references/policy-examples.md](references/policy-examples.md) for complete examples.

```json
{
  "contracts": {
    "<contract_address>": {
      "name": "Contract Name",
      "methods": [
        { "name": "transfer", "entrypoint": "transfer", "description": "Transfer tokens" }
      ]
    }
  }
}
```

## Multi-Call File Format

```json
{
  "calls": [
    {
      "contractAddress": "<contract_address>",
      "entrypoint": "transfer",
      "calldata": ["0xRECIPIENT", "0x64", "0x0"]
    }
  ]
}
```

## Common Contracts (Sepolia)

| Token | Address |
|-------|---------|
| ETH   | `0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7` |
| STRK  | `0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d` |

## Error Handling

| Error | Cause | Fix |
|-------|-------|-----|
| NoSession | No keypair found | Run `controller generate` |
| SessionExpired | Session expired | Run `controller register --file policy.json` |
| ManualExecutionRequired | No authorized session | Register session with appropriate policies |
| PolicyViolation | Transaction not in allowed policies | Register new session with expanded policies |

## Important Notes

- Always use `--json` flag for machine-readable output
- Sessions expire — always check status before transactions
- Human authorization is required for all sessions (cannot be bypassed)
- Sepolia transactions are automatically subsidized (no gas needed)
- Contract addresses must be 32-byte hex with `0x` prefix
