use soroban_sdk::{
    contract, contractimpl, contracttype,
    crypto::bn254::{Bn254Fr, Bn254G1Affine, Bn254G2Affine},
    token, Address, Bytes, BytesN, Env, Vec,
};

use crate::storage::{
    bump_instance, bump_task, get_counter, load_task, save_task, set_counter, EscrowTask,
    TaskStatus, DEADLINE_WINDOW,
};

/// Groth16 proof over BN254.
#[contracttype]
#[derive(Clone)]
pub struct Proof {
    pub a: Bn254G1Affine, // π_A (G1)
    pub b: Bn254G2Affine, // π_B (G2)
    pub c: Bn254G1Affine, // π_C (G1)
}

/// Groth16 verification key.
/// `ic` must have length == number_of_public_inputs + 1.
/// IC[0] is the base point; IC[1..] are per-input points.
#[contracttype]
#[derive(Clone)]
pub struct VerifyingKey {
    pub alpha: Bn254G1Affine,    // α   (G1)
    pub beta: Bn254G2Affine,     // β   (G2)
    pub gamma: Bn254G2Affine,    // γ   (G2)
    pub delta: Bn254G2Affine,    // δ   (G2)
    pub ic: Vec<Bn254G1Affine>,  // IC[0..n] (G1)
}

/// Public inputs as 32-byte big-endian BN254 scalar field elements.
/// Length must equal vk.ic.len() - 1.
pub type PublicInputs = Vec<BytesN<32>>;

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
    InvalidPublicInputs = 10,
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
    /// Verification steps:
    ///   1. sha256(vk_bytes) == task.vk_hash
    ///   2. ic_combined = IC[0] + Σ(public_inputs[i] * IC[i+1])  via g1_msm
    ///   3. pairing_check: e(A,B) · e(α,β) · e(C,δ) · e(ic_combined,γ) == 1
    pub fn claim_reward(
        env: Env,
        worker: Address,
        task_id: u64,
        vk_bytes: Bytes,
        vk: VerifyingKey,
        proof: Proof,
        public_inputs: PublicInputs,
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

        // 1. Verify VK commitment
        let computed: BytesN<32> = env.crypto().sha256(&vk_bytes).to_bytes();
        if computed != task.vk_hash {
            return Err(Error::VkMismatch);
        }

        // 2. Validate IC / public input lengths: ic.len() == public_inputs.len() + 1
        let n_inputs = public_inputs.len();
        if vk.ic.len() != n_inputs + 1 {
            return Err(Error::InvalidPublicInputs);
        }

        // 3. Fold public inputs: ic_combined = IC[0] + Σ(input[i] * IC[i+1])
        //    Use g1_msm for the sum, then add IC[0].
        let bn254 = env.crypto().bn254();

        let ic_combined = if n_inputs == 0 {
            vk.ic.get(0).unwrap()
        } else {
            // Build parallel vecs: IC[1..] and scalars from public_inputs
            let mut ic_points: Vec<Bn254G1Affine> = Vec::new(&env);
            let mut scalars: Vec<Bn254Fr> = Vec::new(&env);
            for i in 0..n_inputs {
                ic_points.push_back(vk.ic.get(i + 1).unwrap());
                scalars.push_back(Bn254Fr::from_bytes(public_inputs.get(i).unwrap()));
            }
            // IC[0] + MSM(IC[1..], inputs)
            bn254.g1_add(&vk.ic.get(0).unwrap(), &bn254.g1_msm(ic_points, scalars))
        };

        // 4. Groth16 pairing check: e(A,B) · e(α,β) · e(C,δ) · e(ic_combined,γ) == 1
        //    Negate A to match the standard equation form used by most toolchains:
        //    e(-A,B) · e(α,β) · e(C,δ) · e(ic_combined,γ) == 1
        let g1 = soroban_sdk::vec![&env, -proof.a, vk.alpha, proof.c, ic_combined];
        let g2 = soroban_sdk::vec![&env, proof.b, vk.beta, vk.delta, vk.gamma];

        if !bn254.pairing_check(g1, g2) {
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
