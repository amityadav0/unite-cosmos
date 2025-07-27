use cosmwasm_std::{Addr, Coin, Uint128};
use cw_multi_test::{App, Contract, ContractWrapper, Executor};
use escrow_contract::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use escrow_contract::state::{TimelockStage, PackedTimelocks};

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
    })
}

#[test]
fn test_instantiate() {
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

    // Query config to verify instantiation
    let config: escrow_contract::msg::ConfigResponse = app
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::Config {})
        .unwrap();

    assert_eq!(config.owner, "owner");
    assert_eq!(config.access_token, "access_token");
    assert_eq!(config.rescue_delay, 3600);
    assert_eq!(config.factory, "factory");
}

#[test]
fn test_create_escrow() {
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

    let create_msg = ExecuteMsg::CreateEscrow {
        order_hash: "order_hash_123".to_string(),
        hashlock: "hashlock_456".to_string(),
        maker: "maker_address_123".to_string(),
        taker: "taker_address_456".to_string(),
        token: "token_address_123".to_string(),
        amount: Uint128::new(1000),
        safety_deposit: Uint128::new(100),
        timelocks: escrow_contract::state::PackedTimelocks::new(
            0, // deployed_at (will be set by contract)
            1, // src_withdrawal (1 hour)
            2, // src_public_withdrawal (2 hours)
            3, // src_cancellation (3 hours)
            4, // src_public_cancellation (4 hours)
            1, // dst_withdrawal (1 hour)
            2, // dst_public_withdrawal (2 hours)
            3, // dst_cancellation (3 hours)
        ),
        dst_chain_id: "cosmoshub-4".to_string(),
        dst_token: "dst_token_address_789".to_string(),
        dst_amount: Uint128::new(1000),
    };

    let result = app.execute_contract(
        Addr::unchecked("factory"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(1100, "uatom")], // amount + safety_deposit
    );

    if let Err(e) = &result {
        println!("Error: {:?}", e);
    }
    assert!(result.is_ok());

    // Query escrow to verify creation
    let escrow: escrow_contract::msg::EscrowResponse = app
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::Escrow { escrow_id: 1 })
        .unwrap();

    assert_eq!(escrow.escrow_id, 1);
    assert_eq!(escrow.immutables.order_hash, "order_hash_123");
    assert_eq!(escrow.immutables.hashlock, "hashlock_456");
    assert_eq!(escrow.immutables.maker, "maker_address_123");
    assert_eq!(escrow.immutables.taker, "taker_address_456");
    assert_eq!(escrow.immutables.token, "token_address_123");
    assert_eq!(escrow.balance, Uint128::new(1000));
    assert_eq!(escrow.native_balance, Uint128::new(100));
    assert!(escrow.is_active);
    assert!(escrow.is_src);
}

#[test]
fn test_sophisticated_timelock_system() {
    // Test the sophisticated timelock system functionality
    
    // Create timelocks with specific values
    let deployed_at = 1000u32; // Timestamp when escrow was deployed
    let timelocks = PackedTimelocks::new(
        deployed_at,
        1,  // src_withdrawal: 1 hour after deployment
        2,  // src_public_withdrawal: 2 hours after deployment
        3,  // src_cancellation: 3 hours after deployment
        4,  // src_public_cancellation: 4 hours after deployment
        1,  // dst_withdrawal: 1 hour after deployment
        2,  // dst_public_withdrawal: 2 hours after deployment
        3,  // dst_cancellation: 3 hours after deployment
    );

    // Test deployed_at extraction
    assert_eq!(timelocks.deployed_at(), deployed_at);

    // Test individual timelock extraction
    assert_eq!(timelocks.get(TimelockStage::SrcWithdrawal), 1);
    assert_eq!(timelocks.get(TimelockStage::SrcPublicWithdrawal), 2);
    assert_eq!(timelocks.get(TimelockStage::SrcCancellation), 3);
    assert_eq!(timelocks.get(TimelockStage::SrcPublicCancellation), 4);
    assert_eq!(timelocks.get(TimelockStage::DstWithdrawal), 1);
    assert_eq!(timelocks.get(TimelockStage::DstPublicWithdrawal), 2);
    assert_eq!(timelocks.get(TimelockStage::DstCancellation), 3);

    // Test stage time calculations (convert hours to seconds)
    let src_withdrawal_time = timelocks.get_stage_time(TimelockStage::SrcWithdrawal);
    assert_eq!(src_withdrawal_time, deployed_at as u64 + (1 * 3600)); // 1 hour in seconds

    let src_public_withdrawal_time = timelocks.get_stage_time(TimelockStage::SrcPublicWithdrawal);
    assert_eq!(src_public_withdrawal_time, deployed_at as u64 + (2 * 3600)); // 2 hours in seconds

    // Test stage validation
    let current_time_before = deployed_at as u64 + 1800; // 30 minutes after deployment
    let current_time_during = deployed_at as u64 + 3600; // 1 hour after deployment
    let current_time_after = deployed_at as u64 + 7200;  // 2 hours after deployment

    // Before src_withdrawal stage
    assert!(!timelocks.is_within_stage(current_time_before, TimelockStage::SrcWithdrawal));
    assert!(!timelocks.is_within_stage(current_time_before, TimelockStage::SrcPublicWithdrawal));

    // During src_withdrawal stage
    assert!(timelocks.is_within_stage(current_time_during, TimelockStage::SrcWithdrawal));
    assert!(!timelocks.is_within_stage(current_time_during, TimelockStage::SrcPublicWithdrawal));

    // After src_public_withdrawal stage
    assert!(timelocks.is_within_stage(current_time_after, TimelockStage::SrcWithdrawal));
    assert!(timelocks.is_within_stage(current_time_after, TimelockStage::SrcPublicWithdrawal));

    // Test stage progression
    assert!(timelocks.has_stage_passed(current_time_after, TimelockStage::SrcWithdrawal));
    assert!(!timelocks.has_stage_passed(current_time_during, TimelockStage::SrcPublicWithdrawal));

    // Test current stage detection
    let stage_at_30min = timelocks.get_current_stage(current_time_before);
    assert!(stage_at_30min.is_none()); // No stage has started yet

    let stage_at_1hour = timelocks.get_current_stage(current_time_during);
    assert_eq!(stage_at_1hour, Some(TimelockStage::SrcWithdrawal));

    let stage_at_2hours = timelocks.get_current_stage(current_time_after);
    assert_eq!(stage_at_2hours, Some(TimelockStage::SrcWithdrawal)); // First stage that has started

    // Test rescue functionality
    let rescue_delay = 24 * 3600; // 24 hours
    let rescue_start = timelocks.rescue_start(rescue_delay);
    assert_eq!(rescue_start, deployed_at as u64 + rescue_delay);

    // Test rescue availability
    assert!(!timelocks.is_rescue_available(current_time_after, rescue_delay)); // Too early
    assert!(timelocks.is_rescue_available(rescue_start + 3600, rescue_delay)); // After rescue start

    // Test stage properties
    assert!(TimelockStage::SrcWithdrawal.is_source());
    assert!(TimelockStage::SrcWithdrawal.is_private());
    assert!(!TimelockStage::SrcWithdrawal.is_public());

    assert!(TimelockStage::SrcPublicWithdrawal.is_source());
    assert!(TimelockStage::SrcPublicWithdrawal.is_public());
    assert!(!TimelockStage::SrcPublicWithdrawal.is_private());

    assert!(TimelockStage::DstWithdrawal.is_destination());
    assert!(TimelockStage::DstWithdrawal.is_private());

    // Test validation
    let valid_timelocks = PackedTimelocks::new(
        1000, // deployed_at
        1,    // src_withdrawal
        2,    // src_public_withdrawal (after private)
        3,    // src_cancellation (after public)
        4,    // src_public_cancellation (after private cancellation)
        1,    // dst_withdrawal
        2,    // dst_public_withdrawal (after private)
        3,    // dst_cancellation (after public)
    );
    assert!(valid_timelocks.validate().is_ok());

    // Test debug info
    let debug_info = valid_timelocks.debug_info();
    assert!(debug_info.contains("Deployed: 1000"));
    assert!(debug_info.contains("Src: [1h, 2h, 3h, 4h]"));
    assert!(debug_info.contains("Dst: [1h, 2h, 3h]"));
} 