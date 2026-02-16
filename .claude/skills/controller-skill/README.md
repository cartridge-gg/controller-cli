# Controller CLI Skill

An MCP skill that enables LLMs to execute Starknet transactions using Cartridge Controller sessions.

## Installation

### For Claude Code

```bash
# From the controller-cli repo root
ln -s "$(pwd)/.claude/skills/controller-skill" ~/.claude/skills/controller-skill
```

Or install directly:

```bash
# Copy skill to Claude skills directory
cp -r .claude/skills/controller-skill ~/.claude/skills/
```

### For Cursor

```bash
# Link to Cursor skills directory
ln -s "$(pwd)/.claude/skills/controller-skill" ~/.cursor/skills/controller-skill
```

## Prerequisites

1. **Install Controller CLI:**
   ```bash
   curl -fsSL https://raw.githubusercontent.com/cartridge-gg/controller-cli/main/install.sh | bash
   ```

2. **Verify Installation:**
   ```bash
   controller --version
   ```

## Quick Start

Once the skill is installed, you can ask Claude to:

- "Check my controller session status"
- "Authorize a new session for STRK transfers"
- "Send 100 STRK to 0xabc123"
- "Check my token balances"
- "What's my username?"
- "Look up the address for username shinobi"
- "Get the receipt for transaction 0x123..."

## Example Usage

### Check Status
```
You: "Check if I have an active controller session"

Claude: [Uses controller_session_status tool]
Claude: "You don't have an active session. Would you like me to set one up?"
```

### Setup Session
```
You: "Set up a session for STRK token transfers"

Claude: [Creates policy file]
Claude: [Uses controller_session_auth]
Claude: "Please open this URL to authorize: https://x.cartridge.gg/session?..."

You: [Opens URL and authorizes]

Claude: "Session authorized! You can now transfer STRK tokens."
```

### Execute Transaction
```
You: "Send 1 STRK to 0x123abc..."

Claude: [Uses controller_execute with u256: prefix]
Claude: "Transaction submitted! Hash: 0x789..."
```

### Check Balances
```
You: "What are my token balances?"

Claude: [Uses controller_balance]
Claude: "Your balances: 0.5 ETH, 100.0 STRK"
```

### Read-Only Call
```
You: "Check the STRK balance of 0x456..."

Claude: [Uses controller_call]
Claude: "The balance is 1000 STRK"
```

### Check Transaction
```
You: "What's the status of transaction 0x789...?"

Claude: [Uses controller_transaction]
Claude: "Transaction 0x789... has been confirmed."
```

### Get Receipt
```
You: "Show me the receipt for 0x789..."

Claude: [Uses controller_receipt]
Claude: "Transaction SUCCEEDED. Fee: 0x... FRI. 3 events emitted."
```

## Policy Files

The skill includes example policy files in the `examples/` directory:

- `strk-token-policy.json` - STRK token transfers and approvals
- `eth-token-policy.json` - ETH token transfers and approvals
- `multi-token-policy.json` - Both STRK and ETH tokens

You can create custom policy files for your specific contracts and methods.

## Tools Available

### Session Management
1. **controller_session_auth** - Generate keypair and authorize a new session (combines old generate + register)
2. **controller_session_status** - Check session status and expiration
3. **controller_session_list** - List all active sessions with pagination
4. **controller_session_clear** - Clear all session data

### Transaction Execution
5. **controller_execute** - Execute transactions (positional args: contract, entrypoint, calldata)
6. **controller_call** - Read-only contract calls (positional args: contract, entrypoint, calldata)

### Transaction Queries
7. **controller_transaction** - Get transaction status and details
8. **controller_receipt** - Get full transaction receipt (fee, events, execution resources)

### Account & Identity
9. **controller_balance** - Query ERC20 token balances (ETH, STRK, USDC, and more)
10. **controller_username** - Get the account's Cartridge username
11. **controller_lookup** - Look up usernames/addresses

### Configuration
12. **controller_config_set** - Set a config value
13. **controller_config_get** - Get a config value
14. **controller_config_list** - List all config values

## Calldata Formats

Calldata values support multiple formats:

| Format | Example | Description |
|--------|---------|-------------|
| Hex | `0x64` | Standard hex felt |
| Decimal | `100` | Decimal felt |
| `u256:` | `u256:1000000000000000000` | Auto-splits into low/high 128-bit felts |
| `str:` | `str:hello` | Cairo short string |

## Security

- Private keys stored securely in `~/.config/controller-cli/`
- Human authorization required for all sessions
- Sessions limit which contracts/methods can be called
- Sessions expire automatically

## Common Workflows

### First-Time Setup
1. Check status (`session status`)
2. Create policy file (or use example/preset)
3. Authorize session (`session auth`) - user authorizes in browser
4. Execute transactions

### Daily Use
1. Check status
2. Execute transactions
3. If expired, re-authorize session

## Troubleshooting

### "No session found"
- Run `controller session status`
- Authorize a new session with `controller session auth`

### "Session expired"
- Authorize new session with same policy file
- User must re-authorize in browser

### "Policy violation"
- Transaction not allowed by current policies
- Authorize new session with expanded policies

## Support

- Repository: https://github.com/cartridge-gg/controller-cli
- Issues: https://github.com/cartridge-gg/controller-cli/issues
- Documentation: See `skill.md` for detailed tool documentation
