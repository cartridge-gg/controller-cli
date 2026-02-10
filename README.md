# Cartridge Controller CLI

Command-line interface for managing Cartridge Controller sessions on Starknet.

## Overview

This CLI tool enables automated execution of Starknet transactions through a human-in-the-loop workflow:

1. **Generate a keypair** - Creates session signing keys
2. **Generate authorization URL** - Creates URL with policies and public key
3. **Human authorizes in browser** - Opens URL, reviews policies, authorizes session
4. **CLI automatically retrieves session** - Polls API until authorization is detected (no manual copy-paste needed!)
5. **Execute transactions** - Autonomously executes within authorized policies

This approach ensures security while enabling automation - the human operator maintains full control by authorizing specific contracts and methods through the browser.

**For LLMs/AI Agents:** See [LLM_USAGE.md](LLM_USAGE.md) for a complete integration guide with JSON examples and error handling.

## Installation

### Quick Install (Recommended)

Install the latest release with a single command:

```bash
curl -fsSL https://raw.githubusercontent.com/cartridge-gg/controller-cli/main/install.sh | bash
```

This will download the appropriate binary for your platform (Linux/macOS, x86_64/ARM64) and install it to `~/.local/bin`.

### Via Cargo (if you have Rust installed)

```bash
cargo install --git https://github.com/cartridge-gg/controller-cli
```

### From Source

```bash
git clone https://github.com/cartridge-gg/controller-cli.git
cd controller-cli
cargo build --release
```

The binary will be at `target/release/controller`.

## Quick Start

### 1. Generate a Keypair

```bash
controller generate-keypair
```

This creates and stores a new session keypair. The public key will be displayed.

**Security Note:** The private key is stored locally on your machine. Even if this key were to be compromised, the session is strictly scoped to:
- **Specific contracts** you authorize (e.g., only the STRK token contract)
- **Specific methods** on those contracts (e.g., only `transfer` and `approve`)
- **Time-limited** access (sessions expire, typically after 7 days)

This means a leaked session key cannot be used to access arbitrary contracts or methods, only those you explicitly authorized during session registration.

### 2. Check Status

```bash
controller status
```

Shows current session status, keypair info, and expiration details.

**Status States:**
- `no_session` - No keypair exists (run `controller generate-keypair`)
- `keypair_only` - Keypair exists but no registered session (run `controller register-session <policy.json>`)
- `active` - Session registered and not expired (ready to execute transactions)

### 3. Register a Session

Generate an authorization URL and wait for authorization:

```bash
controller register-session examples/policies.json
```

This will:
- Generate an authorization URL with your public key and policies
- Display the URL for you to open in a browser
- **Automatically poll the API** until you authorize (polls every 2 minutes, 6-minute total timeout)
- **Automatically store the session** once authorization is detected

Simply open the displayed URL in your browser and authorize - the CLI will handle the rest!

**Example output:**
```
Authorization URL:

https://api.cartridge.gg/s/abc123

Waiting for authorization (timeout: 5 minutes)...
Session registered and stored successfully.
```

The session is now ready to use - no manual copy-paste needed!

**Note:** If you already have an active session, you'll need to generate a new keypair before re-registering (a session keypair can only be registered once).

**Note:** The `store-session` command still exists for manual workflows or testing, but is not needed when using `register-session`:
```bash
# Manual mode (not typically needed)
controller store-session <base64_session_data>
controller store-session --from-file session.txt
```

### 4. Execute Transactions

**Single call**:

```bash
controller execute \
  --contract 0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7 \
  --entrypoint transfer \
  --calldata 0xrecipient,0x100,0x0
```

**Multiple calls from file** (see `examples/calls.json`):

```bash
controller execute --file examples/calls.json
```

Call file format:
```json
{
  "calls": [
    {
      "contractAddress": "0x049d36...",
      "entrypoint": "transfer",
      "calldata": ["0xrecipient", "0x100", "0x0"]
    }
  ]
}
```

**Wait for confirmation**:

```bash
controller execute --file calls.json --wait --timeout 300
```

The execute command will:
- Load and validate your session (check expiration)
- Create a Controller from stored credentials
- Use the RPC URL saved during session registration
- Execute the transaction on Starknet (auto-subsidized when possible)
- Return the transaction hash with a Voyager link
- Optionally wait for confirmation (with `--wait` flag)

### 5. Clear Session

```bash
controller clear
```

Removes all stored session data.

## JSON Output

All commands support `--json` flag for machine-readable output, useful for scripting and automation. Without this flag, commands display human-readable output.

```bash
controller status --json
```

Example JSON output formats:

**Active session:**
```json
{
  "data": {
    "status": "active",
    "session": {
      "address": "0x...",
      "expires_at": 1735689600,
      "expires_in_seconds": 3600,
      "expires_at_formatted": "2025-01-01 00:00:00 UTC",
      "is_expired": false
    },
    "keypair": {
      "public_key": "0x...",
      "has_private_key": true
    }
  },
  "status": "success"
}
```

**Keypair only (no session registered yet):**
```json
{
  "data": {
    "status": "keypair_only",
    "session": null,
    "keypair": {
      "public_key": "0x...",
      "has_private_key": true
    }
  },
  "status": "success"
}
```

**No keypair:**
```json
{
  "data": {
    "status": "no_session",
    "session": null,
    "keypair": null
  },
  "status": "success"
}
```

## Configuration

### Config File

Create `~/.config/controller-cli/config.toml`:

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

- `CARTRIDGE_STORAGE_PATH`: Override storage location
- `CARTRIDGE_CHAIN_ID`: Default chain ID
- `CARTRIDGE_RPC_URL`: Default RPC endpoint
- `CARTRIDGE_API_URL`: Override API endpoint for session queries
- `CARTRIDGE_JSON_OUTPUT`: Default to JSON output

## Session Policies

Policies define which contracts and methods the session can access. Create a JSON file:

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

## Automation & Scripting

The CLI is designed to be easily integrated into scripts, automation tools, and AI agents through its JSON output mode and straightforward command structure.

### Example Automated Workflow

Here's how an automated system might use the CLI:

```bash
# Check if session exists
STATUS=$(controller status --json)

# If no session, set one up
if [ "$(echo $STATUS | jq -r '.status')" = "no_session" ]; then
  # Generate keypair
  controller generate-keypair --json

  # Register session (this will block until authorized)
  controller register-session policies.json --json
  # User opens URL in browser and authorizes
fi

# Execute transaction
controller execute \
  --contract 0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7 \
  --entrypoint transfer \
  --calldata 0xabc123,0x64,0x0 \
  --json
```

### Integration with AI Agents

The CLI works seamlessly with AI agents (like Claude Code, GPT-4, etc.) through tools like Model Context Protocol (MCP). The JSON output format and predictable command structure make it easy to integrate:

```json
{
  "name": "cartridge_execute",
  "description": "Execute a Starknet transaction using Cartridge session",
  "inputSchema": {
    "type": "object",
    "properties": {
      "contract": {"type": "string"},
      "entrypoint": {"type": "string"},
      "calldata": {"type": "array", "items": {"type": "string"}}
    }
  }
}
```

The automatic polling and session management means AI agents can handle the full flow without manual intervention (except for the human authorization step in the browser).

**Example AI Agent Workflow:**

```
User: "Send 100 STRK to 0xabc123"

Agent: [Checks status]
> controller status --json
> Result: {"status": "no_session"}

Agent: [Generates keypair]
> controller generate-keypair --json
> Result: {"public_key": "0x78ad12..."}

Agent: [Requests authorization and waits]
> controller register-session policies.json --json

Agent: "Please open this URL to authorize the session:
       https://api.cartridge.gg/s/abc123

       Waiting for authorization..."

[Human opens URL and authorizes]

> Result: {
    "message": "Session registered and stored successfully",
    "public_key": "0x78ad12...",
    "chain_id": "SN_MAIN"
  }

Agent: "Session authorized! Now executing the transfer..."

Agent: [Executes transaction]
> controller execute \
    --contract 0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7 \
    --entrypoint transfer \
    --calldata 0xabc123,0x64,0x0 \
    --json
> Result: {
    "transaction_hash": "0x789...",
    "message": "Transaction submitted successfully"
  }

Agent: "✅ Transaction submitted!
       Hash: 0x789...
       You can view it on Starkscan."
```

## Error Handling

All errors are returned with:
- `error_code`: Machine-readable error type
- `message`: Human-readable description
- `recovery_hint`: Suggested next steps (when applicable)

Example error:

```json
{
  "status": "error",
  "error_code": "SessionExpired",
  "message": "Session expired at 2025-01-01 00:00:00 UTC",
  "recovery_hint": "Run 'controller register-session' to create a new session"
}
```

## Architecture

This CLI is built on top of the `account_sdk` from [controller-rs](https://github.com/cartridge-gg/controller-rs), which provides:

- Session management and signing
- Starknet transaction execution
- Policy validation
- File-based storage backend

The CLI acts as a thin command-line wrapper optimized for automation and scripting.

### Session Authorization Flow

Instead of requiring manual copy-paste of session data:

1. CLI generates authorization URL with `mode=cli` parameter
2. User authorizes in browser (keychain does NOT include session data in redirect)
3. CLI polls GraphQL API for session information
4. Once authorization is detected, CLI automatically stores session credentials

This uses **authorization signatures** instead of calculating `owner_guid` client-side, simplifying the implementation while maintaining security. See [IMPLEMENTATION.md](IMPLEMENTATION.md) for detailed architecture documentation.

## Development Status

**✅ CLI Implementation Complete**:
- ✅ Keypair generation and storage
- ✅ Session status checking with expiration
- ✅ Clear/reset command
- ✅ JSON output mode for automation
- ✅ Configuration management (TOML + env vars)
- ✅ Session registration URL generation with `mode=cli`
- ✅ Automatic API polling for session retrieval
- ✅ Policy file parsing and URL encoding
- ✅ Transaction execution (single and multi-call) using authorization signatures
- ✅ Session expiration validation
- ✅ Transaction confirmation waiting (--wait flag)

**⏳ Backend Requirements (In Progress)**:
- ⏳ `SessionInfo` GraphQL query endpoint
- ⏳ Keychain support for `mode=cli` parameter
- ⏳ API rate limiting and time-window enforcement

See [IMPLEMENTATION.md](IMPLEMENTATION.md) for backend requirements and testing checklist.

## Security

- **Private keys stored locally** - Session keys are stored in `~/.config/controller-cli/` with restricted permissions
- **Limited blast radius** - Even if a session key is compromised, it can only:
  - Access contracts you explicitly authorized (e.g., only STRK token)
  - Call methods you explicitly authorized (e.g., only `transfer` and `approve`)
  - Be used until the session expires (typically 7 days)
- **Human authorization required** - Every session must be authorized via browser
- **Policy enforcement** - Method-level access control prevents unauthorized calls
- **Session expiration** - Automatically validated before each transaction
- **No credential logging** - Sensitive data never written to logs
- **Rate limiting** - API polling protected against brute force attacks
- **Time-limited registration** - Session queries only available for 15 minutes after creation
- **Separation of concerns** - Authorization signatures cannot execute transactions (require session private key)

## License

MIT
