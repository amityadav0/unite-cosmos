use cosmwasm_std::{Addr, Coin, Empty, Uint128};
use cw_multi_test::{App, Contract, ContractWrapper, Executor};

use escrow_contract::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, ConfigResponse, EscrowResponse};

fn mock_app() -> App {
    App::new(|router, _api, storage| {
        // Set initial balances for test accounts
        router
            .bank
            .init_balance(storage, &Addr::unchecked("owner"), vec![Coin::new(10000, "uatom")])
            .unwrap();
        router
            .bank
            .init_balance(storage, &Addr::unchecked("factory"), vec![Coin::new(5000, "uatom")])
            .unwrap();
    })
}

fn escrow_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        escrow_contract::execute,
        escrow_contract::instantiate,
        escrow_contract::query,
    );
    Box::new(contract)
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
        .instantiate_contract(contract_id, Addr::unchecked("owner"), &msg, &[], "escrow", None)
        .unwrap();

    let config: ConfigResponse = app
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
        .instantiate_contract(contract_id, Addr::unchecked("owner"), &msg, &[], "escrow", None)
        .unwrap();

    let create_escrow_msg = ExecuteMsg::CreateEscrow {
        order_hash: "order_hash_123".to_string(),
        hashlock: "hashlock_456".to_string(),
        maker: "maker_address_123".to_string(),
        taker: "taker_address_456".to_string(),
        token: "token_address_123".to_string(), // CW20 token
        amount: Uint128::new(1000),
        safety_deposit: Uint128::new(100),
        timelocks: escrow_contract::state::PackedTimelocks::new(
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

    // Only factory can create escrows
    let result = app.execute_contract(
        Addr::unchecked("factory"),
        contract_addr.clone(),
        &create_escrow_msg,
        &[Coin::new(1100, "uatom")], // amount + safety_deposit
    );

    if let Err(e) = &result {
        println!("Error: {:?}", e);
    }
    assert!(result.is_ok());

    let escrow: EscrowResponse = app
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::Escrow { escrow_id: 1 })
        .unwrap();

    assert_eq!(escrow.escrow_id, 1);
    assert_eq!(escrow.immutables.order_hash, "order_hash_123");
    assert_eq!(escrow.immutables.hashlock, "hashlock_456");
    assert_eq!(escrow.immutables.maker, Addr::unchecked("maker_address_123"));
    assert_eq!(escrow.immutables.taker, Addr::unchecked("taker_address_456"));
    assert_eq!(escrow.balance, Uint128::new(1000));
    assert_eq!(escrow.native_balance, Uint128::new(100));
    assert!(escrow.is_active);
    assert!(escrow.is_src);
} 