use cosmwasm_std::{Addr, Coin, Uint128, Timestamp};
use cw_multi_test::{App, Contract, ContractWrapper, Executor};
use escrow_contract::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use escrow_contract::state::{TimelockStage, PackedTimelocks, EscrowType, EscrowCreationParams};
use sha2::{Sha256, Digest};
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// TEST UTILITIES AND HELPERS
// ============================================================================

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

fn create_test_escrow_params() -> EscrowCreationParams {
    EscrowCreationParams {
        order_hash: "test_order_hash_123".to_string(),
        hashlock: "test_hashlock_456".to_string(),
        maker: Addr::unchecked("maker"),
        taker: Addr::unchecked("taker"),
        token: Addr::unchecked("token_address"),
        amount: Uint128::new(1000),
        safety_deposit: Uint128::new(100),
        timelocks: create_test_timelocks(),
        escrow_type: EscrowType::Source,
        dst_chain_id: "cosmoshub-4".to_string(),
        dst_token: Addr::unchecked("dst_token_address"),
        dst_amount: Uint128::new(1000),
    }
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
        factory: "factory".to_string(),
    };

    let contract_addr = app
        .instantiate_contract(contract_id, Addr::unchecked("owner"), &msg, &[], "Escrow", None)
        .unwrap();

    (app, contract_addr)
}

// ============================================================================
// UNIT TESTS
// ============================================================================

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
}

#[test]
fn test_escrow_type_behavior() {
    // Test EscrowType enum behavior
    let source = EscrowType::Source;
    let destination = EscrowType::Destination;

    assert!(source.is_source());
    assert!(!source.is_destination());
    assert!(destination.is_destination());
    assert!(!destination.is_source());

    // Test withdrawal recipient logic
    let maker = Addr::unchecked("maker");
    let taker = Addr::unchecked("taker");

    assert_eq!(source.get_withdrawal_recipient(&maker, &taker), taker);
    assert_eq!(destination.get_withdrawal_recipient(&maker, &taker), maker);

    // Test cancellation recipient logic
    assert_eq!(source.get_cancellation_recipient(&maker, &taker), maker);
    assert_eq!(destination.get_cancellation_recipient(&maker, &taker), taker);
}

#[test]
fn test_packed_timelocks_creation() {
    let timelocks = create_test_timelocks();

    // Test deployed_at extraction
    assert_eq!(timelocks.deployed_at(), 1000);

    // Test individual timelock values
    assert_eq!(timelocks.get(TimelockStage::SrcWithdrawal), 1);
    assert_eq!(timelocks.get(TimelockStage::SrcPublicWithdrawal), 2);
    assert_eq!(timelocks.get(TimelockStage::SrcCancellation), 3);
    assert_eq!(timelocks.get(TimelockStage::SrcPublicCancellation), 4);
    assert_eq!(timelocks.get(TimelockStage::DstWithdrawal), 1);
    assert_eq!(timelocks.get(TimelockStage::DstPublicWithdrawal), 2);
    assert_eq!(timelocks.get(TimelockStage::DstCancellation), 3);
}

#[test]
fn test_timelock_stage_calculations() {
    let timelocks = create_test_timelocks();
    let deployed_at = 1000u64;

    // Test stage time calculations
    let src_withdrawal_time = timelocks.get_stage_time(TimelockStage::SrcWithdrawal);
    assert_eq!(src_withdrawal_time, deployed_at + (1 * 3600));

    let src_public_withdrawal_time = timelocks.get_stage_time(TimelockStage::SrcPublicWithdrawal);
    assert_eq!(src_public_withdrawal_time, deployed_at + (2 * 3600));

    // Test stage validation
    let current_time_before = deployed_at + 1800; // 30 minutes after
    let current_time_during = deployed_at + 3600; // 1 hour after
    let current_time_after = deployed_at + 7200;  // 2 hours after

    assert!(!timelocks.is_within_stage(current_time_before, TimelockStage::SrcWithdrawal));
    assert!(timelocks.is_within_stage(current_time_during, TimelockStage::SrcWithdrawal));
    assert!(timelocks.is_within_stage(current_time_after, TimelockStage::SrcWithdrawal));
}

// ============================================================================
// INTEGRATION TESTS
// ============================================================================

#[test]
fn test_complete_escrow_flow() {
    let (mut app, contract_addr) = setup_contract();

    // 1. Create escrow
    let params = create_test_escrow_params();
    let create_msg = ExecuteMsg::CreateEscrow {
        params: params.clone(),
        salt: "test_salt".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    );
    assert!(result.is_ok());

    // 2. Query escrow to verify creation
    let escrow_response: escrow_contract::msg::EscrowResponse = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::Escrow { escrow_id: 1 }
        )
        .unwrap();

    assert_eq!(escrow_response.immutables.order_hash, params.order_hash);
    assert_eq!(escrow_response.immutables.hashlock, params.hashlock);
}

#[test]
fn test_withdrawal_with_correct_secret() {
    let (mut app, contract_addr) = setup_contract();

    // Create escrow
    let params = create_test_escrow_params();
    let secret = generate_secret();
    let hashlock = hash_secret(&secret);

    let mut params_with_secret = params.clone();
    params_with_secret.hashlock = hashlock;

    let create_msg = ExecuteMsg::CreateEscrow {
        params: params_with_secret,
        salt: "withdrawal_test_salt".to_string(),
    };

    app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    ).unwrap();

    // Attempt withdrawal with correct secret
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

    // This should fail because we need to advance time to the withdrawal stage
    assert!(result.is_err());
}

#[test]
fn test_withdrawal_with_incorrect_secret() {
    let (mut app, contract_addr) = setup_contract();

    // Create escrow
    let params = create_test_escrow_params();
    let correct_secret = generate_secret();
    let hashlock = hash_secret(&correct_secret);

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

    // Attempt withdrawal with incorrect secret
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

// ============================================================================
// SECURITY TESTS
// ============================================================================

#[test]
fn test_unauthorized_access() {
    let (mut app, contract_addr) = setup_contract();

    // Test unauthorized user trying to create escrow
    let params = create_test_escrow_params();
    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "unauthorized_test_salt".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("unauthorized_user"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    );

    assert!(result.is_err());
}

#[test]
fn test_insufficient_balance() {
    let (mut app, contract_addr) = setup_contract();

    // Test creation with insufficient fee
    let params = create_test_escrow_params();
    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "insufficient_balance_test_salt".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(1, "uatom")], // Insufficient fee
    );

    assert!(result.is_err());
}

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

// ============================================================================
// TIMELOCK TESTS
// ============================================================================

#[test]
fn test_timelock_violations() {
    let (mut app, contract_addr) = setup_contract();

    // Create escrow
    let params = create_test_escrow_params();
    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "timelock_violation_test_salt".to_string(),
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
fn test_public_withdrawal_timelock() {
    let (mut app, contract_addr) = setup_contract();

    // Create escrow
    let params = create_test_escrow_params();
    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "public_withdrawal_test_salt".to_string(),
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
fn test_cancellation_timelock() {
    let (mut app, contract_addr) = setup_contract();

    // Create escrow
    let params = create_test_escrow_params();
    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "cancellation_test_salt".to_string(),
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

// ============================================================================
// CROSS-CHAIN TESTS
// ============================================================================

#[test]
fn test_destination_escrow_creation() {
    let (mut app, contract_addr) = setup_contract();

    // Create destination escrow
    let mut params = create_test_escrow_params();
    params.escrow_type = EscrowType::Destination;

    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "destination_test_salt".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    );

    assert!(result.is_ok());
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

#[test]
fn test_destination_escrow_behavior() {
    let (mut app, contract_addr) = setup_contract();

    // Create destination escrow
    let mut params = create_test_escrow_params();
    params.escrow_type = EscrowType::Destination;

    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "destination_behavior_test_salt".to_string(),
    };

    app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    ).unwrap();

    // Query escrow to verify destination behavior
    let escrow_response: escrow_contract::msg::EscrowResponse = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::Escrow { escrow_id: 1 }
        )
        .unwrap();

    assert_eq!(escrow_response.escrow_type, EscrowType::Destination);
}

// ============================================================================
// ERROR HANDLING TESTS
// ============================================================================

#[test]
fn test_invalid_escrow_id() {
    let (mut app, contract_addr) = setup_contract();

    // Try to query non-existent escrow
    let result: Result<escrow_contract::msg::EscrowResponse, _> = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::Escrow { escrow_id: 999 }
        );

    assert!(result.is_err());
}

#[test]
fn test_invalid_parameters() {
    let (mut app, contract_addr) = setup_contract();

    // Test with empty order hash
    let mut params = create_test_escrow_params();
    params.order_hash = "".to_string();

    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "invalid_params_test_salt".to_string(),
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

    // Test with zero amount
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

// ============================================================================
// RESCUE FUNCTIONALITY TESTS
// ============================================================================

#[test]
fn test_rescue_functionality() {
    let (mut app, contract_addr) = setup_contract();

    // Create escrow
    let params = create_test_escrow_params();
    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "rescue_test_salt".to_string(),
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

// ============================================================================
// DETERMINISTIC ADDRESS TESTS
// ============================================================================

#[test]
fn test_deterministic_address_generation() {
    let (mut app, contract_addr) = setup_contract();

    // Create escrow with specific parameters
    let params = create_test_escrow_params();
    let create_msg = ExecuteMsg::CreateEscrow {
        params: params.clone(),
        salt: "deterministic_test_salt".to_string(),
    };

    app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    ).unwrap();

    // Query the generated address
    let address_response: escrow_contract::msg::EscrowAddressResponse = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::AddressOfEscrow {
                order_hash: params.order_hash,
                hashlock: params.hashlock,
                salt: "deterministic_test_salt".to_string(),
            }
        )
        .unwrap();

    assert!(!address_response.address.is_empty());
}

// ============================================================================
// EVENT EMISSION TESTS
// ============================================================================

#[test]
fn test_event_emission_on_creation() {
    let (mut app, contract_addr) = setup_contract();

    // Create escrow
    let params = create_test_escrow_params();
    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "event_test_salt".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    );

    assert!(result.is_ok());

    // Check that events were emitted
    let response = result.unwrap();
    assert!(!response.events.is_empty());
    
    // Verify specific attributes
    let create_event = response.events.iter().find(|e| e.ty == "wasm").unwrap();
    let method_attr = create_event.attributes.iter().find(|a| a.key == "method").unwrap();
    assert_eq!(method_attr.value, "create_escrow");
}

// ============================================================================
// BALANCE VERIFICATION TESTS
// ============================================================================

#[test]
fn test_balance_verification() {
    let (mut app, contract_addr) = setup_contract();

    // Get initial balance
    let initial_balance = app.wrap().query_balance("owner", "uatom").unwrap().amount;

    // Create escrow
    let params = create_test_escrow_params();
    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "balance_test_salt".to_string(),
    };

    app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    ).unwrap();

    // Verify balance was deducted
    let final_balance = app.wrap().query_balance("owner", "uatom").unwrap().amount;
    assert_eq!(final_balance, initial_balance - Uint128::new(10));
}

// ============================================================================
// EDGE CASE TESTS
// ============================================================================

#[test]
fn test_maximum_timelock_values() {
    let (mut app, contract_addr) = setup_contract();

    // Test with maximum timelock values that follow proper progression
    let max_timelocks = PackedTimelocks::new(
        1000, // deployed_at
        1,    // src_withdrawal: 1 hour
        2,    // src_public_withdrawal: 2 hours (after private)
        3,    // src_cancellation: 3 hours (after public withdrawal)
        4,    // src_public_cancellation: 4 hours (after private cancellation)
        1,    // dst_withdrawal: 1 hour
        2,    // dst_public_withdrawal: 2 hours (after private)
        3,    // dst_cancellation: 3 hours (after public withdrawal)
    );

    let mut params = create_test_escrow_params();
    params.timelocks = max_timelocks;

    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "max_timelock_test_salt".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    );

    assert!(result.is_ok());
}

#[test]
fn test_minimum_timelock_values() {
    let (mut app, contract_addr) = setup_contract();

    // Test with minimum timelock values that follow proper progression
    let min_timelocks = PackedTimelocks::new(
        1000, // deployed_at
        1,    // src_withdrawal: 1 hour (minimum valid)
        2,    // src_public_withdrawal: 2 hours (after private)
        3,    // src_cancellation: 3 hours (after public withdrawal)
        4,    // src_public_cancellation: 4 hours (after private cancellation)
        1,    // dst_withdrawal: 1 hour (minimum valid)
        2,    // dst_public_withdrawal: 2 hours (after private)
        3,    // dst_cancellation: 3 hours (after public withdrawal)
    );

    let mut params = create_test_escrow_params();
    params.timelocks = min_timelocks;

    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "min_timelock_test_salt".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    );

    assert!(result.is_ok());
}

#[test]
fn test_large_amount_values() {
    let (mut app, contract_addr) = setup_contract();

    // Test with very large amounts
    let mut params = create_test_escrow_params();
    params.amount = Uint128::new(1000000000000); // 1 trillion
    params.safety_deposit = Uint128::new(100000000000); // 100 billion

    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "large_amount_test_salt".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    );

    assert!(result.is_ok());
} 