# Controller CLI Quick Reference

## Installation (One Command)
```bash
curl -fsSL https://raw.githubusercontent.com/cartridge-gg/controller-cli/main/install.sh | bash
```

## Commands

| Command | Usage | JSON Flag |
|---------|-------|-----------|
| `generate-keypair` | Create new session keypair | `--json` |
| `status` | Check session status | `--json` |
| `register-session` | Register session (requires human auth) | `--json` |
| `execute` | Execute transaction | `--json` |
| `clear` | Clear stored session | `--yes` |

## Workflow

```bash
# 1. Generate keypair
controller generate-keypair --json

# 2. Register session (user must authorize in browser)
controller register-session policy.json --json

# 3. Execute transaction
controller execute --contract 0x... --entrypoint transfer --calldata 0x... --json
# or
controller execute --file calls.json --json
```

## JSON Formats

### Policy File (`policy.json`)
```json
{
  "contracts": {
    "0xCONTRACT_ADDRESS": {
      "methods": [
        {"name": "transfer", "entrypoint": "transfer"}
      ]
    }
  }
}
```

### Calls File (`calls.json`)
```json
{
  "calls": [
    {
      "contractAddress": "0x...",
      "entrypoint": "transfer",
      "calldata": ["0xRECIPIENT", "0xAMOUNT", "0x0"]
    }
  ]
}
```

## Common Contracts (Sepolia)

| Token | Address |
|-------|---------|
| STRK | `0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d` |
| ETH | `0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7` |

## Error Codes

| Code | Meaning | Fix |
|------|---------|-----|
| `NoSession` | No keypair found | Run `generate-keypair` |
| `SessionExpired` | Session expired | Run `register-session` again |
| `ManualExecutionRequired` | No valid session | Register session with policies |
| `CallbackTimeout` | User didn't authorize | Retry and authorize faster |

## Flags

| Flag | Purpose | Example |
|------|---------|---------|
| `--json` | JSON output (for LLMs) | All commands |
| `--wait` | Wait for tx confirmation | `execute` |
| `--timeout N` | Confirmation timeout (seconds) | `execute --wait` |
| `--yes` | Skip confirmation | `clear` |

## Exit Codes

- `0` - Success
- `1` - Error (check JSON output for details)
