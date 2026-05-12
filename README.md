# HiveEscrow

> **Trustless M2M escrow on Stellar — AI agents hire and pay each other using ZK-proofs.**

HiveEscrow is a Soroban-native protocol for the Machine-to-Machine (M2M) economy. It enables autonomous AI agents to create, fulfill, and settle service contracts on-chain without human intervention. Payment is released only when a worker agent submits a valid Groth16 zero-knowledge proof of task completion, verified on-chain using Stellar Protocol 25's BN254 host functions.

---

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Contract Interface](#contract-interface)
- [ZK Verification](#zk-verification)
- [State & Storage Model](#state--storage-model)
- [Repository Structure](#repository-structure)
- [Getting Started](#getting-started)
  - [Prerequisites](#prerequisites)
  - [Build the Contract](#build-the-contract)
  - [Run Tests](#run-tests)
  - [Deploy](#deploy)
- [Frontend](#frontend)
- [Security](#security)
- [Roadmap](#roadmap)
- [License](#license)

---

## Overview

| Property | Value |
|---|---|
| Network | Stellar / Soroban (Protocol 25+) |
| Language | Rust (`soroban-sdk = "21.0.0"`) |
| ZK Scheme | Groth16 over BN254 |
| Storage | Temporary (tasks) + Instance (counter) |
| Deadline Window | 48 hours minimum |
| Task TTL | ~30 days (~518,400 ledgers at 5 s/ledger) |

### How It Works

1. **Employer** calls `create_task`, depositing payment into the contract and committing to a verification key hash (`sha256(vk_bytes)`).
2. **Worker** performs the off-chain task and generates a Groth16 ZK-proof.
3. **Worker** calls `claim_reward`, supplying the proof and verification key. The contract:
   - Verifies `sha256(vk_bytes) == vk_hash`
   - Folds public inputs: `ic_combined = IC[0] + Σ(input[i] × IC[i+1])` via `g1_msm`
   - Runs the BN254 multi-pairing check: `e(−A, B) · e(α, β) · e(C, δ) · e(ic_combined, γ) == 1`
   - Transfers payment to the worker on success
4. If the deadline passes without a valid claim, the **Employer** calls `refund` to recover the deposit.

---

## Architecture

```
HiveEscrow/
├── contracts/
│   └── hive-escrow/
│       ├── Cargo.toml              # Contract crate (soroban-sdk 21.0.0)
│       └── src/
│           ├── lib.rs              # Crate root
│           ├── contract.rs         # Public entry points + ZK verification
│           ├── storage.rs          # EscrowTask, DataKey, TTL helpers
│           └── test.rs             # Unit + integration tests
├── docs/
│   └── hive_escrow_spec.md         # EARS notation specification
├── frontend/
│   ├── src/
│   │   ├── app/                    # Next.js App Router pages
│   │   ├── components/             # StatusBadge, TaskCard, ConnectWallet
│   │   └── lib/                    # contract.ts, wallet.tsx
│   └── package.json
├── scripts/
│   ├── deploy.sh                   # Build → optimize → deploy
│   └── invoke.sh                   # Call any contract function
├── .github/
│   └── workflows/
│       └── ci.yml                  # Test → build → optimize → artifact
├── Cargo.toml                      # Workspace root
└── .kiro/
    └── steering.md                 # Coding standards & best practices
```

---

## Contract Interface

### `create_task`

```rust
pub fn create_task(
    env: Env,
    employer: Address,
    worker: Address,
    token: Address,
    amount: i128,
    vk_hash: BytesN<32>,   // sha256(vk_bytes)
    deadline: u64,          // Unix timestamp; must be >= now + 48h
) -> Result<u64, Error>    // Returns task_id
```

Transfers `amount` tokens from `employer` to the contract and stores a new `EscrowTask` in Temporary storage.

---

### `claim_reward`

```rust
pub fn claim_reward(
    env: Env,
    worker: Address,
    task_id: u64,
    vk_bytes: Bytes,          // Full serialized verification key
    vk: VerifyingKey,         // Typed VK struct (alpha, beta, gamma, delta, ic[])
    proof: Proof,             // Groth16 proof (a, b, c)
    public_inputs: Vec<BytesN<32>>,  // BN254 scalar field elements
) -> Result<(), Error>
```

Verifies the Groth16 proof on-chain and transfers payment to the worker.

---

### `refund`

```rust
pub fn refund(
    env: Env,
    employer: Address,
    task_id: u64,
) -> Result<(), Error>
```

Returns the deposit to the employer after the deadline has passed and the task is still `Open`.

---

### `get_task`

```rust
pub fn get_task(env: Env, task_id: u64) -> Option<EscrowTask>
```

Read-only view. Returns `None` if the task has been archived (TTL expired) or never existed.

---

### Error Codes

| Code | Variant | Description |
|---|---|---|
| 1 | `InvalidAmount` | `amount <= 0` |
| 2 | `DeadlineTooSoon` | `deadline < now + 48h` |
| 3 | `Unauthorized` | Caller is not the expected employer/worker |
| 4 | `TaskNotOpen` | Task has already been claimed or refunded |
| 5 | `DeadlineExpired` | Claim attempted after deadline |
| 6 | `InvalidProof` | BN254 pairing check failed |
| 7 | `VkMismatch` | `sha256(vk_bytes) != stored vk_hash` |
| 8 | `DeadlineNotReached` | Refund attempted before deadline |
| 9 | `TaskNotFound` | Task not in storage (archived or non-existent) |
| 10 | `InvalidPublicInputs` | `ic.len() != public_inputs.len() + 1` |

---

## ZK Verification

HiveEscrow uses **Groth16** proofs over the **BN254** elliptic curve, verified via Stellar Protocol 25's native host functions.

### Verification Equation

```
e(−A, B) · e(α, β) · e(C, δ) · e(ic_combined, γ) == 1
```

Where:
- `A, C` — proof points in G1
- `B` — proof point in G2
- `α, β, γ, δ` — verification key points
- `ic_combined = IC[0] + Σ(public_inputs[i] × IC[i+1])` — computed on-chain via `g1_msm`

### VK Commitment

The employer commits to a verification key at task creation time by storing `vk_hash = sha256(vk_bytes)`. The worker must supply the full `vk_bytes` at claim time; the contract recomputes the hash and asserts equality before running the pairing check. This prevents proof substitution attacks.

### Proof Format

The contract accepts proof and VK data in **Ethereum-compatible (EIP-197) encoding**:
- G1 points: 64 bytes — `be(X) || be(Y)`
- G2 points: 128 bytes — `be(X.c1) || be(X.c0) || be(Y.c1) || be(Y.c0)`

This is compatible with the output of **snarkjs**, **Circom**, and **Noir**.

---

## State & Storage Model

| Data | Storage Type | Key | TTL |
|---|---|---|---|
| `EscrowTask` | Temporary | `DataKey::Task(task_id)` | ~30 days |
| `TaskCounter` | Instance | `DataKey::TaskCounter` | ~30 days (bumped on every call) |

**Why Temporary storage?** Tasks have a natural expiry — once claimed or refunded, they are no longer needed. Temporary storage avoids persistent rent costs. The TTL (~30 days) is intentionally larger than the maximum deadline window to ensure tasks are never archived before the employer can call `refund`.

---

## Getting Started

### Prerequisites

| Tool | Version | Install |
|---|---|---|
| Rust | stable | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| wasm32 target | — | `rustup target add wasm32-unknown-unknown` |
| Stellar CLI | latest | `cargo install --locked stellar-cli --features opt` |
| Node.js | 18+ | [nodejs.org](https://nodejs.org) |

### Build the Contract

```bash
# Build optimized WASM
cargo build \
  --manifest-path contracts/hive-escrow/Cargo.toml \
  --target wasm32-unknown-unknown \
  --release

# Optimize (reduces WASM size significantly)
stellar contract optimize \
  --wasm target/wasm32-unknown-unknown/release/hive_escrow.wasm
```

Or use the provided script:

```bash
./scripts/deploy.sh testnet
```

### Run Tests

```bash
cargo test --manifest-path contracts/hive-escrow/Cargo.toml
```

The test suite covers:

- `create_task` — success, invalid amount, deadline too soon, exact boundary
- `refund` — success after deadline, before deadline, wrong caller, double refund
- `claim_reward` — VK mismatch, expired deadline, wrong worker, task not found, invalid public input length, full pre-pairing path execution

### Deploy

```bash
# Set your secret key
export STELLAR_SECRET_KEY=S...

# Deploy to testnet (builds, optimizes, deploys, saves contract ID)
./scripts/deploy.sh testnet

# Deploy to mainnet
./scripts/deploy.sh mainnet
```

The contract ID is saved to `.soroban/contract-id.txt` for use by the invoke script.

### Invoke

```bash
export CONTRACT_ID=$(cat .soroban/contract-id.txt)
export STELLAR_SECRET_KEY=S...

# Look up a task
./scripts/invoke.sh get_task --task-id 0

# Refund an expired task
./scripts/invoke.sh refund --employer GABC... --task-id 0
```

---

## Frontend

A Next.js 15 frontend is included in `frontend/`. It supports:

- **Dashboard** — look up any task by ID
- **Create Task** — employer form with wallet-connected transaction flow
- **Task Detail** — shows task status; employer can refund, worker can submit ZK proof

### Run Locally

```bash
cd frontend
cp .env.local.example .env.local
# Edit .env.local and set NEXT_PUBLIC_CONTRACT_ID

npm install
npm run dev
```

Open [http://localhost:3000](http://localhost:3000).

### Environment Variables

| Variable | Description | Default |
|---|---|---|
| `NEXT_PUBLIC_CONTRACT_ID` | Deployed contract address | — |
| `NEXT_PUBLIC_RPC_URL` | Soroban RPC endpoint | `https://soroban-testnet.stellar.org` |
| `NEXT_PUBLIC_NETWORK_PASSPHRASE` | Network passphrase | Testnet |

### Supported Wallets

Freighter, xBull, Lobstr, Albedo, Rabet — via [Stellar Wallets Kit](https://github.com/Creit-Tech/Stellar-Wallets-Kit).

---

## Security

| Property | Implementation |
|---|---|
| Authorization | `address.require_auth()` is the first statement in every mutating function |
| Reentrancy | State is mutated before any external token transfer |
| Replay protection | Each `task_id` is unique and monotonically increasing; claimed/refunded tasks cannot be re-entered |
| ZK soundness | The contract never trusts off-chain proof validity; `pairing_check` is the sole arbiter |
| VK integrity | `sha256(vk_bytes)` is verified against the stored commitment before the pairing check |
| Input validation | Amount, deadline, and IC length are validated before any state writes |
| Storage expiry | If a task's TTL expires before claim or refund, the entry is archived. The employer must restore it (pay rent) before calling `refund`. This is an accepted trade-off for low-cost storage. |

---

## Roadmap

- [ ] On-chain VK registry — register a VK once, reference by hash across many tasks
- [ ] Milestone-based escrow — partial payments on proof of intermediate steps
- [ ] Multi-worker tasks — split reward among multiple provers
- [ ] Admin pause mechanism — emergency circuit breaker
- [ ] TypeScript / Python client bindings via `stellar contract bindings`
- [ ] Integration tests against a local Stellar node

---

## License

MIT
