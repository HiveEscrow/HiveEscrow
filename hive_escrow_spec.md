# HiveEscrow Protocol Specification

**Version:** 1.0.0  
**Date:** 2026-05-12  
**Network:** Stellar / Soroban (Protocol 25+)

---

## 1. Overview

HiveEscrow is a Soroban-native M2M escrow protocol enabling AI agents to hire and pay each other trustlessly. An Employer agent deposits funds into a time-bounded escrow task; a Worker agent claims the reward by submitting a ZK-proof of work completion. If the deadline passes without a valid claim, the Employer may reclaim the deposit.

---

## 2. Actors

| Actor    | Description                                              |
|----------|----------------------------------------------------------|
| Employer | AI agent that creates a task and deposits payment        |
| Worker   | AI agent that performs the task and submits a ZK-proof   |
| Contract | HiveEscrow Soroban contract enforcing protocol rules     |

---

## 3. State Model

### 3.1 EscrowTask (Temporary Storage)

Active tasks are stored in **Temporary storage** to leverage Soroban's state-rent model. Tasks that expire without a claim are naturally archived, minimizing rent costs.

| Field          | Type      | Description                                      |
|----------------|-----------|--------------------------------------------------|
| `employer`     | `Address` | Account that created and funded the task         |
| `worker`       | `Address` | Account authorized to claim the reward           |
| `token`        | `Address` | SAC token contract for payment                   |
| `amount`       | `i128`    | Reward amount in stroops                         |
| `vk_hash`      | `BytesN<32>` | SHA-256 hash of the Groth16 verification key  |
| `deadline`     | `u64`     | Unix timestamp after which refund is permitted   |
| `status`       | `TaskStatus` | `Open` \| `Claimed` \| `Refunded`             |

### 3.2 Storage Keys

```
DataKey::Task(task_id: u64)   → EscrowTask   [Temporary]
DataKey::TaskCounter          → u64           [Instance]
```

Temporary entries are extended on `create_task` with a TTL of `TASK_TTL_LEDGERS` (≈ 30 days at 5s/ledger = 518,400 ledgers). This is intentionally larger than the maximum deadline window to ensure a task is never archived before the employer can call `refund`. The TTL is bumped on `claim_reward` and `refund` to ensure liveness during the transaction. The contract instance TTL is bumped on every mutating call via `bump_instance`.

---

## 4. Constants

| Constant            | Value       | Rationale                              |
|---------------------|-------------|----------------------------------------|
| `DEADLINE_WINDOW`   | `172800` s  | 48-hour minimum refund window          |
| `TASK_TTL_LEDGERS`  | `518400`    | ~30 days at 5 s/ledger (> max deadline)|
| `BUMP_TTL_LEDGERS`  | `17280`     | ~1 day bump threshold                  |

---

## 5. Functional Requirements (EARS Notation)

### 5.1 create_task

> **EARS (Event-driven):** WHEN an Employer invokes `create_task` with a valid worker address, token, amount, verification-key hash, and deadline, the system SHALL transfer `amount` tokens from the Employer to the contract, store a new `EscrowTask` in Temporary storage with status `Open`, and return the assigned `task_id`.

**Preconditions:**
- `amount > 0`
- `deadline >= ledger_timestamp + DEADLINE_WINDOW`
- Employer has approved the contract to spend `amount` of `token`

**Postconditions:**
- `EscrowTask` stored at `DataKey::Task(task_id)` with `status = Open`
- `amount` tokens held by contract
- `TaskCounter` incremented

**Error cases:**
- `InvalidAmount` — amount ≤ 0
- `DeadlineTooSoon` — deadline < now + DEADLINE_WINDOW
- `TransferFailed` — token transfer reverts

---

### 5.2 claim_reward

> **EARS (Event-driven):** WHEN the designated Worker invokes `claim_reward` with a `task_id` and a Groth16 ZK-proof, the system SHALL verify the proof against the stored `vk_hash` using the BN254 `pairing_check` host function, and IF the proof is valid THEN transfer `amount` tokens to the Worker and set task status to `Claimed`.

**Preconditions:**
- Caller is `task.worker`
- `task.status == Open`
- `ledger_timestamp <= task.deadline`
- ZK-proof is a valid Groth16 proof over BN254 for the circuit committed to by `vk_hash`

**ZK Verification (Protocol 25 BN254):**

The proof is verified via the Soroban host's `pairing_check` function:

```
pairing_check(
    g1_points: [proof.a, vk.alpha],
    g2_points: [proof.b, vk.beta],
    // + public inputs pairing terms
) == true
```

The contract stores only `vk_hash = sha256(vk_bytes)`. The Worker supplies the full `vk_bytes` and `proof_bytes`; the contract recomputes `sha256(vk_bytes)` and asserts equality before calling `pairing_check`.

**Postconditions:**
- `task.status = Claimed`
- `amount` tokens transferred to Worker

**Error cases:**
- `Unauthorized` — caller ≠ worker
- `TaskNotOpen` — status ≠ Open
- `DeadlineExpired` — timestamp > deadline
- `InvalidProof` — pairing check fails
- `VkMismatch` — sha256(vk_bytes) ≠ vk_hash

---

### 5.3 refund

> **EARS (Unwanted behaviour):** IF the task deadline has passed AND the task status is still `Open`, WHEN the Employer invokes `refund` with the `task_id`, the system SHALL transfer `amount` tokens back to the Employer and set task status to `Refunded`.

**Preconditions:**
- Caller is `task.employer`
- `task.status == Open`
- `ledger_timestamp > task.deadline`

**Postconditions:**
- `task.status = Refunded`
- `amount` tokens returned to Employer

**Error cases:**
- `Unauthorized` — caller ≠ employer
- `TaskNotOpen` — status ≠ Open
- `DeadlineNotReached` — timestamp ≤ deadline

---

## 6. Events

| Event          | Data                              | Emitted by       |
|----------------|-----------------------------------|------------------|
| `task_created` | `task_id, employer, worker, amount, deadline` | `create_task` |
| `reward_claimed` | `task_id, worker, amount`       | `claim_reward`   |
| `task_refunded` | `task_id, employer, amount`     | `refund`         |

---

## 7. Security Considerations

- **Replay protection:** Each `task_id` is unique and monotonically increasing; a claimed/refunded task cannot be re-entered.
- **ZK soundness:** The contract never trusts off-chain proof validity; `pairing_check` is the sole arbiter.
- **Reentrancy:** Token transfers use the SAC interface which is atomic within the Soroban VM; no external call occurs after state mutation.
- **Temporary storage expiry:** If a task's TTL expires before claim or refund, the entry is archived. The Employer must restore it (pay rent) before calling `refund`. This is an accepted trade-off for low-cost storage.

---

## 8. Out of Scope (v1.0)

- Partial payments / milestone-based escrow
- Multi-worker tasks
- On-chain VK registry — a separate contract mapping `vk_hash → vk_bytes` so workers register a VK once rather than supplying it on every `claim_reward` call
- Dispute resolution
