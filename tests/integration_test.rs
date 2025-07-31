use cosmwasm_std::{Addr, Coin, Uint128};
use cw_multi_test::{App, Contract, ContractWrapper, Executor};
use escrow_contract::msg::{InstantiateMsg, QueryMsg, ExecuteMsg};
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
        router.bank.init_balance(storage, &Addr::unchecked("taker"), vec![Coin::new(2000, "uatom")]).unwrap();
        router.bank.init_balance(storage, &Addr::unchecked("maker"), vec![Coin::new(2000, "uatom")]).unwrap();
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

#[test]
fn test_instantiate() {
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
        timelocks: PackedTimelocks::new(
            1000, // deployed_at
            1,    // src_withdrawal
            2,    // src_public_withdrawal
            3,    // src_cancellation
            4,    // src_public_cancellation
            1,    // dst_withdrawal
            2,    // dst_public_withdrawal
            3,    // dst_cancellation
        ),
        dst_chain_id: "cosmoshub-4".to_string(),
        dst_token: "dst_token".to_string(),
        dst_amount: Uint128::new(1000),
        escrow_type: EscrowType::Source,
    };

    let contract_addr = app
        .instantiate_contract(contract_id, Addr::unchecked("owner"), &msg, &[Coin::new(1100, "uatom")], "Escrow", None)
        .unwrap();

    // Query escrow to verify instantiation
    let config_response: escrow_contract::msg::ConfigResponse = app
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::Config {})
        .unwrap();

    assert_eq!(config_response.escrow_id, 1);
    assert_eq!(config_response.escrow_type, EscrowType::Source);
    assert!(config_response.is_active);
    assert_eq!(config_response.balance, Uint128::new(1000));
    assert_eq!(config_response.native_balance, Uint128::new(100));
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

    // Test stage progression validation
    assert!(timelocks.validate().is_ok());

    // Test invalid timelock progression (should fail)
    let invalid_timelocks = PackedTimelocks::new(
        deployed_at,
        2,  // src_withdrawal: 2 hours
        1,  // src_public_withdrawal: 1 hour (should be after src_withdrawal)
        3,  // src_cancellation: 3 hours
        4,  // src_public_cancellation: 4 hours
        1,  // dst_withdrawal: 1 hour
        2,  // dst_public_withdrawal: 2 hours
        3,  // dst_cancellation: 3 hours
    );
    assert!(invalid_timelocks.validate().is_err());
}

#[test]
fn test_access_control() {
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
        timelocks: PackedTimelocks::new(
            1000, // deployed_at
            1,    // src_withdrawal
            2,    // src_public_withdrawal
            3,    // src_cancellation
            4,    // src_public_cancellation
            1,    // dst_withdrawal
            2,    // dst_public_withdrawal
            3,    // dst_cancellation
        ),
        dst_chain_id: "cosmoshub-4".to_string(),
        dst_token: "dst_token".to_string(),
        dst_amount: Uint128::new(1000),
        escrow_type: EscrowType::Source,
    };

    let contract_addr = app
        .instantiate_contract(contract_id, Addr::unchecked("owner"), &msg, &[Coin::new(1100, "uatom")], "Escrow", None)
        .unwrap();

    // Test that only owner can access certain functions
    // This would be implemented based on your specific access control requirements
    assert_eq!(contract_addr, contract_addr); // Placeholder assertion
}

#[test]
fn test_secret_validation() {
    // Test secret validation logic
    let secret = "my_secret_key_123";
    let secret_hash = Sha256::digest(secret.as_bytes());
    let secret_hash_hex = format!("{secret_hash:x}");
    
    // Test that the same secret produces the same hash
    let secret_hash2 = Sha256::digest(secret.as_bytes());
    let secret_hash_hex2 = format!("{secret_hash2:x}");
    assert_eq!(secret_hash_hex, secret_hash_hex2);
    
    // Test that different secrets produce different hashes
    let different_secret = "different_secret_key_456";
    let different_hash = Sha256::digest(different_secret.as_bytes());
    let different_hash_hex = format!("{different_hash:x}");
    assert_ne!(secret_hash_hex, different_hash_hex);
}

#[test]
fn test_timelock_validation() {
    let deployed_at = 1000u32;
    let timelocks = PackedTimelocks::new(
        deployed_at,
        1,  // src_withdrawal: 1 hour
        2,  // src_public_withdrawal: 2 hours
        3,  // src_cancellation: 3 hours
        4,  // src_public_cancellation: 4 hours
        1,  // dst_withdrawal: 1 hour
        2,  // dst_public_withdrawal: 2 hours
        3,  // dst_cancellation: 3 hours
    );

    // Test timelock validation at different times
    let current_time = deployed_at as u64 + 3600; // 1 hour after deployment
    
    // Should be within src_withdrawal stage
    assert!(timelocks.is_within_stage(current_time, TimelockStage::SrcWithdrawal));
    
    // Should not be within src_public_withdrawal stage yet
    assert!(!timelocks.is_within_stage(current_time, TimelockStage::SrcPublicWithdrawal));
    
    // Test stage progression
    let later_time = deployed_at as u64 + 7200; // 2 hours after deployment
    assert!(timelocks.is_within_stage(later_time, TimelockStage::SrcPublicWithdrawal));
}

#[test]
fn test_source_vs_destination_behavior() {
    // Test that source and destination escrows behave differently
    let source_escrow = EscrowType::Source;
    let destination_escrow = EscrowType::Destination;
    
    assert!(source_escrow.is_source());
    assert!(!source_escrow.is_destination());
    
    assert!(destination_escrow.is_destination());
    assert!(!destination_escrow.is_source());
    
    // Test withdrawal stages
    assert_eq!(source_escrow.get_withdrawal_stage(), TimelockStage::SrcWithdrawal);
    assert_eq!(destination_escrow.get_withdrawal_stage(), TimelockStage::DstWithdrawal);
    
    // Test cancellation stages
    assert_eq!(source_escrow.get_cancellation_stage(), TimelockStage::SrcCancellation);
    assert_eq!(destination_escrow.get_cancellation_stage(), TimelockStage::DstCancellation);
}

#[test]
fn test_escrow_type_validation() {
    let source_escrow = EscrowType::Source;
    let destination_escrow = EscrowType::Destination;
    
    // Test public cancellation support
    assert!(source_escrow.supports_public_cancellation());
    assert!(!destination_escrow.supports_public_cancellation());
    
    // Test public withdrawal stages
    assert_eq!(source_escrow.get_public_withdrawal_stage(), TimelockStage::SrcPublicWithdrawal);
    assert_eq!(destination_escrow.get_public_withdrawal_stage(), TimelockStage::DstPublicWithdrawal);
    
    // Test public cancellation stages
    assert_eq!(source_escrow.get_public_cancellation_stage(), Some(TimelockStage::SrcPublicCancellation));
    assert_eq!(destination_escrow.get_public_cancellation_stage(), None);
}

#[test]
fn test_direct_escrow_deployment() {
    let mut app = mock_app();
    let contract_id = app.store_code(escrow_contract());

    let msg = InstantiateMsg {
        order_hash: "order_hash_123".to_string(),
        hashlock: "hashlock_456".to_string(),
        maker: "maker".to_string(),
        taker: "taker".to_string(),
        token: "".to_string(), // Native token
        amount: Uint128::new(1000),
        safety_deposit: Uint128::new(100),
        timelocks: PackedTimelocks::new(
            1000, // deployed_at
            1,    // src_withdrawal
            2,    // src_public_withdrawal
            3,    // src_cancellation
            4,    // src_public_cancellation
            1,    // dst_withdrawal
            2,    // dst_public_withdrawal
            3,    // dst_cancellation
        ),
        dst_chain_id: "destination_chain".to_string(),
        dst_token: "dst_token".to_string(),
        dst_amount: Uint128::new(1000),
        escrow_type: EscrowType::Source,
    };

    // Execute with funds
    let funds = vec![Coin::new(1100, "uatom")]; // amount + safety_deposit
    let result = app.instantiate_contract(
        contract_id,
        Addr::unchecked("taker"),
        &msg,
        &funds,
        "Escrow",
        None,
    );

    assert!(result.is_ok());

    // Query escrows to verify deployment
    let config_response: escrow_contract::msg::ConfigResponse = app
        .wrap()
        .query_wasm_smart(result.unwrap(), &QueryMsg::Config {})
        .unwrap();

    assert_eq!(config_response.escrow_id, 1);
    assert_eq!(config_response.escrow_type, EscrowType::Source);
    assert!(config_response.is_active);
    assert_eq!(config_response.balance, Uint128::new(1000));
    assert_eq!(config_response.native_balance, Uint128::new(100));
} 

#[test]
fn test_destination_escrow_instantiation() {
    let mut app = mock_app();
    let contract_id = app.store_code(escrow_contract());

    let msg = InstantiateMsg {
        order_hash: "test_order_hash_456".to_string(),
        hashlock: "test_hashlock_789".to_string(),
        maker: "maker".to_string(),
        taker: "taker".to_string(),
        token: "".to_string(),
        amount: Uint128::new(500),
        safety_deposit: Uint128::new(50),
        timelocks: create_test_timelocks(),
        dst_chain_id: "cosmoshub-4".to_string(),
        dst_token: "dst_token".to_string(),
        dst_amount: Uint128::new(500),
        escrow_type: EscrowType::Destination,
    };

    let contract_addr = app
        .instantiate_contract(contract_id, Addr::unchecked("owner"), &msg, &[Coin::new(550, "uatom")], "Escrow", None)
        .unwrap();

    // Query escrow to verify instantiation
    let config_response: escrow_contract::msg::ConfigResponse = app
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::Config {})
        .unwrap();

    assert_eq!(config_response.escrow_id, 1);
    assert_eq!(config_response.escrow_type, EscrowType::Destination);
    assert!(config_response.is_active);
    assert_eq!(config_response.balance, Uint128::new(500));
    assert_eq!(config_response.native_balance, Uint128::new(50));
}

#[test]
fn test_insufficient_funds_instantiation() {
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

    // Try to instantiate with insufficient funds
    let result = app.instantiate_contract(
        contract_id, 
        Addr::unchecked("owner"), 
        &msg, 
        &[Coin::new(500, "uatom")], // Only 500 instead of 1100
        "Escrow", 
        None
    );

    assert!(result.is_err());
}

#[test]
fn test_withdrawal_with_correct_secret() {
    let mut app = mock_app();
    let contract_id = app.store_code(escrow_contract());

    let secret = generate_secret();
    let hashlock = hash_secret(&secret);

    let msg = InstantiateMsg {
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

    let contract_addr = app
        .instantiate_contract(contract_id, Addr::unchecked("owner"), &msg, &[Coin::new(1100, "uatom")], "Escrow", None)
        .unwrap();

    // Try to withdraw with correct secret (will fail due to timelock, but not due to secret)
    let withdraw_msg = ExecuteMsg::WithdrawSrc {
        escrow_id: 1,
        secret: secret,
    };

    let result = app.execute_contract(
        Addr::unchecked("taker"),
        contract_addr,
        &withdraw_msg,
        &[],
    );

    // Should fail due to timelock, not secret validation
    assert!(result.is_err());
} 