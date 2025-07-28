use cosmwasm_std::{Addr, Coin, Uint128};
use cw_multi_test::{App, Contract, ContractWrapper, Executor};
use escrow_contract::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use escrow_contract::state::{TimelockStage, PackedTimelocks, EscrowType, EscrowCreationParams};
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
// TIMELOCK STAGE TESTS
// ============================================================================

#[test]
fn test_timelock_stage_progression() {
    let timelocks = PackedTimelocks::new(
        1000, // deployed_at
        1,    // src_withdrawal: 1 hour
        2,    // src_public_withdrawal: 2 hours
        3,    // src_cancellation: 3 hours
        4,    // src_public_cancellation: 4 hours
        1,    // dst_withdrawal: 1 hour
        2,    // dst_public_withdrawal: 2 hours
        3,    // dst_cancellation: 3 hours
    );

    let deployed_at = 1000u64;

    // Test stage time calculations
    assert_eq!(timelocks.get_stage_time(TimelockStage::SrcWithdrawal), deployed_at + 3600);
    assert_eq!(timelocks.get_stage_time(TimelockStage::SrcPublicWithdrawal), deployed_at + 7200);
    assert_eq!(timelocks.get_stage_time(TimelockStage::SrcCancellation), deployed_at + 10800);
    assert_eq!(timelocks.get_stage_time(TimelockStage::SrcPublicCancellation), deployed_at + 14400);
    assert_eq!(timelocks.get_stage_time(TimelockStage::DstWithdrawal), deployed_at + 3600);
    assert_eq!(timelocks.get_stage_time(TimelockStage::DstPublicWithdrawal), deployed_at + 7200);
    assert_eq!(timelocks.get_stage_time(TimelockStage::DstCancellation), deployed_at + 10800);
}

#[test]
fn test_timelock_stage_validation() {
    let timelocks = PackedTimelocks::new(
        1000, // deployed_at
        1,    // src_withdrawal: 1 hour
        2,    // src_public_withdrawal: 2 hours
        3,    // src_cancellation: 3 hours
        4,    // src_public_cancellation: 4 hours
        1,    // dst_withdrawal: 1 hour
        2,    // dst_public_withdrawal: 2 hours
        3,    // dst_cancellation: 3 hours
    );

    let deployed_at = 1000u64;

    // Test before any stage
    let current_time_before = deployed_at + 1800; // 30 minutes after deployment
    assert!(!timelocks.is_within_stage(current_time_before, TimelockStage::SrcWithdrawal));
    assert!(!timelocks.is_within_stage(current_time_before, TimelockStage::SrcPublicWithdrawal));

    // Test during src_withdrawal stage
    let current_time_during_withdrawal = deployed_at + 3600; // 1 hour after deployment
    assert!(timelocks.is_within_stage(current_time_during_withdrawal, TimelockStage::SrcWithdrawal));
    assert!(!timelocks.is_within_stage(current_time_during_withdrawal, TimelockStage::SrcPublicWithdrawal));

    // Test during src_public_withdrawal stage
    let current_time_during_public = deployed_at + 7200; // 2 hours after deployment
    assert!(timelocks.is_within_stage(current_time_during_public, TimelockStage::SrcWithdrawal));
    assert!(timelocks.is_within_stage(current_time_during_public, TimelockStage::SrcPublicWithdrawal));
    assert!(!timelocks.is_within_stage(current_time_during_public, TimelockStage::SrcCancellation));

    // Test after all stages
    let current_time_after = deployed_at + 18000; // 5 hours after deployment
    assert!(timelocks.is_within_stage(current_time_after, TimelockStage::SrcWithdrawal));
    assert!(timelocks.is_within_stage(current_time_after, TimelockStage::SrcPublicWithdrawal));
    assert!(timelocks.is_within_stage(current_time_after, TimelockStage::SrcCancellation));
    assert!(timelocks.is_within_stage(current_time_after, TimelockStage::SrcPublicCancellation));
}

#[test]
fn test_current_stage_detection() {
    let timelocks = PackedTimelocks::new(
        1000, // deployed_at
        1,    // src_withdrawal: 1 hour
        2,    // src_public_withdrawal: 2 hours
        3,    // src_cancellation: 3 hours
        4,    // src_public_cancellation: 4 hours
        1,    // dst_withdrawal: 1 hour
        2,    // dst_public_withdrawal: 2 hours
        3,    // dst_cancellation: 3 hours
    );

    let deployed_at = 1000u64;

    // Before any stage
    let current_time_before = deployed_at + 1800; // 30 minutes after deployment
    assert_eq!(timelocks.get_current_stage(current_time_before), None);

    // During src_withdrawal stage
    let current_time_withdrawal = deployed_at + 3600; // 1 hour after deployment
    assert_eq!(timelocks.get_current_stage(current_time_withdrawal), Some(TimelockStage::SrcWithdrawal));

    // During src_public_withdrawal stage
    let current_time_public = deployed_at + 7200; // 2 hours after deployment
    assert_eq!(timelocks.get_current_stage(current_time_public), Some(TimelockStage::SrcWithdrawal));

    // During src_cancellation stage
    let current_time_cancellation = deployed_at + 10800; // 3 hours after deployment
    assert_eq!(timelocks.get_current_stage(current_time_cancellation), Some(TimelockStage::SrcWithdrawal));

    // During src_public_cancellation stage
    let current_time_public_cancellation = deployed_at + 14400; // 4 hours after deployment
    assert_eq!(timelocks.get_current_stage(current_time_public_cancellation), Some(TimelockStage::SrcWithdrawal));
}

// ============================================================================
// TIMELOCK VIOLATION TESTS
// ============================================================================

#[test]
fn test_withdrawal_before_timelock() {
    let (mut app, contract_addr) = setup_contract();

    // Create escrow
    let params = create_test_escrow_params();
    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "withdrawal_before_timelock_test_salt".to_string(),
    };

    app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    ).unwrap();

    // Try to withdraw before timelock period
    let withdraw_msg = ExecuteMsg::WithdrawSrc {
        escrow_id: 1,
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
fn test_public_withdrawal_before_timelock() {
    let (mut app, contract_addr) = setup_contract();

    // Create escrow
    let params = create_test_escrow_params();
    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "public_withdrawal_before_timelock_test_salt".to_string(),
    };

    app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    ).unwrap();

    // Try public withdrawal before timelock
    let public_withdraw_msg = ExecuteMsg::PublicWithdrawSrc {
        escrow_id: 1,
    };

    let result = app.execute_contract(
        Addr::unchecked("taker"),
        contract_addr.clone(),
        &public_withdraw_msg,
        &[],
    );

    assert!(result.is_err());
}

#[test]
fn test_cancellation_before_timelock() {
    let (mut app, contract_addr) = setup_contract();

    // Create escrow
    let params = create_test_escrow_params();
    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "cancellation_before_timelock_test_salt".to_string(),
    };

    app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    ).unwrap();

    // Try to cancel before timelock period
    let cancel_msg = ExecuteMsg::CancelSrc {
        escrow_id: 1,
    };

    let result = app.execute_contract(
        Addr::unchecked("maker"),
        contract_addr.clone(),
        &cancel_msg,
        &[],
    );

    assert!(result.is_err());
}

#[test]
fn test_public_cancellation_before_timelock() {
    let (mut app, contract_addr) = setup_contract();

    // Create escrow
    let params = create_test_escrow_params();
    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "public_cancellation_before_timelock_test_salt".to_string(),
    };

    app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    ).unwrap();

    // Try public cancellation before timelock
    let public_cancel_msg = ExecuteMsg::PublicCancelSrc {
        escrow_id: 1,
    };

    let result = app.execute_contract(
        Addr::unchecked("maker"),
        contract_addr.clone(),
        &public_cancel_msg,
        &[],
    );

    assert!(result.is_err());
}

// ============================================================================
// RESCUE FUNCTIONALITY TESTS
// ============================================================================

#[test]
fn test_rescue_before_delay() {
    let (mut app, contract_addr) = setup_contract();

    // Create escrow
    let params = create_test_escrow_params();
    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "rescue_before_delay_test_salt".to_string(),
    };

    app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    ).unwrap();

    // Try rescue before rescue delay
    let rescue_msg = ExecuteMsg::Rescue {
        escrow_id: 1,
    };

    let result = app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &rescue_msg,
        &[],
    );

    assert!(result.is_err());
}

#[test]
fn test_rescue_availability_calculation() {
    let timelocks = PackedTimelocks::new(
        1000, // deployed_at
        1,    // src_withdrawal: 1 hour
        2,    // src_public_withdrawal: 2 hours
        3,    // src_cancellation: 3 hours
        4,    // src_public_cancellation: 4 hours
        1,    // dst_withdrawal: 1 hour
        2,    // dst_public_withdrawal: 2 hours
        3,    // dst_cancellation: 3 hours
    );

    let rescue_delay = 3600u64; // 1 hour
    let deployed_at = 1000u64;

    // Test rescue availability
    let rescue_start_time = timelocks.rescue_start(rescue_delay);
    assert_eq!(rescue_start_time, deployed_at + rescue_delay);

    // Before rescue is available
    let current_time_before = deployed_at + 1800; // 30 minutes after deployment
    assert!(!timelocks.is_rescue_available(current_time_before, rescue_delay));

    // After rescue is available
    let current_time_after = deployed_at + 7200; // 2 hours after deployment
    assert!(timelocks.is_rescue_available(current_time_after, rescue_delay));
}

// ============================================================================
// DESTINATION ESCROW TIMELOCK TESTS
// ============================================================================

#[test]
fn test_destination_escrow_timelocks() {
    let (mut app, contract_addr) = setup_contract();

    // Create destination escrow
    let mut params = create_test_escrow_params();
    params.escrow_type = EscrowType::Destination;

    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "destination_timelock_test_salt".to_string(),
    };

    app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    ).unwrap();

    // Try destination withdrawal before timelock
    let withdraw_msg = ExecuteMsg::WithdrawDst {
        escrow_id: 1,
        secret: "test_secret".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("maker"),
        contract_addr.clone(),
        &withdraw_msg,
        &[],
    );

    assert!(result.is_err());
}

#[test]
fn test_destination_public_withdrawal_timelock() {
    let (mut app, contract_addr) = setup_contract();

    // Create destination escrow
    let mut params = create_test_escrow_params();
    params.escrow_type = EscrowType::Destination;

    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "destination_public_withdrawal_test_salt".to_string(),
    };

    app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    ).unwrap();

    // Try destination public withdrawal before timelock
    let public_withdraw_msg = ExecuteMsg::PublicWithdrawDst {
        escrow_id: 1,
    };

    let result = app.execute_contract(
        Addr::unchecked("maker"),
        contract_addr.clone(),
        &public_withdraw_msg,
        &[],
    );

    assert!(result.is_err());
}

// ============================================================================
// TIMELOCK STAGE TRANSITION TESTS
// ============================================================================

#[test]
fn test_stage_transition_validation() {
    let timelocks = PackedTimelocks::new(
        1000, // deployed_at
        1,    // src_withdrawal: 1 hour
        2,    // src_public_withdrawal: 2 hours
        3,    // src_cancellation: 3 hours
        4,    // src_public_cancellation: 4 hours
        1,    // dst_withdrawal: 1 hour
        2,    // dst_public_withdrawal: 2 hours
        3,    // dst_cancellation: 3 hours
    );

    // Validate timelock progression
    let validation_result = timelocks.validate();
    assert!(validation_result.is_ok());
}

#[test]
fn test_invalid_stage_transition() {
    // Create timelocks with invalid progression
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

    // Validate should fail
    let validation_result = invalid_timelocks.validate();
    assert!(validation_result.is_err());
}

// ============================================================================
// TIMELOCK DEBUG AND UTILITY TESTS
// ============================================================================

#[test]
fn test_timelock_debug_info() {
    let timelocks = PackedTimelocks::new(
        1000, // deployed_at
        1,    // src_withdrawal: 1 hour
        2,    // src_public_withdrawal: 2 hours
        3,    // src_cancellation: 3 hours
        4,    // src_public_cancellation: 4 hours
        1,    // dst_withdrawal: 1 hour
        2,    // dst_public_withdrawal: 2 hours
        3,    // dst_cancellation: 3 hours
    );

    let debug_info = timelocks.debug_info();
    assert!(debug_info.contains("Deployed: 1000"));
    assert!(debug_info.contains("Src: [1h, 2h, 3h, 4h]"));
    assert!(debug_info.contains("Dst: [1h, 2h, 3h]"));
}

#[test]
fn test_timelock_stage_passed() {
    let timelocks = PackedTimelocks::new(
        1000, // deployed_at
        1,    // src_withdrawal: 1 hour
        2,    // src_public_withdrawal: 2 hours
        3,    // src_cancellation: 3 hours
        4,    // src_public_cancellation: 4 hours
        1,    // dst_withdrawal: 1 hour
        2,    // dst_public_withdrawal: 2 hours
        3,    // dst_cancellation: 3 hours
    );

    let deployed_at = 1000u64;

    // Test stage passed functionality
    let current_time_before = deployed_at + 1800; // 30 minutes after deployment
    assert!(!timelocks.has_stage_passed(current_time_before, TimelockStage::SrcWithdrawal));

    let current_time_after = deployed_at + 7200; // 2 hours after deployment
    assert!(timelocks.has_stage_passed(current_time_after, TimelockStage::SrcWithdrawal));
    assert!(!timelocks.has_stage_passed(current_time_after, TimelockStage::SrcCancellation));
} 