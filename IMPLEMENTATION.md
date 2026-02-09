# Controller CLI Implementation: Authorization-Based Session Flow

## Problem Statement

The Controller CLI enables LLM agents to execute StarkNet transactions through a human-in-the-loop workflow. The initial challenge was how to retrieve session credentials after a user authorizes in their browser without requiring manual copy-paste of base64 data.

## Solution Architecture

### Key Insight: Use Authorization Instead of owner_guid

Instead of trying to calculate `owner_guid` client-side (which requires owner signer details the CLI doesn't have), we use the **authorization signature** that's already sent to the backend during session creation.

**SessionAccount has two constructors:**
- `SessionAccount::new_as_registered(owner_guid, ...)` - requires calculating owner_guid
- `SessionAccount::new(authorization, ...)` - uses authorization signature ✅

The authorization is a signature proving the owner approved this specific session. It's safe to return from the API because:
- It can't be used to sign transactions (need session private key for that)
- It can't be reused for different sessions (cryptographically bound to session hash)
- It's already exposed in the current URL redirect approach (less secure than API with rate limiting)

### Workflow

```
1. LLM runs: controller-cli generate-keypair
   → Generates session keypair, stores locally, outputs public_key

2. LLM runs: controller-cli register-session --policy-file policies.json
   → Builds authorization URL with public_key, policies, mode=cli
   → Displays URL to user
   → Polls API for session creation (3s intervals, 5min timeout)
   → When authorized, automatically stores session credentials

3. LLM runs: controller-cli execute --contract 0x... --entrypoint transfer --calldata ...
   → Loads session from storage
   → Creates SessionAccount using authorization
   → Executes transaction
```

## Implementation Details

### 1. API Module (`src/api.rs`)

Created GraphQL client for querying session information:

```rust
pub struct SessionInfo {
    pub authorization: Vec<String>,    // Hex-encoded Felt values
    pub address: String,
    pub chain_id: String,
    pub expires_at: u64,
    pub username: String,
    pub class_hash: String,
    pub rpc_url: String,
    pub salt: String,
    pub owner_signer: SignerInfo,
}

pub enum SignerInfo {
    Starknet { private_key: String },  // For storage (not signing!)
    Webauthn { data: String },         // TBD: webauthn fields
}
```

**Query function:**
```rust
pub async fn query_session_info(
    api_url: &str,
    session_key_guid: &str,
) -> Result<Option<SessionInfo>>
```

Returns `None` if session doesn't exist yet, `Some(SessionInfo)` once authorized.

### 2. Register-Session Command (`src/commands/register.rs`)

**Key changes:**
1. Added `mode=cli` to authorization URL query parameters
2. Implemented polling mechanism with idempotency check
3. Auto-stores session when authorization is received

**URL format:**
```
https://x.cartridge.gg/session
  ?public_key=0x...
  &redirect_uri=https://x.cartridge.gg
  &policies={"contracts":{...}}
  &rpc_url=https://api.cartridge.gg/x/starknet/sepolia
  &mode=cli  ← Tells keychain not to include session data in redirect
```

**Polling logic:**
```rust
// Check if session already exists first (idempotency)
if let Some(session_info) = api::query_session_info(&api_url, &session_key_guid).await? {
    store_session_from_api(&mut backend, session_info, &public_key)?;
    return Ok(());
}

// Display URL and start polling
loop {
    if timeout_reached { return Err(CallbackTimeout); }

    if let Some(session_info) = api::query_session_info(&api_url, &session_key_guid).await? {
        store_session_from_api(&mut backend, session_info, &public_key)?;
        return Ok(());
    }

    sleep(3 seconds);
}
```

**Storage helper:**
```rust
fn store_session_from_api(
    backend: &mut FileSystemBackend,
    session_info: SessionInfo,
    public_key: &str,
) -> Result<()>
```

Constructs complete `SessionMetadata` and `ControllerMetadata` from API response and stores via `FileSystemBackend`.

### 3. Execute Command (`src/commands/execute.rs`)

**Changed from:**
```rust
let session_account = SessionAccount::new_as_registered(
    provider,
    signer,
    controller_metadata.address,
    controller_metadata.chain_id,
    owner_guid,  // ❌ Don't have this
    session_metadata.session,
);
```

**To:**
```rust
let authorization = credentials.authorization.clone();

let session_account = SessionAccount::new(
    provider,
    signer,
    controller_metadata.address,
    controller_metadata.chain_id,
    authorization,  // ✅ Have this from API
    session_metadata.session.clone(),
);
```

### 4. Configuration (`src/config.rs`)

Added `api_url` field:
```rust
pub struct SessionConfig {
    pub storage_path: String,
    pub default_chain_id: String,
    pub default_rpc_url: String,
    pub keychain_url: String,     // https://x.cartridge.gg (UI)
    pub api_url: String,          // https://api.cartridge.gg/query (GraphQL)
}
```

## Backend Requirements

### GraphQL Query Used

The CLI uses the **existing** `subscribeCreateSession` query from controller-rs:

```graphql
query SubscribeCreateSession($sessionKeyGuid: Felt!) {
  subscribeCreateSession(sessionKeyGuid: $sessionKeyGuid) {
    id
    appID
    chainID
    isRevoked
    expiresAt
    createdAt
    updatedAt
    authorization
    controller {
      address
      accountID
    }
  }
}
```

**Returns:**
- `null` if session doesn't exist yet
- Session data once user authorizes in browser

**Note:** The CLI only uses these fields from the response:
- `authorization` - for SessionAccount creation
- `controller.address` - account address
- `chainID` - chain identifier
- `expiresAt` - session expiration timestamp
- `controller.accountID` - stored as username

Other fields (`username`, `class_hash`, `rpc_url`, `salt`, `owner`) are stored with placeholder values since they're not needed for transaction execution.

### Security Measures to Implement

1. **Rate Limiting**: 10 requests/minute per IP (prevents brute force polling)
2. **Time Windows**: Only allow queries for 15 minutes after session creation (prevents long-term exposure)
3. **Single Use**: Mark session as "claimed" after first successful query (prevents replay)

**Why authorization is safe to return:**
- It's just a signature proving the owner approved this session
- Cannot be used to execute transactions (requires session private key)
- Cannot be reused for different sessions (cryptographically bound to session hash)
- Already exposed in current URL redirect (API with rate limiting is MORE secure)

### Keychain Update Required

Detect `mode=cli` parameter and skip adding session data to redirect URL:

```typescript
if (params.mode === 'cli') {
  // Don't include session data in redirect
  // Backend will make it available via SessionInfo query instead
  redirect(redirectUri);
} else {
  // Current behavior: include session data in URL
  redirect(`${redirectUri}?${redirectQueryName}=${base64SessionData}`);
}
```

## Testing Checklist

Once backend is implemented:

- [ ] Generate keypair successfully
- [ ] Register-session creates authorization URL with mode=cli
- [ ] Open URL in browser, authorize session
- [ ] CLI polling detects authorization and stores session
- [ ] Execute command works with stored session
- [ ] Status command shows correct session details
- [ ] Session expiration is enforced
- [ ] Idempotency: running register-session twice doesn't fail
- [ ] Timeout: polling stops after 5 minutes if not authorized
- [ ] Rate limiting works on API
- [ ] Time window enforcement works (queries fail after 15min)

## Key Files

- `src/api.rs` - GraphQL client
- `src/commands/register.rs` - Polling and storage
- `src/commands/execute.rs` - Transaction execution
- `src/config.rs` - Configuration with api_url
- `examples/policies.json` - Example policy file

## Dependencies Added

```toml
reqwest = { version = "0.12", features = ["json"] }  # HTTP client for API queries
```

## Next Steps

1. **Backend Team**:
   - ✅ `subscribeCreateSession` query already exists (no changes needed!)
   - Add security measures (rate limiting, time windows) to existing query
2. **Frontend Team**: Update keychain to detect `mode=cli` parameter
3. **Testing**: End-to-end testing once keychain changes are ready
4. **Documentation**: ✅ README updated with new workflow

## Notes

- Webauthn signers not yet supported in CLI (returns error if API returns webauthn signer)
- Policy validation not yet implemented (TODO: validate calls against session policies)
- Session policies not stored in SessionMetadata (TODO: store requested_policies if needed)
- Guardian key GUID set to zero (TODO: get from API if needed)
