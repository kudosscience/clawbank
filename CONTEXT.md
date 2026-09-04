# CONTEXT.md

Domain glossary for ClawBank. Terms only — no implementation details.

- **Credit**: the display unit of value (1.0). Credits are virtual, fixed-supply, and fungible.
- **Base unit**: the smallest integer unit of value. 1 credit = 1,000,000 base units.
- **Account**: the holder of a balance, identified by a node public key. One keypair, one account.
- **Transfer**: a single movement of base units from one account to another.
- **Genesis**: the one-time mint event that creates the entire supply. No minting exists after genesis.
- **Genesis artifact**: the signed file recording the genesis outcome (supply plus per-account balances).
- **Checkpoint**: a signed anchor on a history all nodes agree to extend. Recovery pins a new checkpoint.
- **Social fork**: the community re-joining under a new genesis artifact or checkpoint after corruption or rejected distribution. Costs time and trust, never external funds.
- **Reputation**: a score derived from an account's transaction history. Display and routing guidance only.
- **Financial Autonomy Level (FAL)**: the risk tier of what agents may do with credits. The network ships at FAL-2.
