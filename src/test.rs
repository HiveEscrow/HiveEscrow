#![cfg(test)]

use soroban_sdk::{
    crypto::bn254::{Bn254G1Affine, Bn254G2Affine},
    testutils::{Address as _, Ledger},
    token, Address, Bytes, BytesN, Env,
};

use crate::contract::{Error, HiveEscrow, HiveEscrowClient, Proof, VerifyingKey};
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

fn g1(env: &Env, x_last: u8, y_last: u8) -> Bn254G1Affine {
    let mut buf = [0u8; 64];
    buf[31] = x_last;
    buf[63] = y_last;
    unsafe { Bn254G1Affine::from_bytes(BytesN::from_array(env, &buf)) }
}

fn g2_generator(env: &Env) -> Bn254G2Affine {
    let mut buf = [0u8; 128];
    // BN254 G2 generator (big-endian x_c1, x_c0, y_c1, y_c0)
    let coords: [[u8; 32]; 4] = [
        hex32("1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed"),
        hex32("198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2"),
        hex32("12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa"),
        hex32("090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b"),
    ];
    for (i, c) in coords.iter().enumerate() {
        buf[i * 32..(i + 1) * 32].copy_from_slice(c);
    }
    unsafe { Bn254G2Affine::from_bytes(BytesN::from_array(env, &buf)) }
}

fn hex32(s: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (i, c) in s.as_bytes().chunks(2).enumerate() {
        out[i] = (nibble(c[0]) << 4) | nibble(c[1]);
    }
    out
}

fn nibble(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        _ => 0,
    }
}

fn dummy_proof(env: &Env) -> Proof {
    Proof {
        a: g1(env, 1, 2),
        b: g2_generator(env),
        c: g1(env, 3, 4),
    }
}

fn dummy_vk_struct(env: &Env) -> VerifyingKey {
    VerifyingKey {
        alpha: g1(env, 5, 6),
        beta: g2_generator(env),
        gamma: g2_generator(env),
        delta: g2_generator(env),
        ic: g1(env, 7, 8),
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
    assert!(client.get_task(&task_id).is_some());
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
        .try_create_task(
            &employer,
            &worker,
            &token,
            &100,
            &vk_hash,
            &(env.ledger().timestamp() + 1),
        )
        .unwrap_err()
        .unwrap();

    assert_eq!(err, Error::DeadlineTooSoon);
}

// ── get_task ──────────────────────────────────────────────────────────────────

#[test]
fn test_get_task_not_found() {
    let (env, client, _, _, _) = setup();
    assert!(client.get_task(&99).is_none());
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

    let wrong_vk_bytes = Bytes::from_array(&env, &[0xFFu8; 32]);
    let err = client
        .try_claim_reward(
            &worker,
            &task_id,
            &wrong_vk_bytes,
            &dummy_vk_struct(&env),
            &dummy_proof(&env),
        )
        .unwrap_err()
        .unwrap();

    assert_eq!(err, Error::VkMismatch);
}

#[test]
fn test_claim_after_deadline_fails() {
    let (env, client, employer, worker, token) = setup();
    let (vk_bytes, vk_hash) = dummy_vk(&env);
    let deadline = valid_deadline(&env);

    let task_id = client
        .create_task(&employer, &worker, &token, &500_000, &vk_hash, &deadline)
        .unwrap();

    env.ledger().set_timestamp(deadline + 1);

    let err = client
        .try_claim_reward(
            &worker,
            &task_id,
            &vk_bytes,
            &dummy_vk_struct(&env),
            &dummy_proof(&env),
        )
        .unwrap_err()
        .unwrap();

    assert_eq!(err, Error::DeadlineExpired);
}

#[test]
fn test_claim_wrong_worker_fails() {
    let (env, client, employer, worker, token) = setup();
    let (vk_bytes, vk_hash) = dummy_vk(&env);
    let deadline = valid_deadline(&env);

    let task_id = client
        .create_task(&employer, &worker, &token, &500_000, &vk_hash, &deadline)
        .unwrap();

    let impostor = Address::generate(&env);
    let err = client
        .try_claim_reward(
            &impostor,
            &task_id,
            &vk_bytes,
            &dummy_vk_struct(&env),
            &dummy_proof(&env),
        )
        .unwrap_err()
        .unwrap();

    assert_eq!(err, Error::Unauthorized);
}

#[test]
fn test_claim_task_not_found() {
    let (env, client, _, worker, _) = setup();
    let (vk_bytes, _) = dummy_vk(&env);

    let err = client
        .try_claim_reward(
            &worker,
            &999,
            &vk_bytes,
            &dummy_vk_struct(&env),
            &dummy_proof(&env),
        )
        .unwrap_err()
        .unwrap();

    assert_eq!(err, Error::TaskNotFound);
}
