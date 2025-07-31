use cosmwasm_std::{Addr, Coin, Uint128};
use cw_multi_test::{App, Contract, ContractWrapper, Executor};
use escrow_contract::msg::{ExecuteMsg, InstantiateMsg};
use escrow_contract::state::{PackedTimelocks, EscrowType};
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
        order_hash: "test_order_hash_123".to_string(),
        hashlock: "test_hashlock_456".to_string(),
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

    let contract_addr = app
        .instantiate_contract(contract_id, Addr::unchecked("owner"), &msg, &[Coin::new(1100, "uatom")], "Escrow", None)
        .unwrap();

    (app, contract_addr)
}

#[test]
fn test_unauthorized_escrow_creation() {
    let (mut app, contract_addr) = setup_contract();

    // Test that unauthorized users cannot deploy escrows
    let deploy_msg = ExecuteMsg::DeployEscrowWithFunding {
        order_hash: "test_order_hash_123".to_string(),
        hashlock: "test_hashlock_456".to_string(),
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
    let result = app.execute_contract(
        Addr::unchecked("unauthorized"),
        contract_addr.clone(),
        &deploy_msg,
        &funds,
    );

    // Should fail due to insufficient balance
    assert!(result.is_err());
}

#[test]
fn test_insufficient_funds_deployment() {
    let (mut app, contract_addr) = setup_contract();

    // Test deployment with insufficient funds
    let deploy_msg = ExecuteMsg::DeployEscrowWithFunding {
        order_hash: "test_order_hash_123".to_string(),
        hashlock: "test_hashlock_456".to_string(),
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

    let insufficient_funds = vec![Coin::new(500, "uatom")]; // Less than required
    let result = app.execute_contract(
        Addr::unchecked("taker"),
        contract_addr.clone(),
        &deploy_msg,
        &insufficient_funds,
    );

    // Should fail due to insufficient balance
    assert!(result.is_err());
}

#[test]
fn test_duplicate_escrow_deployment() {
    let (mut app, contract_addr) = setup_contract();

    // Deploy first escrow
    let deploy_msg = ExecuteMsg::DeployEscrowWithFunding {
        order_hash: "test_order_hash_123".to_string(),
        hashlock: "test_hashlock_456".to_string(),
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

    // Try to deploy duplicate escrow with same parameters
    let result = app.execute_contract(
        Addr::unchecked("taker"),
        contract_addr.clone(),
        &deploy_msg,
        &funds,
    );

    // Should fail due to duplicate escrow
    assert!(result.is_err());
}

#[test]
fn test_invalid_escrow_parameters() {
    let (mut app, contract_addr) = setup_contract();

    // Test with empty order hash
    let deploy_msg = ExecuteMsg::DeployEscrowWithFunding {
        order_hash: "".to_string(),
        hashlock: "test_hashlock_456".to_string(),
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
    let result = app.execute_contract(
        Addr::unchecked("taker"),
        contract_addr.clone(),
        &deploy_msg,
        &funds,
    );

    // Should fail due to invalid parameters
    assert!(result.is_err());
}

#[test]
fn test_zero_amount_validation() {
    let (mut app, contract_addr) = setup_contract();

    // Test with zero amount
    let deploy_msg = ExecuteMsg::DeployEscrowWithFunding {
        order_hash: "test_order_hash_123".to_string(),
        hashlock: "test_hashlock_456".to_string(),
        maker: "maker".to_string(),
        taker: "taker".to_string(),
        token: "".to_string(),
        amount: Uint128::zero(),
        safety_deposit: Uint128::new(100),
        timelocks: create_test_timelocks(),
        dst_chain_id: "cosmoshub-4".to_string(),
        dst_token: "dst_token".to_string(),
        dst_amount: Uint128::new(1000),
        escrow_type: EscrowType::Source,
    };

    let funds = vec![Coin::new(1100, "uatom")];
    let result = app.execute_contract(
        Addr::unchecked("taker"),
        contract_addr.clone(),
        &deploy_msg,
        &funds,
    );

    // Should fail due to zero amount
    assert!(result.is_err());
}

#[test]
fn test_invalid_timelock_validation() {
    let (mut app, contract_addr) = setup_contract();

    // Create invalid timelocks (public withdrawal before private)
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

    let deploy_msg = ExecuteMsg::DeployEscrowWithFunding {
        order_hash: "test_order_hash_123".to_string(),
        hashlock: "test_hashlock_456".to_string(),
        maker: "maker".to_string(),
        taker: "taker".to_string(),
        token: "".to_string(),
        amount: Uint128::new(1000),
        safety_deposit: Uint128::new(100),
        timelocks: invalid_timelocks,
        dst_chain_id: "cosmoshub-4".to_string(),
        dst_token: "dst_token".to_string(),
        dst_amount: Uint128::new(1000),
        escrow_type: EscrowType::Source,
    };

    let funds = vec![Coin::new(1100, "uatom")];
    let result = app.execute_contract(
        Addr::unchecked("taker"),
        contract_addr.clone(),
        &deploy_msg,
        &funds,
    );

    // Should fail due to invalid timelocks
    assert!(result.is_err());
}

#[test]
fn test_unauthorized_withdrawal() {
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

    // Test unauthorized withdrawal
    let withdraw_msg = ExecuteMsg::WithdrawSrc {
        escrow_id: 1,
        secret: secret.clone(),
    };

    let result = app.execute_contract(
        Addr::unchecked("unauthorized"),
        contract_addr.clone(),
        &withdraw_msg,
        &[],
    );

    // Should fail due to unauthorized access
    assert!(result.is_err());
}

#[test]
fn test_unauthorized_cancellation() {
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

    // Test unauthorized cancellation
    let cancel_msg = ExecuteMsg::CancelSrc { escrow_id: 1 };

    let result = app.execute_contract(
        Addr::unchecked("unauthorized"),
        contract_addr.clone(),
        &cancel_msg,
        &[],
    );

    // Should fail due to unauthorized access
    assert!(result.is_err());
} 