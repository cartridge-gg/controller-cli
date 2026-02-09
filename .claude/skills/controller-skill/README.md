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

- "Check my controller status"
- "Generate a new controller keypair"
- "Send 100 STRK to 0xabc123"
- "Execute a transaction on Starknet"

## Example Usage

### Check Status
```
You: "Check if I have an active controller session"

Claude: [Uses controller_status tool]
Claude: "You don't have an active session. Would you like me to set one up?"
```

### Setup Session
```
You: "Set up a session for STRK token transfers"

Claude: [Uses controller_generate_keypair]
Claude: [Creates policy file]
Claude: [Uses controller_register_session]
Claude: "Please open this URL to authorize: https://x.cartridge.gg/session?..."

You: [Opens URL and authorizes]

Claude: "Session authorized! You can now transfer STRK tokens."
```

### Execute Transaction
```
You: "Send 100 STRK to 0x123abc..."

Claude: [Uses controller_execute]
Claude: "Transaction submitted! Hash: 0x789..."
```

## Policy Files

The skill includes example policy files in the `examples/` directory:

- `strk-token-policy.json` - STRK token transfers and approvals
- `eth-token-policy.json` - ETH token transfers and approvals
- `multi-token-policy.json` - Both STRK and ETH tokens

You can create custom policy files for your specific contracts and methods.

## Tools Available

1. **controller_generate_keypair** - Generate session keypair
2. **controller_status** - Check session status
3. **controller_register_session** - Register new session (requires browser auth)
4. **controller_execute** - Execute transactions
5. **controller_clear** - Clear session data

## Security

- Private keys stored securely in `~/.config/cartridge/`
- Human authorization required for all sessions
- Sessions limit which contracts/methods can be called
- Sessions expire automatically
- Currently only supports Sepolia testnet

## Common Workflows

### First-Time Setup
1. Generate keypair
2. Create policy file (or use example)
3. Register session (user authorizes in browser)
4. Execute transactions

### Daily Use
1. Check status
2. Execute transactions
3. If expired, re-register session

## Troubleshooting

### "No session found"
- Run controller status
- Generate keypair if needed
- Register new session

### "Session expired"
- Register new session with same policy file
- User must re-authorize in browser

### "Policy violation"
- Transaction not allowed by current policies
- Register new session with expanded policies

## Support

- Repository: https://github.com/cartridge-gg/controller-cli
- Issues: https://github.com/cartridge-gg/controller-cli/issues
- Documentation: See `skill.md` for detailed tool documentation
