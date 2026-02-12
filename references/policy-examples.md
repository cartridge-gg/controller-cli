# Policy File Examples

## ETH Token Policy

```json
{
  "contracts": {
    "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7": {
      "name": "ETH Token",
      "methods": [
        {
          "name": "transfer",
          "entrypoint": "transfer",
          "description": "Transfer ETH tokens to another address"
        },
        {
          "name": "approve",
          "entrypoint": "approve",
          "description": "Approve another address to spend ETH tokens"
        }
      ]
    }
  }
}
```

## STRK Token Policy

```json
{
  "contracts": {
    "0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d": {
      "name": "STRK Token",
      "methods": [
        {
          "name": "transfer",
          "entrypoint": "transfer",
          "description": "Transfer STRK tokens to another address"
        },
        {
          "name": "approve",
          "entrypoint": "approve",
          "description": "Approve another address to spend STRK tokens"
        }
      ]
    }
  }
}
```

## Multi-Token Policy

Combine multiple contracts in a single policy file:

```json
{
  "contracts": {
    "0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d": {
      "name": "STRK Token",
      "methods": [
        { "name": "transfer", "entrypoint": "transfer", "description": "Transfer STRK tokens" },
        { "name": "approve", "entrypoint": "approve", "description": "Approve STRK token spending" }
      ]
    },
    "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7": {
      "name": "ETH Token",
      "methods": [
        { "name": "transfer", "entrypoint": "transfer", "description": "Transfer ETH tokens" },
        { "name": "approve", "entrypoint": "approve", "description": "Approve ETH token spending" }
      ]
    }
  }
}
```

## Custom Game Contract Policy

```json
{
  "contracts": {
    "0xYOUR_GAME_CONTRACT_ADDRESS": {
      "name": "My Game",
      "methods": [
        { "name": "move", "entrypoint": "move", "description": "Make a move in the game" },
        { "name": "attack", "entrypoint": "attack", "description": "Attack another player" },
        { "name": "claim", "entrypoint": "claim_rewards", "description": "Claim game rewards" }
      ]
    }
  }
}
```
