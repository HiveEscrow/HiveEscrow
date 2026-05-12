use soroban_sdk::{
    contract, contractimpl, contracttype,
    crypto::bn254::{Bn254G1Affine, Bn254G2Affine},
    token, Address, Bytes, BytesN, Env,
};

use crate::storage::{
    bump_instance, bump_task, get_counter, load_task, save_task, set_counter, EscrowTask,
    TaskStatus, DEADLINE_WINDOW,
};

/// Groth16 proof over BN254: the three elliptic curve points.
#[contracttype]
#[derive(Clone)]
pub struct Proof {
    pub a: Bn254G1Affine, // π_A  (G1)
    pub b: Bn254G2Affine, // π_B  (G2)
    pub c: Bn254G1Affine, // π_C  (G1)
}

/// Groth16 verification key components needed for the pairing check.
/// Caller supplies this alongside the proof; the contract verifies
/// sha256(vk_bytes) == stored vk_hash before using it.
#[contracttype]
#[derive(Clone)]
pub struct VerifyingKey {
    pub alpha: Bn254G1Affine, // α   (G1)
    pub beta: Bn254G2Affine,  // β   (G2)
    pub gamma: Bn254G2Affine, // γ   (G2)
    pub delta: Bn254G2Affine, // δ   (G2)
    pub ic: Bn254G1Affine,    // IC[0] (G1) — combined with public inputs off-chain
}

#[soroban_sdk::contracterror]
#[derive(Debug)]
pub enum Error {
    InvalidAmount = 1,
    DeadlineTooSoon = 2,
    Unauthorized = 3,
    TaskNotOpen = 4,
    DeadlineExpired = 5,
    InvalidProof = 6,
    VkMismatch = 7,
    DeadlineNotReached = 8,
    TaskNotFound = 9,
}

#[contract]
pub struct HiveEscrow;

#[contractimpl]
impl HiveEscrow {
    /// Employer deposits funds and registers a task.
    pub fn create_task(
        env: Env,
        employer: Address,
        worker: Address,
        token: Address,
        amount: i128,
        vk_hash: BytesN<32>,
        deadline: u64,
    ) -> Result<u64, Error> {
        employer.require_auth();
        bump_instance(&env);

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        if deadline < env.ledger().timestamp() + DEADLINE_WINDOW {
            return Err(Error::DeadlineTooSoon);
        }

        token::Client::new(&env, &token).transfer(
            &employer,
            &env.current_contract_address(),
            &amount,
        );

        let task_id = get_counter(&env);
        set_counter(&env, task_id + 1);

        save_task(
            &env,
            task_id,
            &EscrowTask {
                employer: employer.clone(),
                worker: worker.clone(),
                token,
                amount,
                vk_hash,
                deadline,
                status: TaskStatus::Open,
            },
        );

        env.events().publish(
            (soroban_sdk::symbol_short!("task_crtd"), task_id),
            (employer, worker, amount, deadline),
        );

        Ok(task_id)
    }

    /// Worker submits a Groth16 ZK-proof to claim the reward.
    ///
    /// The contract assembles the pairing check internally from the typed
    /// `Proof` and `VerifyingKey` structs, eliminating caller assembly errors.
    ///
    /// Pairing equation verified:
    ///   e(A, B) · e(α, β) · e(C, δ) · e(ic_combined, γ) == 1
    pub fn claim_reward(
        env: Env,
        worker: Address,
        task_id: u64,
        vk_bytes: Bytes,
        vk: VerifyingKey,
        proof: Proof,
    ) -> Result<(), Error> {
        worker.require_auth();
        bump_instance(&env);

        let mut task = load_task(&env, task_id).ok_or(Error::TaskNotFound)?;

        if task.worker != worker {
            return Err(Error::Unauthorized);
        }
        if task.status != TaskStatus::Open {
            return Err(Error::TaskNotOpen);
        }
        if env.ledger().timestamp() > task.deadline {
            return Err(Error::DeadlineExpired);
        }

        // Verify the supplied VK matches the committed hash
        let computed: BytesN<32> = env.crypto().sha256(&vk_bytes).to_bytes();
        if computed != task.vk_hash {
            return Err(Error::VkMismatch);
        }

        // Groth16 pairing check via BN254 host function (Protocol 25)
        // e(A,B) · e(α,β) · e(C,δ) · e(ic,γ) == 1
        // Assembled as two parallel vectors of equal length.
        let g1 = soroban_sdk::vec![&env, proof.a, vk.alpha, proof.c, vk.ic];
        let g2 = soroban_sdk::vec![&env, proof.b, vk.beta, vk.delta, vk.gamma];

        if !env.crypto().bn254().pairing_check(g1, g2) {
            return Err(Error::InvalidProof);
        }

        // Mutate state before external transfer
        task.status = TaskStatus::Claimed;
        bump_task(&env, task_id);
        save_task(&env, task_id, &task);

        token::Client::new(&env, &task.token).transfer(
            &env.current_contract_address(),
            &worker,
            &task.amount,
        );

        env.events().publish(
            (soroban_sdk::symbol_short!("rewrd_clmd"), task_id),
            (worker, task.amount),
        );

        Ok(())
    }

    /// Employer reclaims deposit after deadline passes without a valid claim.
    pub fn refund(env: Env, employer: Address, task_id: u64) -> Result<(), Error> {
        employer.require_auth();
        bump_instance(&env);

        let mut task = load_task(&env, task_id).ok_or(Error::TaskNotFound)?;

        if task.employer != employer {
            return Err(Error::Unauthorized);
        }
        if task.status != TaskStatus::Open {
            return Err(Error::TaskNotOpen);
        }
        if env.ledger().timestamp() <= task.deadline {
            return Err(Error::DeadlineNotReached);
        }

        task.status = TaskStatus::Refunded;
        bump_task(&env, task_id);
        save_task(&env, task_id, &task);

        token::Client::new(&env, &task.token).transfer(
            &env.current_contract_address(),
            &employer,
            &task.amount,
        );

        env.events().publish(
            (soroban_sdk::symbol_short!("task_rfnd"), task_id),
            (employer, task.amount),
        );

        Ok(())
    }

    /// Read-only view of a task. Returns None if archived or non-existent.
    pub fn get_task(env: Env, task_id: u64) -> Option<EscrowTask> {
        load_task(&env, task_id)
    }
}
