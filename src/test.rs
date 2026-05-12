#![cfg(test)]

use soroban_sdk::{
    crypto::bn254::{Bn254G1Affine, Bn254G2Affine},
    testutils::{Address as _, Ledger},
    token, Address, Bytes, BytesN, Env, Vec,
};

use crate::contract::{Error, HiveEscrow, HiveEscrowClient};
use crate::storage::DEADLINE_WINDOW;

// ── helpers ──────────────────────────────────────────────────────────────────

fn setup() -> (Env, HiveEscrowClient<'static>, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, HiveEscrow);
    let client = HiveEscrowClient::new(&env, &contract_id);

    let employer = Address::generate(&env);
    let worker = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract(token_admin.clone());
    token::StellarAssetClient::new(&env, &token_id).mint(&employer, &1_000_000);

    (env, client, employer, worker, token_id)
}

fn dummy_vk(env: &Env) -> (Bytes, BytesN<32>) {
    let vk = Bytes::from_array(env, &[0xABu8; 32]);
    let hash: BytesN<32> = env.crypto().sha256(&vk).to_bytes();
    (vk, hash)
}

/// Minimal valid BN254 G1 point (generator): x=1, y=2 (64 bytes, big-endian)
fn g1_generator(env: &Env) -> Bn254G1Affine {
    let mut buf = [0u8; 64];
    buf[31] = 1; // x = 1
    buf[63] = 2; // y = 2
    unsafe { Bn254G1Affine::from_bytes(BytesN::from_array(env, &buf)) }
}

/// Minimal valid BN254 G2 point (generator): 128 bytes
fn g2_generator(env: &Env) -> Bn254G2Affine {
    // BN254 G2 generator coordinates (big-endian, 128 bytes)
    let x_c1: [u8; 32] = hex_to_bytes(
        "1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed",
    );
    let x_c0: [u8; 32] = hex_to_bytes(
        "198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2",
    );
    let y_c1: [u8; 32] = hex_to_bytes(
        "12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa",
    );
    let y_c0: [u8; 32] = hex_to_bytes(
        "090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b",
    );
    let mut buf = [0u8; 128];
    buf[0..32].copy_from_slice(&x_c1);
    buf[32..64].copy_from_slice(&x_c0);
    buf[64..96].copy_from_slice(&y_c1);
    buf[96..128].copy_from_slice(&y_c0);
    unsafe { Bn254G2Affine::from_bytes(BytesN::from_array(env, &buf)) }
}

fn hex_to_bytes(hex: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let hi = hex_nibble(chunk[0]);
        let lo = hex_nibble(chunk[1]);
        out[i] = (hi << 4) | lo;
    }
    out
}

fn hex_nibble(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        _ => 0,
    }
}

fn valid_deadline(env: &Env) -> u64 {
    env.ledger().timestamp() + DEADLINE_WINDOW + 1
}

// ── create_task ───────────────────────────────────────────────────────────────

#[test]
fn test_create_task_success() {
    let (env, client, employer, worker, token) = setup();
    let (_, vk_hash) = dummy_vk(&env);

    let task_id = client
        .create_task(&employer, &worker, &token, &500_000, &vk_hash, &valid_deadline(&env))
        .unwrap();

    assert_eq!(task_id, 0);
}

#[test]
fn test_create_task_invalid_amount() {
    let (env, client, employer, worker, token) = setup();
    let (_, vk_hash) = dummy_vk(&env);

    let err = client
        .try_create_task(&employer, &worker, &token, &0, &vk_hash, &valid_deadline(&env))
        .unwrap_err()
        .unwrap();

    assert_eq!(err, Error::InvalidAmount);
}

#[test]
fn test_create_task_deadline_too_soon() {
    let (env, client, employer, worker, token) = setup();
    let (_, vk_hash) = dummy_vk(&env);

    let err = client
        .try_create_task(&employer, &worker, &token, &100, &vk_hash, &(env.ledger().timestamp() + 1))
        .unwrap_err()
        .unwrap();

    assert_eq!(err, Error::DeadlineTooSoon);
}

// ── refund ────────────────────────────────────────────────────────────────────

#[test]
fn test_refund_after_deadline() {
    let (env, client, employer, worker, token) = setup();
    let (_, vk_hash) = dummy_vk(&env);
    let deadline = valid_deadline(&env);

    let task_id = client
        .create_task(&employer, &worker, &token, &500_000, &vk_hash, &deadline)
        .unwrap();

    env.ledger().set_timestamp(deadline + 1);
    client.refund(&employer, &task_id).unwrap();

    assert_eq!(token::Client::new(&env, &token).balance(&employer), 1_000_000);
}

#[test]
fn test_refund_before_deadline_fails() {
    let (env, client, employer, worker, token) = setup();
    let (_, vk_hash) = dummy_vk(&env);
    let deadline = valid_deadline(&env);

    let task_id = client
        .create_task(&employer, &worker, &token, &500_000, &vk_hash, &deadline)
        .unwrap();

    let err = client.try_refund(&employer, &task_id).unwrap_err().unwrap();
    assert_eq!(err, Error::DeadlineNotReached);
}

#[test]
fn test_refund_wrong_caller_fails() {
    let (env, client, employer, worker, token) = setup();
    let (_, vk_hash) = dummy_vk(&env);
    let deadline = valid_deadline(&env);

    let task_id = client
        .create_task(&employer, &worker, &token, &500_000, &vk_hash, &deadline)
        .unwrap();

    env.ledger().set_timestamp(deadline + 1);
    let err = client.try_refund(&worker, &task_id).unwrap_err().unwrap();
    assert_eq!(err, Error::Unauthorized);
}

#[test]
fn test_double_refund_fails() {
    let (env, client, employer, worker, token) = setup();
    let (_, vk_hash) = dummy_vk(&env);
    let deadline = valid_deadline(&env);

    let task_id = client
        .create_task(&employer, &worker, &token, &500_000, &vk_hash, &deadline)
        .unwrap();

    env.ledger().set_timestamp(deadline + 1);
    client.refund(&employer, &task_id).unwrap();

    let err = client.try_refund(&employer, &task_id).unwrap_err().unwrap();
    assert_eq!(err, Error::TaskNotOpen);
}

// ── claim_reward ──────────────────────────────────────────────────────────────

#[test]
fn test_claim_vk_mismatch_fails() {
    let (env, client, employer, worker, token) = setup();
    let (_, vk_hash) = dummy_vk(&env);
    let deadline = valid_deadline(&env);

    let task_id = client
        .create_task(&employer, &worker, &token, &500_000, &vk_hash, &deadline)
        .unwrap();

    let wrong_vk = Bytes::from_array(&env, &[0xFFu8; 32]);
    let g1 = Vec::from_array(&env, [g1_generator(&env)]);
    let g2 = Vec::from_array(&env, [g2_generator(&env)]);

    let err = client
        .try_claim_reward(&worker, &task_id, &wrong_vk, &g1, &g2)
        .unwrap_err()
        .unwrap();

    assert_eq!(err, Error::VkMismatch);
}

#[test]
fn test_claim_after_deadline_fails() {
    let (env, client, employer, worker, token) = setup();
    let (vk, vk_hash) = dummy_vk(&env);
    let deadline = valid_deadline(&env);

    let task_id = client
        .create_task(&employer, &worker, &token, &500_000, &vk_hash, &deadline)
        .unwrap();

    env.ledger().set_timestamp(deadline + 1);

    let g1 = Vec::from_array(&env, [g1_generator(&env)]);
    let g2 = Vec::from_array(&env, [g2_generator(&env)]);

    let err = client
        .try_claim_reward(&worker, &task_id, &vk, &g1, &g2)
        .unwrap_err()
        .unwrap();

    assert_eq!(err, Error::DeadlineExpired);
}

#[test]
fn test_claim_wrong_worker_fails() {
    let (env, client, employer, worker, token) = setup();
    let (vk, vk_hash) = dummy_vk(&env);
    let deadline = valid_deadline(&env);

    let task_id = client
        .create_task(&employer, &worker, &token, &500_000, &vk_hash, &deadline)
        .unwrap();

    let impostor = Address::generate(&env);
    let g1 = Vec::from_array(&env, [g1_generator(&env)]);
    let g2 = Vec::from_array(&env, [g2_generator(&env)]);

    let err = client
        .try_claim_reward(&impostor, &task_id, &vk, &g1, &g2)
        .unwrap_err()
        .unwrap();

    assert_eq!(err, Error::Unauthorized);
}
