use soroban_sdk::{
    contract, contractimpl,
    crypto::bn254::{Bn254G1Affine, Bn254G2Affine},
    token, Address, Bytes, BytesN, Env, Vec,
};

use crate::storage::{
    bump_task, get_counter, load_task, save_task, set_counter, EscrowTask, TaskStatus,
    DEADLINE_WINDOW,
};

#[derive(Debug)]
#[soroban_sdk::contracterror]
pub enum Error {
    InvalidAmount = 1,
    DeadlineTooSoon = 2,
    Unauthorized = 3,
    TaskNotOpen = 4,
    DeadlineExpired = 5,
    InvalidProof = 6,
    VkMismatch = 7,
    DeadlineNotReached = 8,
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

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let now = env.ledger().timestamp();
        if deadline < now + DEADLINE_WINDOW {
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
    /// `vk_bytes`     – serialized verification key; sha256 must match stored vk_hash
    /// `proof_g1`     – [proof.A, vk.alpha, ...public_input_G1_terms]
    /// `proof_g2`     – [proof.B, vk.beta,  ...public_input_G2_terms]
    ///
    /// The pairing check verifies: e(A,B) · e(alpha,beta) · Σe(Pi,γi) == 1
    pub fn claim_reward(
        env: Env,
        worker: Address,
        task_id: u64,
        vk_bytes: Bytes,
        proof_g1: Vec<Bn254G1Affine>,
        proof_g2: Vec<Bn254G2Affine>,
    ) -> Result<(), Error> {
        worker.require_auth();

        let mut task = load_task(&env, task_id);

        if task.worker != worker {
            return Err(Error::Unauthorized);
        }
        if task.status != TaskStatus::Open {
            return Err(Error::TaskNotOpen);
        }
        if env.ledger().timestamp() > task.deadline {
            return Err(Error::DeadlineExpired);
        }

        // Verify vk_hash matches supplied vk_bytes before touching the pairing check
        let computed: BytesN<32> = env.crypto().sha256(&vk_bytes).to_bytes();
        if computed != task.vk_hash {
            return Err(Error::VkMismatch);
        }

        // BN254 multi-pairing check (Protocol 25 host function)
        if !env.crypto().bn254().pairing_check(proof_g1, proof_g2) {
            return Err(Error::InvalidProof);
        }

        // Settle: mutate state before external transfer
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

        let mut task = load_task(&env, task_id);

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
}
