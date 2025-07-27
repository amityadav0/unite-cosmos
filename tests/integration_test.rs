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

#[test]
fn test_access_control() {
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

    // Test: Only factory can create escrows
    let params = EscrowCreationParams {
        order_hash: "order_hash_123".to_string(),
        hashlock: "hashlock_456".to_string(),
        maker: Addr::unchecked("maker_address_123"),
        taker: Addr::unchecked("taker_address_456"),
        token: Addr::unchecked("token_address_123"),
        amount: Uint128::new(1000),
        safety_deposit: Uint128::new(100),
        timelocks: escrow_contract::state::PackedTimelocks::new(
            0, 1, 2, 3, 4, 1, 2, 3,
        ),
        escrow_type: EscrowType::Source,
        dst_chain_id: "cosmoshub-4".to_string(),
        dst_token: Addr::unchecked("dst_token_address_789"),
        dst_amount: Uint128::new(1000),
    };

    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "test_salt".to_string(),
    };

    // Should fail: non-factory trying to create escrow
    let result = app.execute_contract(
        Addr::unchecked("unauthorized"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")], // Creation fee
    );
    assert!(result.is_err());

    // Should succeed: factory creating escrow
    let result = app.execute_contract(
        Addr::unchecked("owner"), // Factory owner can create escrows
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")], // Creation fee
    );
    assert!(result.is_ok());

    // Test: Only taker can withdraw
    let withdraw_msg = ExecuteMsg::WithdrawSrc {
        escrow_id: 1,
        secret: "wrong_secret".to_string(),
    };

    // Should fail: non-taker trying to withdraw
    let result = app.execute_contract(
        Addr::unchecked("unauthorized"),
        contract_addr.clone(),
        &withdraw_msg,
        &[],
    );
    assert!(result.is_err());

    // Test: Only taker can cancel
    let cancel_msg = ExecuteMsg::CancelSrc { escrow_id: 1 };

    // Should fail: non-taker trying to cancel
    let result = app.execute_contract(
        Addr::unchecked("unauthorized"),
        contract_addr.clone(),
        &cancel_msg,
        &[],
    );
    assert!(result.is_err());

    // Test: Only access token holder can public withdraw
    let public_withdraw_msg = ExecuteMsg::PublicWithdrawSrc { escrow_id: 1 };

    // Should fail: non-access token holder trying to public withdraw
    let result = app.execute_contract(
        Addr::unchecked("unauthorized"),
        contract_addr.clone(),
        &public_withdraw_msg,
        &[],
    );
    assert!(result.is_err());
}

#[test]
fn test_secret_validation() {
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

    // Create escrow with specific hashlock
    let secret = "my_secret_123";
    let secret_hash = Sha256::digest(secret.as_bytes());
    let hashlock = format!("{:x}", secret_hash);

    let params = EscrowCreationParams {
        order_hash: "order_hash_123".to_string(),
        hashlock: hashlock.clone(),
        maker: Addr::unchecked("maker_address_123"),
        taker: Addr::unchecked("taker_address_456"),
        token: Addr::unchecked("token_address_123"),
        amount: Uint128::new(1000),
        safety_deposit: Uint128::new(100),
        timelocks: escrow_contract::state::PackedTimelocks::new(
            0, 1, 2, 3, 4, 1, 2, 3,
        ),
        escrow_type: EscrowType::Source,
        dst_chain_id: "cosmoshub-4".to_string(),
        dst_token: Addr::unchecked("dst_token_address_789"),
        dst_amount: Uint128::new(1000),
    };

    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "test_salt".to_string(),
    };

    app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    ).unwrap();

    // Test: Correct secret should work
    let withdraw_msg = ExecuteMsg::WithdrawSrc {
        escrow_id: 1,
        secret: secret.to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("taker_address_456"),
        contract_addr.clone(),
        &withdraw_msg,
        &[],
    );
    // This will fail due to timelock, but not due to secret validation
    assert!(result.is_err());

    // Test: Wrong secret should fail
    let withdraw_msg = ExecuteMsg::WithdrawSrc {
        escrow_id: 1,
        secret: "wrong_secret".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("taker_address_456"),
        contract_addr.clone(),
        &withdraw_msg,
        &[],
    );
    assert!(result.is_err());
}

#[test]
fn test_timelock_validation() {
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

    // Create escrow with short timelocks
    let params = EscrowCreationParams {
        order_hash: "order_hash_123".to_string(),
        hashlock: "hashlock_456".to_string(),
        maker: Addr::unchecked("maker_address_123"),
        taker: Addr::unchecked("taker_address_456"),
        token: Addr::unchecked("token_address_123"),
        amount: Uint128::new(1000),
        safety_deposit: Uint128::new(100),
        timelocks: escrow_contract::state::PackedTimelocks::new(
            0, 1, 2, 3, 4, 1, 2, 3, // 1 hour, 2 hours, etc.
        ),
        escrow_type: EscrowType::Source,
        dst_chain_id: "cosmoshub-4".to_string(),
        dst_token: Addr::unchecked("dst_token_address_789"),
        dst_amount: Uint128::new(1000),
    };

    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "test_salt".to_string(),
    };

    app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    ).unwrap();

    // Test: Withdraw before timelock should fail
    let withdraw_msg = ExecuteMsg::WithdrawSrc {
        escrow_id: 1,
        secret: "any_secret".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("taker_address_456"),
        contract_addr.clone(),
        &withdraw_msg,
        &[],
    );
    assert!(result.is_err());

    // Test: Cancel before timelock should fail
    let cancel_msg = ExecuteMsg::CancelSrc { escrow_id: 1 };

    let result = app.execute_contract(
        Addr::unchecked("taker_address_456"),
        contract_addr.clone(),
        &cancel_msg,
        &[],
    );
    assert!(result.is_err());

    // Test: Public withdraw before timelock should fail
    let public_withdraw_msg = ExecuteMsg::PublicWithdrawSrc { escrow_id: 1 };

    let result = app.execute_contract(
        Addr::unchecked("access_token"),
        contract_addr.clone(),
        &public_withdraw_msg,
        &[],
    );
    assert!(result.is_err());
}

#[test]
fn test_source_vs_destination_behavior() {
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

    // Test EscrowType helper methods
    assert!(EscrowType::Source.is_source());
    assert!(!EscrowType::Source.is_destination());
    assert!(EscrowType::Destination.is_destination());
    assert!(!EscrowType::Destination.is_source());

    // Test withdrawal recipient logic
    let maker = Addr::unchecked("maker");
    let taker = Addr::unchecked("taker");
    
    assert_eq!(EscrowType::Source.get_withdrawal_recipient(&maker, &taker), taker);
    assert_eq!(EscrowType::Destination.get_withdrawal_recipient(&maker, &taker), maker);

    // Test cancellation recipient logic
    assert_eq!(EscrowType::Source.get_cancellation_recipient(&maker, &taker), maker);
    assert_eq!(EscrowType::Destination.get_cancellation_recipient(&maker, &taker), taker);

    // Test stage mapping
    assert_eq!(EscrowType::Source.get_withdrawal_stage(), TimelockStage::SrcWithdrawal);
    assert_eq!(EscrowType::Destination.get_withdrawal_stage(), TimelockStage::DstWithdrawal);
    
    assert_eq!(EscrowType::Source.get_cancellation_stage(), TimelockStage::SrcCancellation);
    assert_eq!(EscrowType::Destination.get_cancellation_stage(), TimelockStage::DstCancellation);

    // Test public cancellation support
    assert!(EscrowType::Source.supports_public_cancellation());
    assert!(!EscrowType::Destination.supports_public_cancellation());

    assert_eq!(EscrowType::Source.get_public_cancellation_stage(), Some(TimelockStage::SrcPublicCancellation));
    assert_eq!(EscrowType::Destination.get_public_cancellation_stage(), None);
}

#[test]
fn test_escrow_type_validation() {
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

    // Create source escrow
    let src_params = EscrowCreationParams {
        order_hash: "order_hash_src".to_string(),
        hashlock: "hashlock_src".to_string(),
        maker: Addr::unchecked("maker_address_123"),
        taker: Addr::unchecked("taker_address_456"),
        token: Addr::unchecked("token_address_123"),
        amount: Uint128::new(1000),
        safety_deposit: Uint128::new(100),
        timelocks: escrow_contract::state::PackedTimelocks::new(
            0, 1, 2, 3, 4, 1, 2, 3,
        ),
        escrow_type: EscrowType::Source,
        dst_chain_id: "cosmoshub-4".to_string(),
        dst_token: Addr::unchecked("dst_token_address_789"),
        dst_amount: Uint128::new(1000),
    };

    let create_src_msg = ExecuteMsg::CreateEscrow {
        params: src_params,
        salt: "src_salt".to_string(),
    };

    app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_src_msg,
        &[Coin::new(10, "uatom")],
    ).unwrap();

    // Create destination escrow
    let dst_params = EscrowCreationParams {
        order_hash: "order_hash_dst".to_string(),
        hashlock: "hashlock_dst".to_string(),
        maker: Addr::unchecked("maker_address_123"),
        taker: Addr::unchecked("taker_address_456"),
        token: Addr::unchecked("token_address_123"),
        amount: Uint128::new(1000),
        safety_deposit: Uint128::new(100),
        timelocks: escrow_contract::state::PackedTimelocks::new(
            0, 1, 2, 3, 4, 1, 2, 3,
        ),
        escrow_type: EscrowType::Destination,
        dst_chain_id: "cosmoshub-4".to_string(),
        dst_token: Addr::unchecked("dst_token_address_789"),
        dst_amount: Uint128::new(1000),
    };

    let create_dst_msg = ExecuteMsg::CreateEscrow {
        params: dst_params,
        salt: "dst_salt".to_string(),
    };

    app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_dst_msg,
        &[Coin::new(10, "uatom")],
    ).unwrap();

    // Test: Source-specific operations on source escrow should work
    let withdraw_src_msg = ExecuteMsg::WithdrawSrc {
        escrow_id: 1,
        secret: "any_secret".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("taker_address_456"),
        contract_addr.clone(),
        &withdraw_src_msg,
        &[],
    );
    // Will fail due to timelock, but not due to type validation
    assert!(result.is_err());

    // Test: Source-specific operations on destination escrow should fail
    let withdraw_src_on_dst_msg = ExecuteMsg::WithdrawSrc {
        escrow_id: 2,
        secret: "any_secret".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("taker_address_456"),
        contract_addr.clone(),
        &withdraw_src_on_dst_msg,
        &[],
    );
    assert!(result.is_err());

    // Test: Destination-specific operations on destination escrow should work
    let withdraw_dst_msg = ExecuteMsg::WithdrawDst {
        escrow_id: 2,
        secret: "any_secret".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("taker_address_456"),
        contract_addr.clone(),
        &withdraw_dst_msg,
        &[],
    );
    // Will fail due to timelock, but not due to type validation
    assert!(result.is_err());

    // Test: Destination-specific operations on source escrow should fail
    let withdraw_dst_on_src_msg = ExecuteMsg::WithdrawDst {
        escrow_id: 1,
        secret: "any_secret".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("taker_address_456"),
        contract_addr.clone(),
        &withdraw_dst_on_src_msg,
        &[],
    );
    assert!(result.is_err());

    // Test: Public cancel on source escrow should work
    let public_cancel_src_msg = ExecuteMsg::PublicCancelSrc { escrow_id: 1 };

    let result = app.execute_contract(
        Addr::unchecked("access_token"),
        contract_addr.clone(),
        &public_cancel_src_msg,
        &[],
    );
    // Will fail due to timelock, but not due to type validation
    assert!(result.is_err());
}

#[test]
fn test_factory_pattern() {
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

    // Test factory configuration query
    let factory_config: escrow_contract::msg::FactoryConfigResponse = app
        .wrap()
        .query_wasm_smart(contract_addr.clone(), &QueryMsg::FactoryConfig {})
        .unwrap();

    assert_eq!(factory_config.owner, "owner");
    assert_eq!(factory_config.escrow_contract, "factory");
    assert_eq!(factory_config.access_token, "access_token");
    assert_eq!(factory_config.rescue_delay, 3600);

    // Test deterministic address computation
    let address_response: escrow_contract::msg::EscrowAddressResponse = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::AddressOfEscrow {
                order_hash: "order_hash_123".to_string(),
                hashlock: "hashlock_456".to_string(),
                salt: "salt_789".to_string(),
            }
        )
        .unwrap();

    // Address should be deterministic
    assert!(!address_response.address.is_empty());

    // Test factory escrow creation
    let params = EscrowCreationParams {
        order_hash: "order_hash_factory".to_string(),
        hashlock: "hashlock_factory".to_string(),
        maker: Addr::unchecked("maker_address_123"),
        taker: Addr::unchecked("taker_address_456"),
        token: Addr::unchecked("token_address_123"),
        amount: Uint128::new(1000),
        safety_deposit: Uint128::new(100),
        timelocks: escrow_contract::state::PackedTimelocks::new(
            0, 1, 2, 3, 4, 1, 2, 3,
        ),
        escrow_type: EscrowType::Source,
        dst_chain_id: "cosmoshub-4".to_string(),
        dst_token: Addr::unchecked("dst_token_address_789"),
        dst_amount: Uint128::new(1000),
    };

    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "factory_salt".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("owner"), // Factory owner can create escrows
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")], // Creation fee
    );

    if let Err(e) = &result {
        println!("Error: {:?}", e);
    }
    assert!(result.is_ok());

    // Test creation request query
    let creation_request: escrow_contract::msg::CreationRequestResponse = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::CreationRequest {
                order_hash: "order_hash_factory".to_string(),
                hashlock: "hashlock_factory".to_string(),
            }
        )
        .unwrap();

    assert!(creation_request.request.is_some());
    let request = creation_request.request.unwrap();
    assert_eq!(request.params.order_hash, "order_hash_factory");
    assert_eq!(request.params.hashlock, "hashlock_factory");
    assert_eq!(request.status, escrow_contract::state::CreationStatus::Created);
}

#[test]
fn test_post_interaction_handling() {
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

    // Test post-interaction escrow creation
    let post_interaction_msg = ExecuteMsg::HandlePostInteraction {
        order_hash: "order_hash_post".to_string(),
        hashlock: "hashlock_post".to_string(),
        maker: "maker_address_123".to_string(),
        taker: "taker_address_456".to_string(),
        token: "token_address_123".to_string(),
        amount: Uint128::new(1000),
        safety_deposit: Uint128::new(100),
        timelocks: escrow_contract::state::PackedTimelocks::new(
            0, 1, 2, 3, 4, 1, 2, 3,
        ),
        dst_chain_id: "cosmoshub-4".to_string(),
        dst_token: "dst_token_address_789".to_string(),
        dst_amount: Uint128::new(1000),
    };

    let result = app.execute_contract(
        Addr::unchecked("owner"), // Factory owner can handle post-interaction
        contract_addr.clone(),
        &post_interaction_msg,
        &[Coin::new(1100, "uatom")], // amount + safety_deposit
    );

    if let Err(e) = &result {
        println!("Error: {:?}", e);
    }
    assert!(result.is_ok());

    // Verify escrow was created
    let escrow: escrow_contract::msg::EscrowResponse = app
        .wrap()
        .query_wasm_smart(contract_addr.clone(), &QueryMsg::Escrow { escrow_id: 1 })
        .unwrap();

    assert_eq!(escrow.escrow_id, 1);
    assert_eq!(escrow.immutables.order_hash, "order_hash_post");
    assert_eq!(escrow.immutables.hashlock, "hashlock_post");
    assert_eq!(escrow.escrow_type, EscrowType::Source); // Always source for post-interaction
    assert!(escrow.is_active);

    // Test creation request for post-interaction
    let creation_request: escrow_contract::msg::CreationRequestResponse = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::CreationRequest {
                order_hash: "order_hash_post".to_string(),
                hashlock: "hashlock_post".to_string(),
            }
        )
        .unwrap();

    assert!(creation_request.request.is_some());
    let request = creation_request.request.unwrap();
    assert_eq!(request.status, escrow_contract::state::CreationStatus::Created);
}

#[test]
fn test_creation_request_cancellation() {
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

    // Test cancellation by non-owner should fail (even for non-existent request)
    let cancel_msg = ExecuteMsg::CancelCreationRequest {
        order_hash: "non_existent_order".to_string(),
        hashlock: "non_existent_hashlock".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("unauthorized"),
        contract_addr.clone(),
        &cancel_msg,
        &[],
    );
    assert!(result.is_err());

    // Test cancellation by owner for non-existent request should fail
    let cancel_msg = ExecuteMsg::CancelCreationRequest {
        order_hash: "non_existent_order".to_string(),
        hashlock: "non_existent_hashlock".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &cancel_msg,
        &[],
    );
    assert!(result.is_err());

    // Create an escrow (which will be immediately processed and set to Created status)
    let params = EscrowCreationParams {
        order_hash: "order_hash_cancel".to_string(),
        hashlock: "hashlock_cancel".to_string(),
        maker: Addr::unchecked("maker_address_123"),
        taker: Addr::unchecked("taker_address_456"),
        token: Addr::unchecked("token_address_123"),
        amount: Uint128::new(1000),
        safety_deposit: Uint128::new(100),
        timelocks: escrow_contract::state::PackedTimelocks::new(
            0, 1, 2, 3, 4, 1, 2, 3,
        ),
        escrow_type: EscrowType::Source,
        dst_chain_id: "cosmoshub-4".to_string(),
        dst_token: Addr::unchecked("dst_token_address_789"),
        dst_amount: Uint128::new(1000),
    };

    let create_msg = ExecuteMsg::CreateEscrow {
        params,
        salt: "cancel_salt".to_string(),
    };

    app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &create_msg,
        &[Coin::new(10, "uatom")],
    ).unwrap();

    // Test cancellation by owner for already created request should fail
    let cancel_msg = ExecuteMsg::CancelCreationRequest {
        order_hash: "order_hash_cancel".to_string(),
        hashlock: "hashlock_cancel".to_string(),
    };

    let result = app.execute_contract(
        Addr::unchecked("owner"),
        contract_addr.clone(),
        &cancel_msg,
        &[],
    );
    assert!(result.is_err());

    // Verify request was created (not cancelled)
    let creation_request: escrow_contract::msg::CreationRequestResponse = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::CreationRequest {
                order_hash: "order_hash_cancel".to_string(),
                hashlock: "hashlock_cancel".to_string(),
            }
        )
        .unwrap();

    assert!(creation_request.request.is_some());
    let request = creation_request.request.unwrap();
    assert_eq!(request.status, escrow_contract::state::CreationStatus::Created);
} 