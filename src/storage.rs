use soroban_sdk::{contracttype, Address, BytesN, Env};

pub const TASK_TTL_LEDGERS: u32 = 120_960; // ~7 days @ 5s/ledger
pub const BUMP_TTL_LEDGERS: u32 = 17_280;  // ~1 day threshold
pub const DEADLINE_WINDOW: u64 = 172_800;  // 48 hours in seconds

#[contracttype]
#[derive(Clone, PartialEq)]
pub enum TaskStatus {
    Open,
    Claimed,
    Refunded,
}

#[contracttype]
#[derive(Clone)]
pub struct EscrowTask {
    pub employer: Address,
    pub worker: Address,
    pub token: Address,
    pub amount: i128,
    pub vk_hash: BytesN<32>,
    pub deadline: u64,
    pub status: TaskStatus,
}

#[contracttype]
pub enum DataKey {
    Task(u64),
    TaskCounter,
}

pub fn get_counter(env: &Env) -> u64 {
    env.storage().instance().get(&DataKey::TaskCounter).unwrap_or(0u64)
}

pub fn set_counter(env: &Env, counter: u64) {
    env.storage().instance().set(&DataKey::TaskCounter, &counter);
}

pub fn save_task(env: &Env, task_id: u64, task: &EscrowTask) {
    env.storage()
        .temporary()
        .set(&DataKey::Task(task_id), task);
    env.storage()
        .temporary()
        .extend_ttl(&DataKey::Task(task_id), BUMP_TTL_LEDGERS, TASK_TTL_LEDGERS);
}

pub fn load_task(env: &Env, task_id: u64) -> EscrowTask {
    env.storage()
        .temporary()
        .get(&DataKey::Task(task_id))
        .expect("task not found")
}

pub fn bump_task(env: &Env, task_id: u64) {
    env.storage()
        .temporary()
        .extend_ttl(&DataKey::Task(task_id), BUMP_TTL_LEDGERS, TASK_TTL_LEDGERS);
}
