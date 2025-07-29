use cosmwasm_std::{Addr, Coin, Uint128};
use cw_multi_test::{App, Contract, ContractWrapper, Executor};
use escrow_contract::msg::{ExecuteMsg, InstantiateMsg};
use escrow_contract::state::{PackedTimelocks, EscrowType, EscrowCreationParams};
use sha2::{Sha256, Digest};

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
        router.bank.init_balance(storage, &Addr::unchecked("factory"), vec![Coin::new(5000, "uatom")]).unwrap();
        router.bank.init_balance(storage, &Addr::unchecked("access_token"), vec![Coin::new(1000, "uatom")]).unwrap();
        router.bank.init_balance(storage, &Addr::unchecked("maker"), vec![Coin::new(2000, "uatom")]).unwrap();
        router.bank.init_balance(storage, &Addr::unchecked("taker"), vec![Coin::new(2000, "uatom")]).unwrap();
    })
}

fn setup_contract() -> (App, Addr) {
    let mut app = mock_app();
    let contract_id = app.store_code(escrow_contract());

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        access_token: "access_token".to_string(),
        rescue_delay: 3600,
        factory: "factory".to_string(),
    };

    let contract_addr = app
        .instantiate_contract(contract_id, Addr::unchecked("owner"), &msg, &[], "Escrow", None)
        .unwrap();

    (app, contract_addr)
}

fn create_test_escrow_params() -> EscrowCreationParams {
    EscrowCreationParams {
        order_hash: "test_order_hash_123".to_string(),
        hashlock: "test_hashlock_456".to_string(),
        maker: Addr::unchecked("maker"),
        taker: Addr::unchecked("taker"),
        token: Addr::unchecked("token_address"),
        amount: Uint128::new(1000),
        safety_deposit: Uint128::new(100),
        timelocks: PackedTimelocks::new(
            1000, // deployed_at
            1,    // src_withdrawal: 1 hour
            2,    // src_public_withdrawal: 2 hours
            3,    // src_cancellation: 3 hours
            4,    // src_public_cancellation: 4 hours
            1,    // dst_withdrawal: 1 hour
            2,    // dst_public_withdrawal: 2 hours
            3,    // dst_cancellation: 3 hours
        ),
        escrow_type: EscrowType::Source,
        dst_chain_id: "cosmoshub-4".to_string(),
        dst_token: Addr::unchecked("dst_token_address"),
        dst_amount: Uint128::new(1000),
    }
}

// ============================================================================
// ACCESS CONTROL TESTS
// ============================================================================

#[test]
fn test_owner_only_operations() {
    let (mut app, contract_addr) = setup_contract();

    // Test that only owner can cancel creation requests
    let cancel_msg = ExecuteMsg::CancelCreationRequest {
        order_hash: "test_order".to_string(),
        hashlock: "test_hashlock".to_string(),
    };

    // Non-owner should fail
    let result = app.execute_contract(
        Addr::unchecked("unauthorized_user"),
        contract_addr.clone(),
        &cancel_msg,
        &[],
    );
    assert!(result.is_err());

    // Owner should succeed (even if request doesn't exist, it should fail for different reason)
    let result = app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &cancel_msg,
        &[],
    );
    // This will fail because the request doesn't exist, but not due to access control
    assert!(result.is_err());
}

#[test]
fn test_unauthorized_escrow_creation() {
    let (mut app, contract_addr) = setup_contract();

    let params = create_test_escrow_params();
    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "unauthorized_test_salt".to_string(),
    };

    // Non-owner trying to create escrow should fail
    let result = app.execute_contract(
        Addr::unchecked("unauthorized_user"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    );

    assert!(result.is_err());
}

#[test]
fn test_unauthorized_withdrawal() {
    let (mut app, contract_addr) = setup_contract();

    // Create escrow first
    let params = create_test_escrow_params();
    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "unauthorized_withdrawal_test_salt".to_string(),
    };

    app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    ).unwrap();

    // Try withdrawal with wrong user
    let withdraw_msg = ExecuteMsg::WithdrawSrc {
        escrow_id: 1,
        secret: "wrong_secret".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("wrong_user"),
        contract_addr.clone(),
        &withdraw_msg,
        &[],
    );

    assert!(result.is_err());
}

// ============================================================================
// VALIDATION TESTS
// ============================================================================

#[test]
fn test_empty_order_hash_validation() {
    let (mut app, contract_addr) = setup_contract();

    let mut params = create_test_escrow_params();
    params.order_hash = "".to_string();

    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "empty_order_hash_test_salt".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    );

    assert!(result.is_err());
}

#[test]
fn test_empty_hashlock_validation() {
    let (mut app, contract_addr) = setup_contract();

    let mut params = create_test_escrow_params();
    params.hashlock = "".to_string();

    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "empty_hashlock_test_salt".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    );

    assert!(result.is_err());
}

#[test]
fn test_zero_amount_validation() {
    let (mut app, contract_addr) = setup_contract();

    let mut params = create_test_escrow_params();
    params.amount = Uint128::zero();

    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "zero_amount_test_salt".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    );

    assert!(result.is_err());
}

#[test]
fn test_zero_safety_deposit_validation() {
    let (mut app, contract_addr) = setup_contract();

    let mut params = create_test_escrow_params();
    params.safety_deposit = Uint128::zero();

    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "zero_safety_deposit_test_salt".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    );

    assert!(result.is_err());
}

#[test]
fn test_invalid_timelock_progression() {
    let (mut app, contract_addr) = setup_contract();

    // Create timelocks with invalid progression (public withdrawal before private)
    let invalid_timelocks = PackedTimelocks::new(
        1000, // deployed_at
        2,    // src_withdrawal: 2 hours
        1,    // src_public_withdrawal: 1 hour (INVALID: before private)
        3,    // src_cancellation: 3 hours
        4,    // src_public_cancellation: 4 hours
        1,    // dst_withdrawal: 1 hour
        2,    // dst_public_withdrawal: 2 hours
        3,    // dst_cancellation: 3 hours
    );

    let mut params = create_test_escrow_params();
    params.timelocks = invalid_timelocks;

    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "invalid_timelock_test_salt".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    );

    assert!(result.is_err());
}

// ============================================================================
// SECURITY EDGE CASES
// ============================================================================

#[test]
fn test_duplicate_escrow_creation() {
    let (mut app, contract_addr) = setup_contract();

    // Create first escrow
    let params = create_test_escrow_params();
    let create_msg = ExecuteMsg::CreateEscrow {
        params: params.clone(),
        salt: "duplicate_test_salt".to_string(),
    };

    app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    ).unwrap();

    // Try to create duplicate escrow with same parameters
    let duplicate_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "duplicate_test_salt".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &duplicate_msg,
        &[Coin::new(10, "uatom")],
    );

    assert!(result.is_err());
}

#[test]
fn test_insufficient_creation_fee() {
    let (mut app, contract_addr) = setup_contract();

    let params = create_test_escrow_params();
    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "insufficient_fee_test_salt".to_string(),
    };

    // Try with insufficient fee
    let result = app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(1, "uatom")], // Insufficient fee
    );

    assert!(result.is_err());
}

#[test]
fn test_invalid_escrow_id_access() {
    let (mut app, contract_addr) = setup_contract();

    // Try to access non-existent escrow
    let withdraw_msg = ExecuteMsg::WithdrawSrc {
        escrow_id: 999, // Non-existent escrow
        secret: "test_secret".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("taker"),
        contract_addr.clone(),
        &withdraw_msg,
        &[],
    );

    assert!(result.is_err());
}

#[test]
fn test_cross_chain_parameter_validation() {
    let (mut app, contract_addr) = setup_contract();

    // Test with empty destination chain ID for source escrow
    let mut params = create_test_escrow_params();
    params.dst_chain_id = "".to_string();

    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "cross_chain_validation_test_salt".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    );

    assert!(result.is_err());
}

// ============================================================================
// SECRET VALIDATION TESTS
// ============================================================================

#[test]
fn test_incorrect_secret_withdrawal() {
    let (mut app, contract_addr) = setup_contract();

    // Create escrow with specific hashlock
    let params = create_test_escrow_params();
    let correct_secret = "correct_secret_123";
    let hashlock = {
        let mut hasher = Sha256::new();
        hasher.update(correct_secret.as_bytes());
        format!("{:x}", hasher.finalize())
    };

    let mut params_with_secret = params.clone();
    params_with_secret.hashlock = hashlock;

    let create_msg = ExecuteMsg::CreateEscrow {
        params: params_with_secret,
        salt: "incorrect_secret_test_salt".to_string(),
    };

    app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    ).unwrap();

    // Try withdrawal with incorrect secret
    let withdraw_msg = ExecuteMsg::WithdrawSrc {
        escrow_id: 1,
        secret: "incorrect_secret".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("taker"),
        contract_addr.clone(),
        &withdraw_msg,
        &[],
    );

    assert!(result.is_err());
}

#[test]
fn test_empty_secret_withdrawal() {
    let (mut app, contract_addr) = setup_contract();

    // Create escrow
    let params = create_test_escrow_params();
    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "empty_secret_test_salt".to_string(),
    };

    app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    ).unwrap();

    // Try withdrawal with empty secret
    let withdraw_msg = ExecuteMsg::WithdrawSrc {
        escrow_id: 1,
        secret: "".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("taker"),
        contract_addr.clone(),
        &withdraw_msg,
        &[],
    );

    assert!(result.is_err());
} 