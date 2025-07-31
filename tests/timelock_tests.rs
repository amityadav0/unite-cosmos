use cosmwasm_std::{Addr, Coin, Uint128};
use cw_multi_test::{App, Contract, ContractWrapper, Executor};
use escrow_contract::msg::{ExecuteMsg, InstantiateMsg};
use escrow_contract::state::{TimelockStage, PackedTimelocks, EscrowType};
use sha2::{Sha256, Digest};
use std::time::{SystemTime, UNIX_EPOCH};

fn escrow_contract() -> Box<dyn Contract<cosmwasm_std::Empty>> {
    let contract = ContractWrapper::new(
        escrow_contract::execute,
        escrow_contract::instantiate,
        escrow_contract::query,
    );
    Box::new(contract)
}

fn mock_app() -> App {
    App::new(|router, _api, storage| {
        router.bank.init_balance(storage, &Addr::unchecked("owner"), vec![Coin::new(10000, "uatom")]).unwrap();
        router.bank.init_balance(storage, &Addr::unchecked("access_token"), vec![Coin::new(1000, "uatom")]).unwrap();
        router.bank.init_balance(storage, &Addr::unchecked("maker"), vec![Coin::new(2000, "uatom")]).unwrap();
        router.bank.init_balance(storage, &Addr::unchecked("taker"), vec![Coin::new(2000, "uatom")]).unwrap();
    })
}

fn create_test_timelocks() -> PackedTimelocks {
    PackedTimelocks::new(
        1000, // deployed_at
        1,    // src_withdrawal: 1 hour
        2,    // src_public_withdrawal: 2 hours
        3,    // src_cancellation: 3 hours
        4,    // src_public_cancellation: 4 hours
        1,    // dst_withdrawal: 1 hour
        2,    // dst_public_withdrawal: 2 hours
        3,    // dst_cancellation: 3 hours
    )
}

fn generate_secret() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("secret_{}", timestamp)
}

fn hash_secret(secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn setup_contract() -> (App, Addr) {
    let mut app = mock_app();
    let contract_id = app.store_code(escrow_contract());

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        access_token: "access_token".to_string(),
        rescue_delay: 3600,
    };

    let contract_addr = app
        .instantiate_contract(contract_id, Addr::unchecked("owner"), &msg, &[], "Escrow", None)
        .unwrap();

    (app, contract_addr)
}

#[test]
fn test_timelock_stage_enum() {
    // Test TimelockStage enum values and bit offsets
    assert_eq!(TimelockStage::SrcWithdrawal.bit_offset(), 0);
    assert_eq!(TimelockStage::SrcPublicWithdrawal.bit_offset(), 1);
    assert_eq!(TimelockStage::SrcCancellation.bit_offset(), 2);
    assert_eq!(TimelockStage::SrcPublicCancellation.bit_offset(), 3);
    assert_eq!(TimelockStage::DstWithdrawal.bit_offset(), 4);
    assert_eq!(TimelockStage::DstPublicWithdrawal.bit_offset(), 5);
    assert_eq!(TimelockStage::DstCancellation.bit_offset(), 6);

    // Test stage type classification
    assert!(TimelockStage::SrcWithdrawal.is_source());
    assert!(TimelockStage::SrcPublicWithdrawal.is_source());
    assert!(TimelockStage::DstWithdrawal.is_destination());
    assert!(TimelockStage::DstPublicWithdrawal.is_destination());

    // Test public/private classification
    assert!(TimelockStage::SrcWithdrawal.is_private());
    assert!(TimelockStage::SrcPublicWithdrawal.is_public());
    assert!(TimelockStage::DstWithdrawal.is_private());
    assert!(TimelockStage::DstPublicWithdrawal.is_public());
}

#[test]
fn test_packed_timelocks_creation() {
    let timelocks = create_test_timelocks();

    // Test deployed_at extraction
    assert_eq!(timelocks.deployed_at(), 1000);

    // Test individual timelock extraction
    assert_eq!(timelocks.get(TimelockStage::SrcWithdrawal), 1);
    assert_eq!(timelocks.get(TimelockStage::SrcPublicWithdrawal), 2);
    assert_eq!(timelocks.get(TimelockStage::SrcCancellation), 3);
    assert_eq!(timelocks.get(TimelockStage::SrcPublicCancellation), 4);
    assert_eq!(timelocks.get(TimelockStage::DstWithdrawal), 1);
    assert_eq!(timelocks.get(TimelockStage::DstPublicWithdrawal), 2);
    assert_eq!(timelocks.get(TimelockStage::DstCancellation), 3);

    // Test validation
    assert!(timelocks.validate().is_ok());
}

#[test]
fn test_timelock_stage_calculations() {
    let timelocks = create_test_timelocks();
    let _deployed_at = timelocks.deployed_at() as u64;

    // Test stage time calculations
    let src_withdrawal_time = timelocks.get_stage_time(TimelockStage::SrcWithdrawal);
    assert_eq!(src_withdrawal_time, _deployed_at + (1 * 3600)); // 1 hour in seconds

    let src_public_withdrawal_time = timelocks.get_stage_time(TimelockStage::SrcPublicWithdrawal);
    assert_eq!(src_public_withdrawal_time, _deployed_at + (2 * 3600)); // 2 hours in seconds

    // Test stage validation at different times
    let current_time_before = _deployed_at + 1800; // 30 minutes after deployment
    let current_time_during = _deployed_at + 3600; // 1 hour after deployment
    let current_time_after = _deployed_at + 7200;  // 2 hours after deployment

    // Before src_withdrawal stage
    assert!(!timelocks.is_within_stage(current_time_before, TimelockStage::SrcWithdrawal));
    assert!(!timelocks.is_within_stage(current_time_before, TimelockStage::SrcPublicWithdrawal));

    // During src_withdrawal stage
    assert!(timelocks.is_within_stage(current_time_during, TimelockStage::SrcWithdrawal));
    assert!(!timelocks.is_within_stage(current_time_during, TimelockStage::SrcPublicWithdrawal));

    // After src_public_withdrawal stage
    assert!(timelocks.is_within_stage(current_time_after, TimelockStage::SrcWithdrawal));
    assert!(timelocks.is_within_stage(current_time_after, TimelockStage::SrcPublicWithdrawal));
}

#[test]
fn test_withdrawal_timelock_validation() {
    let (mut app, contract_addr) = setup_contract();

    // Deploy escrow
    let secret = generate_secret();
    let hashlock = hash_secret(&secret);

    let deploy_msg = ExecuteMsg::DeployEscrowWithFunding {
        order_hash: "test_order_hash_123".to_string(),
        hashlock: hashlock.clone(),
        maker: "maker".to_string(),
        taker: "taker".to_string(),
        token: "".to_string(),
        amount: Uint128::new(1000),
        safety_deposit: Uint128::new(100),
        timelocks: create_test_timelocks(),
        dst_chain_id: "cosmoshub-4".to_string(),
        dst_token: "dst_token".to_string(),
        dst_amount: Uint128::new(1000),
        escrow_type: EscrowType::Source,
    };

    let funds = vec![Coin::new(1100, "uatom")];
    app.execute_contract(
        Addr::unchecked("taker"),
        contract_addr.clone(),
        &deploy_msg,
        &funds,
    ).unwrap();

    // Try to withdraw before timelock period
    let withdraw_msg = ExecuteMsg::WithdrawSrc {
        escrow_id: 1,
        secret: secret.clone(),
    };

    let result = app.execute_contract(
        Addr::unchecked("taker"),
        contract_addr.clone(),
        &withdraw_msg,
        &[],
    );

    // Should fail due to timelock
    assert!(result.is_err());
}

#[test]
fn test_public_withdrawal_timelock() {
    let (mut app, contract_addr) = setup_contract();

    // Deploy escrow
    let secret = generate_secret();
    let hashlock = hash_secret(&secret);

    let deploy_msg = ExecuteMsg::DeployEscrowWithFunding {
        order_hash: "test_order_hash_123".to_string(),
        hashlock: hashlock.clone(),
        maker: "maker".to_string(),
        taker: "taker".to_string(),
        token: "".to_string(),
        amount: Uint128::new(1000),
        safety_deposit: Uint128::new(100),
        timelocks: create_test_timelocks(),
        dst_chain_id: "cosmoshub-4".to_string(),
        dst_token: "dst_token".to_string(),
        dst_amount: Uint128::new(1000),
        escrow_type: EscrowType::Source,
    };

    let funds = vec![Coin::new(1100, "uatom")];
    app.execute_contract(
        Addr::unchecked("taker"),
        contract_addr.clone(),
        &deploy_msg,
        &funds,
    ).unwrap();

    // Try to public withdraw before timelock period
    let public_withdraw_msg = ExecuteMsg::PublicWithdrawSrc { escrow_id: 1 };

    let result = app.execute_contract(
        Addr::unchecked("access_token"),
        contract_addr.clone(),
        &public_withdraw_msg,
        &[],
    );

    // Should fail due to timelock
    assert!(result.is_err());
}

#[test]
fn test_cancellation_timelock() {
    let (mut app, contract_addr) = setup_contract();

    // Deploy escrow
    let secret = generate_secret();
    let hashlock = hash_secret(&secret);

    let deploy_msg = ExecuteMsg::DeployEscrowWithFunding {
        order_hash: "test_order_hash_123".to_string(),
        hashlock: hashlock.clone(),
        maker: "maker".to_string(),
        taker: "taker".to_string(),
        token: "".to_string(),
        amount: Uint128::new(1000),
        safety_deposit: Uint128::new(100),
        timelocks: create_test_timelocks(),
        dst_chain_id: "cosmoshub-4".to_string(),
        dst_token: "dst_token".to_string(),
        dst_amount: Uint128::new(1000),
        escrow_type: EscrowType::Source,
    };

    let funds = vec![Coin::new(1100, "uatom")];
    app.execute_contract(
        Addr::unchecked("taker"),
        contract_addr.clone(),
        &deploy_msg,
        &funds,
    ).unwrap();

    // Try to cancel before timelock period
    let cancel_msg = ExecuteMsg::CancelSrc { escrow_id: 1 };

    let result = app.execute_contract(
        Addr::unchecked("taker"),
        contract_addr.clone(),
        &cancel_msg,
        &[],
    );

    // Should fail due to timelock
    assert!(result.is_err());
}

#[test]
fn test_destination_escrow_timelocks() {
    let (mut app, contract_addr) = setup_contract();

    // Deploy destination escrow
    let secret = generate_secret();
    let hashlock = hash_secret(&secret);

    let deploy_msg = ExecuteMsg::DeployEscrowWithFunding {
        order_hash: "test_order_hash_123".to_string(),
        hashlock: hashlock.clone(),
        maker: "maker".to_string(),
        taker: "taker".to_string(),
        token: "".to_string(),
        amount: Uint128::new(1000),
        safety_deposit: Uint128::new(100),
        timelocks: create_test_timelocks(),
        dst_chain_id: "cosmoshub-4".to_string(),
        dst_token: "dst_token".to_string(),
        dst_amount: Uint128::new(1000),
        escrow_type: EscrowType::Destination,
    };

    let funds = vec![Coin::new(1100, "uatom")];
    let result = app.execute_contract(
        Addr::unchecked("taker"),
        contract_addr.clone(),
        &deploy_msg,
        &funds,
    );

    assert!(result.is_ok());

    // Try to withdraw from destination escrow before timelock
    let withdraw_msg = ExecuteMsg::WithdrawDst {
        escrow_id: 1,
        secret: secret.clone(),
    };

    let result = app.execute_contract(
        Addr::unchecked("taker"),
        contract_addr.clone(),
        &withdraw_msg,
        &[],
    );

    // Should fail due to timelock
    assert!(result.is_err());
}

#[test]
fn test_invalid_timelock_progression() {
    // Create timelocks with invalid progression (public withdrawal before private)
    let invalid_timelocks = PackedTimelocks::new(
        1000, // deployed_at
        2,    // src_withdrawal: 2 hours
        1,    // src_public_withdrawal: 1 hour (should be after private)
        3,    // src_cancellation: 3 hours
        4,    // src_public_cancellation: 4 hours
        1,    // dst_withdrawal: 1 hour
        2,    // dst_public_withdrawal: 2 hours
        3,    // dst_cancellation: 3 hours
    );

    // Test validation
    assert!(invalid_timelocks.validate().is_err());
}

#[test]
fn test_timelock_stage_progression() {
    let timelocks = create_test_timelocks();
    let _deployed_at = timelocks.deployed_at() as u64;

    // Test that stages progress in the correct order
    let src_withdrawal_time = timelocks.get_stage_time(TimelockStage::SrcWithdrawal);
    let src_public_withdrawal_time = timelocks.get_stage_time(TimelockStage::SrcPublicWithdrawal);
    let src_cancellation_time = timelocks.get_stage_time(TimelockStage::SrcCancellation);
    let src_public_cancellation_time = timelocks.get_stage_time(TimelockStage::SrcPublicCancellation);

    // Verify progression order
    assert!(src_withdrawal_time < src_public_withdrawal_time);
    assert!(src_public_withdrawal_time < src_cancellation_time);
    assert!(src_cancellation_time < src_public_cancellation_time);

    // Test destination timelocks
    let dst_withdrawal_time = timelocks.get_stage_time(TimelockStage::DstWithdrawal);
    let dst_public_withdrawal_time = timelocks.get_stage_time(TimelockStage::DstPublicWithdrawal);
    let dst_cancellation_time = timelocks.get_stage_time(TimelockStage::DstCancellation);

    // Verify destination progression order
    assert!(dst_withdrawal_time < dst_public_withdrawal_time);
    assert!(dst_public_withdrawal_time < dst_cancellation_time);
} 