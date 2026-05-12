# HiveEscrow — Kiro Steering Document

## Project Identity

HiveEscrow is a Soroban-native M2M escrow protocol on Stellar. All code targets the Soroban smart-contract VM (Protocol 25+). The primary language is Rust with `soroban-sdk = "21.0.0"`.

## Architecture

```
src/
  lib.rs        – crate root, re-exports contract + client
  contract.rs   – public entry points: create_task, claim_reward, refund
  storage.rs    – EscrowTask struct, DataKey enum, storage helpers
  test.rs       – unit + integration tests (cfg(test))
```

## Coding Rules

### Soroban SDK
- Always use `soroban-sdk = "21.0.0"` (exact version). Do not upgrade without updating this file.
- Use `#![no_std]` in `lib.rs`. Never import `std`.
- Derive `#[contracttype]` on all types that cross the host boundary.
- Use `#[contracterror]` for error enums; map each variant to a unique `u32`.

### State Archival (Stellar Best Practices)
- **Active tasks → Temporary storage.** Use `env.storage().temporary()` for `EscrowTask` entries.
- **Contract metadata → Instance storage.** Use `env.storage().instance()` for `TaskCounter` and any admin config.
- **Never use Persistent storage** for data that has a natural expiry (tasks, sessions).
- Always call `extend_ttl` after writing a Temporary entry. Use `BUMP_TTL_LEDGERS` as the threshold and `TASK_TTL_LEDGERS` as the target.
- Document TTL constants in `storage.rs` with their human-readable equivalent (e.g., `// ~7 days @ 5s/ledger`).

### ZK / Cryptography
- Use `env.crypto().bn254().pairing_check(vp1: Vec<Bn254G1Affine>, vp2: Vec<Bn254G2Affine>)` for BN254 proof verification (Protocol 25 host function).
- Never trust off-chain proof validity. Always verify on-chain.
- Store only `vk_hash = sha256(vk_bytes)` in contract state. Require callers to supply full `vk_bytes` and verify the hash before calling `pairing_check`.

### Security
- Call `address.require_auth()` as the **first** statement in any function that requires authorization.
- Mutate state **before** external token transfers to prevent reentrancy patterns.
- Validate all inputs (amount > 0, deadline bounds) before any state writes.

### Testing
- Use `env.mock_all_auths()` in test setup.
- Use `env.ledger().set_timestamp(ts)` to simulate time passage for deadline tests.
- Cover: happy path, each error variant, double-spend/double-refund, wrong-caller.
- Use `try_*` client methods to assert specific `Error` variants.

### Build & Release
- Build with `cargo build --target wasm32-unknown-unknown --release`.
- Optimize profile: `opt-level = "z"`, `lto = true`, `panic = "abort"`.
- Use `soroban contract optimize` to shrink the final `.wasm` before deployment.

## What NOT to Do
- Do not add `std` or `alloc` features beyond what `soroban-sdk` already provides.
- Do not store large blobs (VK bytes, proof bytes) in contract state — they are call-time inputs only.
- Do not use `unwrap()` in production code paths; propagate errors via `Result<_, Error>`.
- Do not push directly to `main`; open a PR for all changes.
