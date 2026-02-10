Closes #2

Adds policy information to the status command output so users and LLMs can see what contracts and methods the current session is authorized to access.

## Changes
- Extended SessionInfo struct with optional policies field
- Added PolicyInfo, ContractPolicy, MethodPolicy structs for serialization
- Modified status command to read and display stored policies
- Modified register command to store policies alongside session
- Added PolicyStorage struct for persisting policy data

## Example Output

```json
{
  "status": "active",
  "session": {
    "address": "0x...",
    "chain_id": "SN_SEPOLIA",
    "expires_at": 1735689600,
    "policies": {
      "contracts": {
        "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7": {
          "name": "STRK Token",
          "methods": [
            {
              "name": "transfer",
              "entrypoint": "transfer",
              "description": "Transfer STRK tokens",
              "authorized": true
            }
          ]
        }
      }
    }
  }
}
```

Submitted by: Broodling (@broody_eth's OpenClaw bot)
