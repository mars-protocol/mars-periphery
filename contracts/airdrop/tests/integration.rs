use cosmwasm_bignumber::Uint256;
use cosmwasm_std::testing::{mock_env, MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{attr, Addr, Timestamp, Uint128};
use mars_periphery::airdrop::{
    ClaimResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg, StateResponse,
    UserInfoResponse,
};
use terra_multi_test::{App, BankKeeper, ContractWrapper, Executor, TerraMockQuerier};

fn mock_app() -> App {
    let api = MockApi::default();
    let env = mock_env();
    let bank = BankKeeper::new();
    let storage = MockStorage::new();
    let tmq = TerraMockQuerier::new(MockQuerier::new(&[]));

    App::new(api, env.block, bank, storage, tmq)
}

fn init_contracts(app: &mut App) -> (Addr, Addr, InstantiateMsg) {
    let owner = Addr::unchecked("contract_owner");

    // Instantiate MARS Token Contract
    let mars_token_contract = Box::new(ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    ));

    let mars_token_code_id = app.store_code(mars_token_contract);

    let msg = cw20_base::msg::InstantiateMsg {
        name: String::from("Astro token"),
        symbol: String::from("MARS"),
        decimals: 6,
        initial_balances: vec![],
        mint: Some(cw20::MinterResponse {
            minter: owner.to_string(),
            cap: None,
        }),
        marketing: None,
    };

    let mars_token_instance = app
        .instantiate_contract(
            mars_token_code_id,
            owner.clone(),
            &msg,
            &[],
            String::from("MARS"),
            None,
        )
        .unwrap();

    // Instantiate Airdrop Contract
    let airdrop_contract = Box::new(ContractWrapper::new(
        mars_airdrop::contract::execute,
        mars_airdrop::contract::instantiate,
        mars_airdrop::contract::query,
    ));

    let airdrop_code_id = app.store_code(airdrop_contract);

    let aidrop_instantiate_msg = InstantiateMsg {
        owner: Some(owner.clone().to_string()),
        mars_token_address: mars_token_instance.clone().into_string(),
        terra_merkle_roots: Some(vec!["terra_merkle_roots".to_string()]),
        evm_merkle_roots: Some(vec!["evm_merkle_roots".to_string()]),
        from_timestamp: Some(1_000_00),
        to_timestamp: 100_000_00,
        auction_contract_address: String::from("auction_contract_address"),
        total_airdrop_size: Uint128::new(100_000_000_000),
    };

    // Init contract
    let airdrop_instance = app
        .instantiate_contract(
            airdrop_code_id,
            owner.clone(),
            &aidrop_instantiate_msg,
            &[],
            "airdrop",
            None,
        )
        .unwrap();

    (
        airdrop_instance,
        mars_token_instance,
        aidrop_instantiate_msg,
    )
}

fn mint_some_mars(
    app: &mut App,
    owner: Addr,
    mars_token_instance: Addr,
    amount: Uint128,
    to: String,
) {
    let msg = cw20::Cw20ExecuteMsg::Mint {
        recipient: to.clone(),
        amount: amount,
    };
    let res = app
        .execute_contract(owner.clone(), mars_token_instance.clone(), &msg, &[])
        .unwrap();
    assert_eq!(res.events[1].attributes[1], attr("action", "mint"));
    assert_eq!(res.events[1].attributes[2], attr("to", to));
    assert_eq!(res.events[1].attributes[3], attr("amount", amount));
}

// Helper function. Enables claims (MARS Withdrawals) from the Airdrop contract
fn enable_claims(app: &mut App, airdrop_instance: Addr, auction_contract_address: Addr) {
    let msg = ExecuteMsg::EnableClaims {};
    app.execute_contract(
        auction_contract_address.clone(),
        airdrop_instance.clone(),
        &msg,
        &[],
    )
    .unwrap();
    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&airdrop_instance, &QueryMsg::Config {})
        .unwrap();
    assert_eq!(true, resp.are_claims_allowed);
}

#[test]
fn proper_initialization() {
    let mut app = mock_app();
    let (airdrop_instance, _, init_msg) = init_contracts(&mut app);

    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&airdrop_instance, &QueryMsg::Config {})
        .unwrap();

    // Check config
    assert_eq!(init_msg.mars_token_address, resp.mars_token_address);
    assert_eq!(
        init_msg.auction_contract_address,
        resp.auction_contract_address
    );
    assert_eq!(init_msg.owner.unwrap(), resp.owner);
    assert_eq!(
        init_msg.terra_merkle_roots.unwrap(),
        resp.terra_merkle_roots
    );
    assert_eq!(init_msg.evm_merkle_roots.unwrap(), resp.evm_merkle_roots);
    assert_eq!(init_msg.from_timestamp.unwrap(), resp.from_timestamp);
    assert_eq!(init_msg.to_timestamp, resp.to_timestamp);

    // Check state
    let resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&airdrop_instance, &QueryMsg::State {})
        .unwrap();

    assert_eq!(init_msg.total_airdrop_size, resp.total_airdrop_size);
    assert_eq!(init_msg.total_airdrop_size, resp.unclaimed_tokens);
    assert_eq!(Uint128::zero(), resp.total_delegated_amount);
}

#[test]
fn update_config() {
    let mut app = mock_app();
    let (airdrop_instance, _, init_msg) = init_contracts(&mut app);

    // Only owner can update
    let err = app
        .execute_contract(
            Addr::unchecked("wrong_owner"),
            airdrop_instance.clone(),
            &ExecuteMsg::UpdateConfig {
                owner: None,
                auction_contract_address: None,
                terra_merkle_roots: None,
                evm_merkle_roots: None,
                from_timestamp: None,
                to_timestamp: None,
            },
            &[],
        )
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        "Generic error: Only owner can update configuration"
    );

    let new_owner = String::from("new_owner");
    let terra_merkle_roots = vec!["new_terra_merkle_roots".to_string()];
    let evm_merkle_roots = vec!["new_evm_merkle_roots".to_string()];
    let from_timestamp = 2_000_00;
    let to_timestamp = 200_000_00;

    let update_msg = ExecuteMsg::UpdateConfig {
        owner: Some(new_owner.clone()),
        auction_contract_address: None,
        terra_merkle_roots: Some(terra_merkle_roots.clone()),
        evm_merkle_roots: Some(evm_merkle_roots.clone()),
        from_timestamp: Some(from_timestamp),
        to_timestamp: Some(to_timestamp),
    };

    // should be a success
    app.execute_contract(
        Addr::unchecked(init_msg.owner.unwrap()),
        airdrop_instance.clone(),
        &update_msg,
        &[],
    )
    .unwrap();

    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&airdrop_instance, &QueryMsg::Config {})
        .unwrap();

    // Check config and make sure all fields are updated
    assert_eq!(new_owner, resp.owner);
    assert_eq!(terra_merkle_roots, resp.terra_merkle_roots);
    assert_eq!(evm_merkle_roots, resp.evm_merkle_roots);
    assert_eq!(from_timestamp, resp.from_timestamp);
    assert_eq!(to_timestamp, resp.to_timestamp);
}

#[cfg(test)]
#[test]
fn test_transfer_unclaimed_tokens() {
    let mut app = mock_app();
    let (airdrop_instance, mars_instance, init_msg) = init_contracts(&mut app);

    // mint MARS for to Airdrop Contract
    mint_some_mars(
        &mut app,
        Addr::unchecked(init_msg.owner.clone().unwrap()),
        mars_instance.clone(),
        Uint128::new(100_000_000_000),
        airdrop_instance.clone().to_string(),
    );

    // Check Airdrop Contract balance
    let bal_resp: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &mars_instance,
            &cw20::Cw20QueryMsg::Balance {
                address: airdrop_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(100_000_000_000u64), bal_resp.balance);

    // Can only be called by the owner
    let err = app
        .execute_contract(
            Addr::unchecked("wrong_owner"),
            airdrop_instance.clone(),
            &ExecuteMsg::TransferUnclaimedTokens {
                recepient: "recepient".to_string(),
                amount: Uint128::from(1000000 as u64),
            },
            &[],
        )
        .unwrap_err();

    assert_eq!(err.to_string(), "Generic error: Sender not authorized!");

    // claim period is not over
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_00)
    });

    // Can only be called after the claim period is over
    let err = app
        .execute_contract(
            Addr::unchecked(init_msg.owner.clone().unwrap()),
            airdrop_instance.clone(),
            &ExecuteMsg::TransferUnclaimedTokens {
                recepient: "recepient".to_string(),
                amount: Uint128::from(1000000 as u64),
            },
            &[],
        )
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        "Generic error: 9900000 seconds left before unclaimed tokens can be transferred"
    );

    // claim period is over
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_00_00)
    });

    // Amount needs to be less than unclaimed_tokens balance
    let err = app
        .execute_contract(
            Addr::unchecked(init_msg.owner.clone().unwrap()),
            airdrop_instance.clone(),
            &ExecuteMsg::TransferUnclaimedTokens {
                recepient: "recepient".to_string(),
                amount: Uint128::from(100_000_000_0000 as u64),
            },
            &[],
        )
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        "Generic error: Amount cannot exceed unclaimed token balance"
    );

    // Should successfully transfer and update state
    app.execute_contract(
        Addr::unchecked(init_msg.owner.clone().unwrap()),
        airdrop_instance.clone(),
        &ExecuteMsg::TransferUnclaimedTokens {
            recepient: "recepient".to_string(),
            amount: Uint128::from(100_000_00 as u64),
        },
        &[],
    )
    .unwrap();

    let state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&airdrop_instance, &QueryMsg::State {})
        .unwrap();

    // Check config and make sure all fields are updated
    assert_eq!(
        Uint128::from(100_000_000_000u64),
        state_resp.total_airdrop_size
    );
    assert_eq!(Uint128::from(0u32), state_resp.total_delegated_amount);
    assert_eq!(Uint128::from(99990000000u64), state_resp.unclaimed_tokens);
}

#[cfg(test)]
#[test]
fn test_claim_by_terra_user() {
    let mut app = mock_app();
    let (airdrop_instance, mars_instance, init_msg) = init_contracts(&mut app);

    // mint MARS for to Airdrop Contract
    mint_some_mars(
        &mut app,
        Addr::unchecked(init_msg.owner.clone().unwrap()),
        mars_instance.clone(),
        Uint128::new(100_000_000_000),
        airdrop_instance.clone().to_string(),
    );

    // Check Airdrop Contract balance
    let bal_resp: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &mars_instance,
            &cw20::Cw20QueryMsg::Balance {
                address: airdrop_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(100_000_000_000u64), bal_resp.balance);

    let terra_merkle_roots =
        vec!["cdcdfad1c342f5f55a2639dcae7321a64cd000807fa24c2c4ddaa944fd52d34e".to_string()];
    let update_msg = ExecuteMsg::UpdateConfig {
        owner: None,
        auction_contract_address: None,
        terra_merkle_roots: Some(terra_merkle_roots.clone()),
        evm_merkle_roots: None,
        from_timestamp: None,
        to_timestamp: None,
    };

    // Update Config :: should be a success
    app.execute_contract(
        Addr::unchecked(init_msg.owner.clone().unwrap()),
        airdrop_instance.clone(),
        &update_msg,
        &[],
    )
    .unwrap();

    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&airdrop_instance, &QueryMsg::Config {})
        .unwrap();

    // Check config and make sure all fields are updated
    assert_eq!(init_msg.owner.clone().unwrap(), resp.owner);
    assert_eq!(terra_merkle_roots, resp.terra_merkle_roots);
    assert_eq!(init_msg.evm_merkle_roots.unwrap(), resp.evm_merkle_roots);
    assert_eq!(init_msg.from_timestamp.unwrap(), resp.from_timestamp);
    assert_eq!(init_msg.to_timestamp, resp.to_timestamp);

    // Claim period has not started yet
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000)
    });

    let mut claim_msg = ExecuteMsg::ClaimByTerraUser {
        claim_amount: Uint128::from(250000000 as u64),
        merkle_proof: vec![
            "7719b79a65e5aa0bbfd144cf5373138402ab1c374d9049e490b5b61c23d90065".to_string(),
            "60368f2058e0fb961a7721a241f9b973c3dd6c57e10a627071cd81abca6aa490".to_string(),
        ],
        root_index: 0,
    };
    let mut claim_msg_wrong_amount = ExecuteMsg::ClaimByTerraUser {
        claim_amount: Uint128::from(210000000 as u64),
        merkle_proof: vec![
            "7719b79a65e5aa0bbfd144cf5373138402ab1c374d9049e490b5b61c23d90065".to_string(),
            "60368f2058e0fb961a7721a241f9b973c3dd6c57e10a627071cd81abca6aa490".to_string(),
        ],
        root_index: 0,
    };
    let mut claim_msg_incorrect_proof = ExecuteMsg::ClaimByTerraUser {
        claim_amount: Uint128::from(250000000 as u64),
        merkle_proof: vec![
            "7719b79a65e4aa0bbfd144cf5373138402ab1c374d9049e490b5b61c23d90065".to_string(),
            "60368f2058e0fb961a7721a241f9b973c3dd6c57e10a627071cd81abca6aa490".to_string(),
        ],
        root_index: 0,
    };

    // ################################
    // USER #1 :: Claims not allowed. MARS Rewards will Not be transferred to the user
    // ################################

    // **** "Claim not allowed" Error should be returned ****
    let mut claim_f = app
        .execute_contract(
            Addr::unchecked("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string()),
            airdrop_instance.clone(),
            &claim_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(claim_f.to_string(), "Generic error: Claim not allowed");

    // Update Block to test successful claim
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_05)
    });

    // **** "Incorrect Merkle Root Index" Error should be returned ****
    claim_f = app
        .execute_contract(
            Addr::unchecked("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string()),
            airdrop_instance.clone(),
            &ExecuteMsg::ClaimByTerraUser {
                claim_amount: Uint128::from(250000000 as u64),
                merkle_proof: vec![
                    "7719b79a65e4aa0bbfd144cf5373138402ab1c374d9049e490b5b61c23d90065".to_string(),
                    "60368f2058e0fb961a7721a241f9b973c3dd6c57e10a627071cd81abca6aa490".to_string(),
                ],
                root_index: 5,
            },
            &[],
        )
        .unwrap_err();

    assert_eq!(
        claim_f.to_string(),
        "Generic error: Incorrect Merkle Root Index"
    );

    // **** "Incorrect Merkle Proof" Error should be returned ****
    claim_f = app
        .execute_contract(
            Addr::unchecked("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string()),
            airdrop_instance.clone(),
            &claim_msg_incorrect_proof,
            &[],
        )
        .unwrap_err();

    assert_eq!(claim_f.to_string(), "Generic error: Incorrect Merkle Proof");

    // **** "Incorrect Merkle Proof" Error should be returned ****
    claim_f = app
        .execute_contract(
            Addr::unchecked("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string()),
            airdrop_instance.clone(),
            &claim_msg_wrong_amount,
            &[],
        )
        .unwrap_err();

    assert_eq!(claim_f.to_string(), "Generic error: Incorrect Merkle Proof");

    // **** User should successfully claim the Airdrop ****

    // Check :: User hasn't yet claimed the airdrop
    let resp: ClaimResponse = app
        .wrap()
        .query_wasm_smart(
            &airdrop_instance,
            &QueryMsg::HasUserClaimed {
                address: "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string(),
            },
        )
        .unwrap();
    assert_eq!(false, resp.is_claimed);

    // Should be a success
    let mut success_ = app
        .execute_contract(
            Addr::unchecked("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string()),
            airdrop_instance.clone(),
            &claim_msg,
            &[],
        )
        .unwrap();

    assert_eq!(
        success_.events[1].attributes[1],
        attr("action", "Airdrop::ExecuteMsg::ClaimByTerraUser")
    );
    assert_eq!(
        success_.events[1].attributes[2],
        attr("claimer", "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp")
    );
    assert_eq!(
        success_.events[1].attributes[3],
        attr("airdrop", "250000000")
    );

    // Check :: User successfully claimed the airdrop
    let mut claim_query_resp: ClaimResponse = app
        .wrap()
        .query_wasm_smart(
            &airdrop_instance,
            &QueryMsg::HasUserClaimed {
                address: "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string(),
            },
        )
        .unwrap();
    assert_eq!(true, claim_query_resp.is_claimed);

    // Check :: User state
    let mut user_info_query_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &airdrop_instance,
            &QueryMsg::UserInfo {
                address: "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(250000000u64),
        user_info_query_resp.airdrop_amount
    );
    assert_eq!(Uint128::from(0u64), user_info_query_resp.delegated_amount);
    assert_eq!(false, user_info_query_resp.tokens_withdrawn);

    // Check :: Contract state
    let mut state_query_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&airdrop_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(
        Uint128::from(100_000_000_000u64),
        state_query_resp.total_airdrop_size
    );
    assert_eq!(Uint128::from(0u64), state_query_resp.total_delegated_amount);
    assert_eq!(
        Uint128::from(99750000000u64),
        state_query_resp.unclaimed_tokens
    );

    // **** "Already claimed" Error should be returned ****

    claim_f = app
        .execute_contract(
            Addr::unchecked("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string()),
            airdrop_instance.clone(),
            &claim_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(claim_f.to_string(), "Generic error: Already claimed");

    // Enable MARS Withdrawals
    enable_claims(
        &mut app,
        Addr::unchecked(airdrop_instance.clone()),
        Addr::unchecked(init_msg.auction_contract_address),
    );

    // ################################
    // USER #2 :: MARS Rewards will be transferred to the user
    // ################################

    claim_msg = ExecuteMsg::ClaimByTerraUser {
        claim_amount: Uint128::from(1 as u64),
        merkle_proof: vec![
            "7fd0f6ac4074cef9f89eedcf72459ad7b0891855f8084b54dc7de7569849d1c8".to_string(),
            "4fab6b0ef8d988835ad968d03d61de408772d033e9ce734394bb623309c5d7fc".to_string(),
        ],
        root_index: 0,
    };
    claim_msg_wrong_amount = ExecuteMsg::ClaimByTerraUser {
        claim_amount: Uint128::from(2 as u64),
        merkle_proof: vec![
            "7fd0f6ac4074cef9f89eedcf72459ad7b0891855f8084b54dc7de7569849d1c8".to_string(),
            "4fab6b0ef8d988835ad968d03d61de408772d033e9ce734394bb623309c5d7fc".to_string(),
        ],
        root_index: 0,
    };
    claim_msg_incorrect_proof = ExecuteMsg::ClaimByTerraUser {
        claim_amount: Uint128::from(1 as u64),
        merkle_proof: vec![
            "7fd0f6ac4074cef1f89eedcf72459ad7b0891855f8084b54dc7de7569849d1c8".to_string(),
            "4fab6b0ef8d988835ad968d03d61de408772d033e9ce734394bb623309c5d7fc".to_string(),
        ],
        root_index: 0,
    };

    // **** "Incorrect Merkle Root Index" Error should be returned ****
    claim_f = app
        .execute_contract(
            Addr::unchecked("terra1757tkx08n0cqrw7p86ny9lnxsqeth0wgp0em95".to_string()),
            airdrop_instance.clone(),
            &ExecuteMsg::ClaimByTerraUser {
                claim_amount: Uint128::from(1 as u64),
                merkle_proof: vec![
                    "7fd0f6ac4074cef9f89eedcf72459ad7b0891855f8084b54dc7de7569849d1c8".to_string(),
                    "4fab6b0ef8d988835ad968d03d61de408772d033e9ce734394bb623309c5d7fc".to_string(),
                ],
                root_index: 5,
            },
            &[],
        )
        .unwrap_err();

    assert_eq!(
        claim_f.to_string(),
        "Generic error: Incorrect Merkle Root Index"
    );

    // **** "Incorrect Merkle Proof" Error should be returned ****
    claim_f = app
        .execute_contract(
            Addr::unchecked("terra1757tkx08n0cqrw7p86ny9lnxsqeth0wgp0em95".to_string()),
            airdrop_instance.clone(),
            &claim_msg_incorrect_proof,
            &[],
        )
        .unwrap_err();

    assert_eq!(claim_f.to_string(), "Generic error: Incorrect Merkle Proof");

    // **** "Incorrect Merkle Proof" Error should be returned ****
    claim_f = app
        .execute_contract(
            Addr::unchecked("terra1757tkx08n0cqrw7p86ny9lnxsqeth0wgp0em95".to_string()),
            airdrop_instance.clone(),
            &claim_msg_wrong_amount,
            &[],
        )
        .unwrap_err();

    assert_eq!(claim_f.to_string(), "Generic error: Incorrect Merkle Proof");

    // **** User should successfully claim the Airdrop ****

    // Check :: User hasn't yet claimed the airdrop
    let resp: ClaimResponse = app
        .wrap()
        .query_wasm_smart(
            &airdrop_instance,
            &QueryMsg::HasUserClaimed {
                address: "terra1757tkx08n0cqrw7p86ny9lnxsqeth0wgp0em95".to_string(),
            },
        )
        .unwrap();
    assert_eq!(false, resp.is_claimed);

    // Should be a success
    success_ = app
        .execute_contract(
            Addr::unchecked("terra1757tkx08n0cqrw7p86ny9lnxsqeth0wgp0em95".to_string()),
            airdrop_instance.clone(),
            &claim_msg,
            &[],
        )
        .unwrap();

    assert_eq!(
        success_.events[1].attributes[1],
        attr("action", "Airdrop::ExecuteMsg::ClaimByTerraUser")
    );
    assert_eq!(
        success_.events[1].attributes[2],
        attr("claimer", "terra1757tkx08n0cqrw7p86ny9lnxsqeth0wgp0em95")
    );
    assert_eq!(success_.events[1].attributes[3], attr("airdrop", "1"));

    // Check user MARS balance
    let bal_resp: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &mars_instance,
            &cw20::Cw20QueryMsg::Balance {
                address: "terra1757tkx08n0cqrw7p86ny9lnxsqeth0wgp0em95".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1u64), bal_resp.balance);

    // Check :: User successfully claimed the airdrop
    claim_query_resp = app
        .wrap()
        .query_wasm_smart(
            &airdrop_instance,
            &QueryMsg::HasUserClaimed {
                address: "terra1757tkx08n0cqrw7p86ny9lnxsqeth0wgp0em95".to_string(),
            },
        )
        .unwrap();
    assert_eq!(true, claim_query_resp.is_claimed);

    // Check :: User state
    user_info_query_resp = app
        .wrap()
        .query_wasm_smart(
            &airdrop_instance,
            &QueryMsg::UserInfo {
                address: "terra1757tkx08n0cqrw7p86ny9lnxsqeth0wgp0em95".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1u64), user_info_query_resp.airdrop_amount);
    assert_eq!(Uint128::from(0u64), user_info_query_resp.delegated_amount);
    assert_eq!(true, user_info_query_resp.tokens_withdrawn);

    // Check :: Contract state
    state_query_resp = app
        .wrap()
        .query_wasm_smart(&airdrop_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(
        Uint128::from(100000000000u64),
        state_query_resp.total_airdrop_size
    );
    assert_eq!(Uint128::from(0u64), state_query_resp.total_delegated_amount);
    assert_eq!(
        Uint128::from(99749999999u64),
        state_query_resp.unclaimed_tokens
    );

    // **** "Already claimed" Error should be returned ****

    claim_f = app
        .execute_contract(
            Addr::unchecked("terra1757tkx08n0cqrw7p86ny9lnxsqeth0wgp0em95".to_string()),
            airdrop_instance.clone(),
            &claim_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(claim_f.to_string(), "Generic error: Already claimed");

    // Claim period has concluded
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_000_000)
    });

    // **** "Claim period has concluded" Error should be returned ****

    claim_f = app
        .execute_contract(
            Addr::unchecked("terra1757tkx08n0cqrw7p86ny9lnxsqeth0wgp0em95".to_string()),
            airdrop_instance.clone(),
            &claim_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(
        claim_f.to_string(),
        "Generic error: Claim period has concluded"
    );
}

#[cfg(test)]
#[test]
fn test_claim_by_evm_user_claims_disabled() {
    let mut app = mock_app();
    let (airdrop_instance, mars_instance, init_msg) = init_contracts(&mut app);

    // mint MARS for to Airdrop Contract
    mint_some_mars(
        &mut app,
        Addr::unchecked(init_msg.owner.clone().unwrap()),
        mars_instance.clone(),
        Uint128::new(100_000_000_000),
        airdrop_instance.clone().to_string(),
    );

    // Check Airdrop Contract balance
    let bal_resp: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &mars_instance,
            &cw20::Cw20QueryMsg::Balance {
                address: airdrop_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(100_000_000_000u64), bal_resp.balance);

    let evm_merkle_roots =
        vec!["1680ce46cb2c916f103afb54006b53dc751edccb8c0ba668fe1311ee7592c232".to_string()];
    let update_msg = ExecuteMsg::UpdateConfig {
        owner: None,
        auction_contract_address: None,
        terra_merkle_roots: None,
        evm_merkle_roots: Some(evm_merkle_roots.clone()),
        from_timestamp: None,
        to_timestamp: None,
    };

    // Update Config :: should be a success
    app.execute_contract(
        Addr::unchecked(init_msg.owner.clone().unwrap()),
        airdrop_instance.clone(),
        &update_msg,
        &[],
    )
    .unwrap();

    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&airdrop_instance, &QueryMsg::Config {})
        .unwrap();

    // Check config and make sure all fields are updated
    assert_eq!(init_msg.owner.clone().unwrap(), resp.owner);
    assert_eq!(evm_merkle_roots, resp.evm_merkle_roots);
    assert_eq!(
        init_msg.terra_merkle_roots.unwrap(),
        resp.terra_merkle_roots
    );
    assert_eq!(init_msg.from_timestamp.unwrap(), resp.from_timestamp);
    assert_eq!(init_msg.to_timestamp, resp.to_timestamp);

    // Claim period has not started yet
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000)
    });

    let user_info_evm_address = "2c21b6fa9f82892d9853d8ee2351dc3c3e8e176d";
    let user_info_claim_amount = 50000000;
    let user_info_signed_msg_hash =
        "91f879f53729f18888d74aa10ea7737d629e36a1675bce35e1fb1be9065501df";
    let user_info_signature = "ca6c32751cf2b46429b98a1649d5b115c8836328427080335d6c401ed8ac3a9030cc13f4793d05cc68162f1231122eba6ea9d1dda1c13b7e327efbaf0c024d7d";

    let claim_msg = ExecuteMsg::ClaimByEvmUser {
        eth_address: user_info_evm_address.to_string(),
        claim_amount: Uint128::from(user_info_claim_amount as u64),
        merkle_proof: vec![
            "0a3419fc5fa4cb0ecb878dc3aaf01fa00782e5d79b02fbb4097dc8df8f191c60".to_string(),
            "45cc757ac5eda8bcd1a45a7bd2cb23f4af5147683f120fa287b99617834b83aa".to_string(),
        ],
        root_index: 0,
        signature: user_info_signature.to_string(),
        signed_msg_hash: user_info_signed_msg_hash.to_string(),
    };
    let claim_msg_wrong_amount = ExecuteMsg::ClaimByEvmUser {
        eth_address: user_info_evm_address.to_string(),
        claim_amount: Uint128::from(150000000 as u64),
        merkle_proof: vec![
            "0a3419fc5fa4cb0ecb878dc3aaf01fa00782e5d79b02fbb4097dc8df8f191c60".to_string(),
            "45cc757ac5eda8bcd1a45a7bd2cb23f4af5147683f120fa287b99617834b83aa".to_string(),
        ],
        root_index: 0,
        signature: user_info_signature.to_string(),
        signed_msg_hash: user_info_signed_msg_hash.to_string(),
    };
    let claim_msg_incorrect_proof = ExecuteMsg::ClaimByEvmUser {
        eth_address: user_info_evm_address.to_string(),
        claim_amount: Uint128::from(user_info_claim_amount as u64),
        merkle_proof: vec![
            "0b3419fc5fa4cb0ecb878dc3aaf01fa00782e5d79b02fbb4097dc8df8f191c60".to_string(),
            "45cc757ac5eda8bcd1a45a7bd2cb23f4af5147683f120fa287b99617834b83aa".to_string(),
        ],
        root_index: 0,
        signature: user_info_signature.to_string(),
        signed_msg_hash: user_info_signed_msg_hash.to_string(),
    };
    let claim_msg_incorrect_msg_hash = ExecuteMsg::ClaimByEvmUser {
        eth_address: user_info_evm_address.to_string(),
        claim_amount: Uint128::from(user_info_claim_amount as u64),
        merkle_proof: vec![
            "0a3419fc5fa4cb0ecb878dc3aaf01fa00782e5d79b02fbb4097dc8df8f191c60".to_string(),
            "45cc757ac5eda8bcd1a45a7bd2cb23f4af5147683f120fa287b99617834b83aa".to_string(),
        ],
        root_index: 0,
        signature: user_info_signature.to_string(),
        signed_msg_hash: "11f879f53729f18888d74aa10ea7737d629e36a1675bce35e1fb1be9065501df"
            .to_string(),
    };
    let claim_msg_incorrect_signature = ExecuteMsg::ClaimByEvmUser {
                                            eth_address : user_info_evm_address.to_string() ,
                                            claim_amount : Uint128::from(user_info_claim_amount as u64),
                                            merkle_proof : vec!["0a3419fc5fa4cb0ecb878dc3aaf01fa00782e5d79b02fbb4097dc8df8f191c60".to_string(),
                                                                "45cc757ac5eda8bcd1a45a7bd2cb23f4af5147683f120fa287b99617834b83aa".to_string()],
                                            root_index : 0,
                                            signature : user_info_signature.to_string(),
                                            signed_msg_hash : "ca7c32751cf2b46429b98a1649d5b115c8836328427080335d6c401ed8ac3a9030cc13f4793d05cc68162f1231122eba6ea9d1dda1c13b7e327efbaf0c024d7d1b".to_string()
                                        };

    // ################################
    //  Claims not allowed. MARS Rewards will Not be transferred to the user
    // ################################

    // **** "Claim not allowed" Error should be returned ****
    let mut claim_f = app
        .execute_contract(
            Addr::unchecked("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string()),
            airdrop_instance.clone(),
            &claim_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(claim_f.to_string(), "Generic error: Claim not allowed");

    // Update Block to test successful claim
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_05)
    });

    // **** "Incorrect Merkle Root Index" Error should be returned ****
    claim_f = app
        .execute_contract(
            Addr::unchecked("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string()),
            airdrop_instance.clone(),
            &ExecuteMsg::ClaimByEvmUser {
                eth_address: user_info_evm_address.to_string(),
                claim_amount: Uint128::from(user_info_claim_amount as u64),
                merkle_proof: vec![
                    "0a3419fc5fa4cb0ecb878dc3aaf01fa00782e5d79b02fbb4097dc8df8f191c60".to_string(),
                    "45cc757ac5eda8bcd1a45a7bd2cb23f4af5147683f120fa287b99617834b83aa".to_string(),
                ],
                root_index: 5,
                signature: user_info_signature.to_string(),
                signed_msg_hash: user_info_signed_msg_hash.to_string(),
            },
            &[],
        )
        .unwrap_err();

    assert_eq!(
        claim_f.to_string(),
        "Generic error: Incorrect Merkle Root Index"
    );

    // **** "Incorrect Merkle Proof" Error should be returned ****
    claim_f = app
        .execute_contract(
            Addr::unchecked("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string()),
            airdrop_instance.clone(),
            &claim_msg_incorrect_proof,
            &[],
        )
        .unwrap_err();

    assert_eq!(claim_f.to_string(), "Generic error: Incorrect Merkle Proof");

    // **** "Incorrect Merkle Proof" Error should be returned ****
    claim_f = app
        .execute_contract(
            Addr::unchecked("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string()),
            airdrop_instance.clone(),
            &claim_msg_wrong_amount,
            &[],
        )
        .unwrap_err();

    assert_eq!(claim_f.to_string(), "Generic error: Incorrect Merkle Proof");

    // **** "Invalid Signature" Error should be returned ****
    claim_f = app
        .execute_contract(
            Addr::unchecked("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string()),
            airdrop_instance.clone(),
            &claim_msg_incorrect_msg_hash,
            &[],
        )
        .unwrap_err();

    assert_eq!(claim_f.to_string(), "Generic error: Invalid Signature");

    // **** "Invalid Signature" Error should be returned ****
    claim_f = app
        .execute_contract(
            Addr::unchecked("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string()),
            airdrop_instance.clone(),
            &claim_msg_incorrect_signature,
            &[],
        )
        .unwrap_err();

    assert_eq!(claim_f.to_string(), "Generic error: Invalid Signature");

    // **** User should successfully claim the Airdrop ****

    // Check :: User hasn't yet claimed the airdrop
    let resp: ClaimResponse = app
        .wrap()
        .query_wasm_smart(
            &airdrop_instance,
            &QueryMsg::HasUserClaimed {
                address: "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string(),
            },
        )
        .unwrap();
    assert_eq!(false, resp.is_claimed);

    // Should be a success
    let success_ = app
        .execute_contract(
            Addr::unchecked("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string()),
            airdrop_instance.clone(),
            &claim_msg,
            &[],
        )
        .unwrap();

    assert_eq!(
        success_.events[1].attributes[1],
        attr("action", "Airdrop::ExecuteMsg::ClaimByEvmUser")
    );
    assert_eq!(
        success_.events[1].attributes[2],
        attr("claimer", "2c21b6fa9f82892d9853d8ee2351dc3c3e8e176d")
    );
    assert_eq!(
        success_.events[1].attributes[3],
        attr("recipient", "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp")
    );
    assert_eq!(
        success_.events[1].attributes[4],
        attr("airdrop", "50000000")
    );

    // Check :: User successfully claimed the airdrop
    let claim_query_resp: ClaimResponse = app
        .wrap()
        .query_wasm_smart(
            &airdrop_instance,
            &QueryMsg::HasUserClaimed {
                address: "2c21b6fa9f82892d9853d8ee2351dc3c3e8e176d".to_string(),
            },
        )
        .unwrap();
    assert_eq!(true, claim_query_resp.is_claimed);

    // Check :: User state
    let user_info_query_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &airdrop_instance,
            &QueryMsg::UserInfo {
                address: "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(50000000u64),
        user_info_query_resp.airdrop_amount
    );
    assert_eq!(Uint128::from(0u64), user_info_query_resp.delegated_amount);
    assert_eq!(false, user_info_query_resp.tokens_withdrawn);

    // Check :: Contract state
    let state_query_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&airdrop_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(
        Uint128::from(100_000_000_000u64),
        state_query_resp.total_airdrop_size
    );
    assert_eq!(Uint128::from(0u64), state_query_resp.total_delegated_amount);
    assert_eq!(
        Uint128::from(99950000000u64),
        state_query_resp.unclaimed_tokens
    );

    // **** "Already claimed" Error should be returned ****

    claim_f = app
        .execute_contract(
            Addr::unchecked("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string()),
            airdrop_instance.clone(),
            &claim_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(claim_f.to_string(), "Generic error: Already claimed");
}

#[cfg(test)]
#[test]
fn test_claim_by_evm_user_claims_enabled() {
    let mut app = mock_app();
    let (airdrop_instance, mars_instance, init_msg) = init_contracts(&mut app);

    // mint MARS for to Airdrop Contract
    mint_some_mars(
        &mut app,
        Addr::unchecked(init_msg.owner.clone().unwrap()),
        mars_instance.clone(),
        Uint128::new(100_000_000_000),
        airdrop_instance.clone().to_string(),
    );

    // Check Airdrop Contract balance
    let bal_resp: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &mars_instance,
            &cw20::Cw20QueryMsg::Balance {
                address: airdrop_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(100_000_000_000u64), bal_resp.balance);

    let evm_merkle_roots =
        vec!["1680ce46cb2c916f103afb54006b53dc751edccb8c0ba668fe1311ee7592c232".to_string()];
    let update_msg = ExecuteMsg::UpdateConfig {
        owner: None,
        auction_contract_address: None,
        terra_merkle_roots: None,
        evm_merkle_roots: Some(evm_merkle_roots.clone()),
        from_timestamp: None,
        to_timestamp: None,
    };

    // Update Config :: should be a success
    app.execute_contract(
        Addr::unchecked(init_msg.owner.clone().unwrap()),
        airdrop_instance.clone(),
        &update_msg,
        &[],
    )
    .unwrap();

    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&airdrop_instance, &QueryMsg::Config {})
        .unwrap();

    // Check config and make sure all fields are updated
    assert_eq!(init_msg.owner.clone().unwrap(), resp.owner);
    assert_eq!(evm_merkle_roots, resp.evm_merkle_roots);
    assert_eq!(
        init_msg.terra_merkle_roots.unwrap(),
        resp.terra_merkle_roots
    );
    assert_eq!(init_msg.from_timestamp.unwrap(), resp.from_timestamp);
    assert_eq!(init_msg.to_timestamp, resp.to_timestamp);

    let user_info_evm_address = "2c21b6fa9f82892d9853d8ee2351dc3c3e8e176d";
    let user_info_claim_amount = 50000000;
    let user_info_signed_msg_hash =
        "91f879f53729f18888d74aa10ea7737d629e36a1675bce35e1fb1be9065501df";
    let user_info_signature = "ca6c32751cf2b46429b98a1649d5b115c8836328427080335d6c401ed8ac3a9030cc13f4793d05cc68162f1231122eba6ea9d1dda1c13b7e327efbaf0c024d7d";

    let claim_msg = ExecuteMsg::ClaimByEvmUser {
        eth_address: user_info_evm_address.to_string(),
        claim_amount: Uint128::from(user_info_claim_amount as u64),
        merkle_proof: vec![
            "0a3419fc5fa4cb0ecb878dc3aaf01fa00782e5d79b02fbb4097dc8df8f191c60".to_string(),
            "45cc757ac5eda8bcd1a45a7bd2cb23f4af5147683f120fa287b99617834b83aa".to_string(),
        ],
        root_index: 0,
        signature: user_info_signature.to_string(),
        signed_msg_hash: user_info_signed_msg_hash.to_string(),
    };

    // Update Block to test successful claim
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_05)
    });

    // Enable MARS Withdrawals
    enable_claims(
        &mut app,
        Addr::unchecked(airdrop_instance.clone()),
        Addr::unchecked(init_msg.auction_contract_address),
    );

    // ################################
    //  Claims allowed. MARS Rewards will be transferred to the user
    // ################################

    // **** User should successfully claim the Airdrop ****

    // Check :: User hasn't yet claimed the airdrop
    let resp: ClaimResponse = app
        .wrap()
        .query_wasm_smart(
            &airdrop_instance,
            &QueryMsg::HasUserClaimed {
                address: "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string(),
            },
        )
        .unwrap();
    assert_eq!(false, resp.is_claimed);

    // Should be a success
    let success_ = app
        .execute_contract(
            Addr::unchecked("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string()),
            airdrop_instance.clone(),
            &claim_msg,
            &[],
        )
        .unwrap();

    assert_eq!(
        success_.events[1].attributes[1],
        attr("action", "Airdrop::ExecuteMsg::ClaimByEvmUser")
    );
    assert_eq!(
        success_.events[1].attributes[2],
        attr("claimer", "2c21b6fa9f82892d9853d8ee2351dc3c3e8e176d")
    );
    assert_eq!(
        success_.events[1].attributes[3],
        attr("recipient", "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp")
    );
    assert_eq!(
        success_.events[1].attributes[4],
        attr("airdrop", "50000000")
    );

    // Check :: User successfully claimed the airdrop
    let claim_query_resp: ClaimResponse = app
        .wrap()
        .query_wasm_smart(
            &airdrop_instance,
            &QueryMsg::HasUserClaimed {
                address: "2c21b6fa9f82892d9853d8ee2351dc3c3e8e176d".to_string(),
            },
        )
        .unwrap();
    assert_eq!(true, claim_query_resp.is_claimed);

    // Check :: User state
    let user_info_query_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &airdrop_instance,
            &QueryMsg::UserInfo {
                address: "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(50000000u64),
        user_info_query_resp.airdrop_amount
    );
    assert_eq!(Uint128::from(0u64), user_info_query_resp.delegated_amount);
    assert_eq!(true, user_info_query_resp.tokens_withdrawn);

    // Check :: Contract state
    let state_query_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&airdrop_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(
        Uint128::from(100_000_000_000u64),
        state_query_resp.total_airdrop_size
    );
    assert_eq!(Uint128::from(0u64), state_query_resp.total_delegated_amount);
    assert_eq!(
        Uint128::from(99950000000u64),
        state_query_resp.unclaimed_tokens
    );

    // Check user MARS balance
    let bal_resp: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &mars_instance,
            &cw20::Cw20QueryMsg::Balance {
                address: "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(50000000u64), bal_resp.balance);

    // **** "Already claimed" Error should be returned ****

    let claim_f = app
        .execute_contract(
            Addr::unchecked("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string()),
            airdrop_instance.clone(),
            &claim_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(claim_f.to_string(), "Generic error: Already claimed");
}

#[cfg(test)]
#[test]
fn test_enable_claims() {
    let mut app = mock_app();
    let (airdrop_instance, _, init_msg) = init_contracts(&mut app);

    let msg = ExecuteMsg::EnableClaims {};

    // ###### Should give "Unauthorized" Error ######

    let mut resp_f = app
        .execute_contract(
            Addr::unchecked("not_bootstrap_auction_contract".to_string()),
            airdrop_instance.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(resp_f.to_string(), "Generic error: Unauthorized");

    // ###### Should successfully enable claims ######

    app.execute_contract(
        Addr::unchecked(init_msg.auction_contract_address.clone()),
        airdrop_instance.clone(),
        &msg,
        &[],
    )
    .unwrap();

    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&airdrop_instance, &QueryMsg::Config {})
        .unwrap();
    assert_eq!(true, resp.are_claims_allowed);

    // ###### Should give "Claims already enabled" Error ######

    resp_f = app
        .execute_contract(
            Addr::unchecked(init_msg.auction_contract_address.clone()),
            airdrop_instance.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(resp_f.to_string(), "Generic error: Claims already enabled");
}

#[cfg(test)]
#[test]
fn test_withdraw_airdrop_rewards() {
    let mut app = mock_app();
    let (airdrop_instance, mars_instance, init_msg) = init_contracts(&mut app);

    // mint MARS for to Airdrop Contract
    mint_some_mars(
        &mut app,
        Addr::unchecked(init_msg.owner.clone().unwrap()),
        mars_instance.clone(),
        Uint128::new(100_000_000_000),
        airdrop_instance.clone().to_string(),
    );

    // Check Airdrop Contract balance
    let bal_resp: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &mars_instance,
            &cw20::Cw20QueryMsg::Balance {
                address: airdrop_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(100_000_000_000u64), bal_resp.balance);

    let terra_merkle_roots =
        vec!["cdcdfad1c342f5f55a2639dcae7321a64cd000807fa24c2c4ddaa944fd52d34e".to_string()];
    let evm_merkle_roots =
        vec!["1680ce46cb2c916f103afb54006b53dc751edccb8c0ba668fe1311ee7592c232".to_string()];

    let update_msg = ExecuteMsg::UpdateConfig {
        owner: None,
        auction_contract_address: None,
        terra_merkle_roots: Some(terra_merkle_roots.clone()),
        evm_merkle_roots: Some(evm_merkle_roots.clone()),
        from_timestamp: None,
        to_timestamp: None,
    };

    // Update Config :: should be a success
    app.execute_contract(
        Addr::unchecked(init_msg.owner.clone().unwrap()),
        airdrop_instance.clone(),
        &update_msg,
        &[],
    )
    .unwrap();

    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&airdrop_instance, &QueryMsg::Config {})
        .unwrap();

    // Check config and make sure all fields are updated
    assert_eq!(init_msg.owner.clone().unwrap(), resp.owner);
    assert_eq!(terra_merkle_roots, resp.terra_merkle_roots);
    assert_eq!(evm_merkle_roots, resp.evm_merkle_roots);
    assert_eq!(init_msg.from_timestamp.unwrap(), resp.from_timestamp);
    assert_eq!(init_msg.to_timestamp, resp.to_timestamp);

    // Update Block to test successful claim
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_05)
    });

    // ################################
    // TERRA USER :: Claims not allowed. MARS Rewards will Not be transferred to the user
    // ################################

    let claim_msg = ExecuteMsg::ClaimByTerraUser {
        claim_amount: Uint128::from(250000000 as u64),
        merkle_proof: vec![
            "7719b79a65e5aa0bbfd144cf5373138402ab1c374d9049e490b5b61c23d90065".to_string(),
            "60368f2058e0fb961a7721a241f9b973c3dd6c57e10a627071cd81abca6aa490".to_string(),
        ],
        root_index: 0,
    };

    // **** User should successfully claim the Airdrop ****

    // Check :: User hasn't yet claimed the airdrop
    let resp: ClaimResponse = app
        .wrap()
        .query_wasm_smart(
            &airdrop_instance,
            &QueryMsg::HasUserClaimed {
                address: "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string(),
            },
        )
        .unwrap();
    assert_eq!(false, resp.is_claimed);

    // Should be a success
    let success_ = app
        .execute_contract(
            Addr::unchecked("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string()),
            airdrop_instance.clone(),
            &claim_msg,
            &[],
        )
        .unwrap();

    assert_eq!(
        success_.events[1].attributes[1],
        attr("action", "Airdrop::ExecuteMsg::ClaimByTerraUser")
    );
    assert_eq!(
        success_.events[1].attributes[2],
        attr("claimer", "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp")
    );
    assert_eq!(
        success_.events[1].attributes[3],
        attr("airdrop", "250000000")
    );

    // Check :: Terra User successfully claimed the airdrop
    let claim_query_resp: ClaimResponse = app
        .wrap()
        .query_wasm_smart(
            &airdrop_instance,
            &QueryMsg::HasUserClaimed {
                address: "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string(),
            },
        )
        .unwrap();
    assert_eq!(true, claim_query_resp.is_claimed);

    // Check :: User state
    let user_info_query_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &airdrop_instance,
            &QueryMsg::UserInfo {
                address: "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(250000000u64),
        user_info_query_resp.airdrop_amount
    );
    assert_eq!(Uint128::from(0u64), user_info_query_resp.delegated_amount);
    assert_eq!(false, user_info_query_resp.tokens_withdrawn);

    // Check :: Contract state
    let state_query_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&airdrop_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(
        Uint128::from(100_000_000_000u64),
        state_query_resp.total_airdrop_size
    );
    assert_eq!(Uint128::from(0u64), state_query_resp.total_delegated_amount);
    assert_eq!(
        Uint128::from(99750000000u64),
        state_query_resp.unclaimed_tokens
    );

    // **** "Already claimed" Error should be returned ****

    let claim_f = app
        .execute_contract(
            Addr::unchecked("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string()),
            airdrop_instance.clone(),
            &claim_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(claim_f.to_string(), "Generic error: Already claimed");

    // ################################
    // EVM USER :: Claims not allowed. MARS Rewards will Not be transferred to the user
    // ################################

    let user_info_evm_address = "2c21b6fa9f82892d9853d8ee2351dc3c3e8e176d";
    let user_info_claim_amount = 50000000;
    let user_info_signed_msg_hash =
        "91f879f53729f18888d74aa10ea7737d629e36a1675bce35e1fb1be9065501df";
    let user_info_signature = "ca6c32751cf2b46429b98a1649d5b115c8836328427080335d6c401ed8ac3a9030cc13f4793d05cc68162f1231122eba6ea9d1dda1c13b7e327efbaf0c024d7d";

    let claim_msg = ExecuteMsg::ClaimByEvmUser {
        eth_address: user_info_evm_address.to_string(),
        claim_amount: Uint128::from(user_info_claim_amount as u64),
        merkle_proof: vec![
            "0a3419fc5fa4cb0ecb878dc3aaf01fa00782e5d79b02fbb4097dc8df8f191c60".to_string(),
            "45cc757ac5eda8bcd1a45a7bd2cb23f4af5147683f120fa287b99617834b83aa".to_string(),
        ],
        root_index: 0,
        signature: user_info_signature.to_string(),
        signed_msg_hash: user_info_signed_msg_hash.to_string(),
    };

    // **** Evm User should successfully claim the Airdrop ****

    // Check :: Evm User hasn't yet claimed the airdrop
    let resp: ClaimResponse = app
        .wrap()
        .query_wasm_smart(
            &airdrop_instance,
            &QueryMsg::HasUserClaimed {
                address: "2c21b6fa9f82892d9853d8ee2351dc3c3e8e176d".to_string(),
            },
        )
        .unwrap();
    assert_eq!(false, resp.is_claimed);

    // Should be a success
    let success_ = app
        .execute_contract(
            Addr::unchecked("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string()),
            airdrop_instance.clone(),
            &claim_msg,
            &[],
        )
        .unwrap();

    assert_eq!(
        success_.events[1].attributes[1],
        attr("action", "Airdrop::ExecuteMsg::ClaimByEvmUser")
    );
    assert_eq!(
        success_.events[1].attributes[2],
        attr("claimer", "2c21b6fa9f82892d9853d8ee2351dc3c3e8e176d")
    );
    assert_eq!(
        success_.events[1].attributes[3],
        attr("recipient", "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp")
    );
    assert_eq!(
        success_.events[1].attributes[4],
        attr("airdrop", "50000000")
    );

    // Check :: Evm User successfully claimed the airdrop
    let claim_query_resp: ClaimResponse = app
        .wrap()
        .query_wasm_smart(
            &airdrop_instance,
            &QueryMsg::HasUserClaimed {
                address: "2c21b6fa9f82892d9853d8ee2351dc3c3e8e176d".to_string(),
            },
        )
        .unwrap();
    assert_eq!(true, claim_query_resp.is_claimed);

    // Check :: User state
    let user_info_query_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &airdrop_instance,
            &QueryMsg::UserInfo {
                address: "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(300000000u64),
        user_info_query_resp.airdrop_amount
    );
    assert_eq!(Uint128::from(0u64), user_info_query_resp.delegated_amount);
    assert_eq!(false, user_info_query_resp.tokens_withdrawn);

    // Check :: Contract state
    let state_query_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&airdrop_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(
        Uint128::from(100_000_000_000u64),
        state_query_resp.total_airdrop_size
    );
    assert_eq!(Uint128::from(0u64), state_query_resp.total_delegated_amount);
    assert_eq!(
        Uint128::from(99700000000u64),
        state_query_resp.unclaimed_tokens
    );

    // #################
    // ENABLE CLAIMS ::
    // #################

    // Enable MARS Withdrawals
    enable_claims(
        &mut app,
        Addr::unchecked(airdrop_instance.clone()),
        Addr::unchecked(init_msg.auction_contract_address),
    );

    // Should be a success
    app.execute_contract(
        Addr::unchecked("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string()),
        airdrop_instance.clone(),
        &ExecuteMsg::WithdrawAirdropReward {},
        &[],
    )
    .unwrap();

    // Check :: User state
    let user_info_query_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &airdrop_instance,
            &QueryMsg::UserInfo {
                address: "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(300000000u64),
        user_info_query_resp.airdrop_amount
    );
    assert_eq!(Uint128::from(0u64), user_info_query_resp.delegated_amount);
    assert_eq!(true, user_info_query_resp.tokens_withdrawn);
}

#[cfg(test)]
#[test]
fn test_delegate_mars_to_bootstrap_auction() {
    let mut app = mock_app();
    let (airdrop_instance, mars_instance, init_msg) = init_contracts(&mut app);

    // mint MARS for to Airdrop Contract
    mint_some_mars(
        &mut app,
        Addr::unchecked(init_msg.owner.clone().unwrap()),
        mars_instance.clone(),
        Uint128::new(100_000_000_000),
        airdrop_instance.clone().to_string(),
    );

    // Initialize Bootstrap Auction contract
    let auction_contract = Box::new(ContractWrapper::new(
        mars_auction::contract::execute,
        mars_auction::contract::instantiate,
        mars_auction::contract::query,
    ));
    let auction_contract_code_id = app.store_code(auction_contract);
    let auction_init_msg = mars_periphery::auction::InstantiateMsg {
        owner: init_msg.owner.clone().unwrap(),
        mars_token_address: mars_instance.clone().to_string(),
        airdrop_contract_address: airdrop_instance.clone().to_string(),
        lockdrop_contract_address: "lockdrop_contract_address".to_string(),
        astroport_lp_pool: None,
        lp_token_address: None,
        generator_contract: None,
        mars_rewards: Uint256::from(10000000000000u64),
        mars_vesting_duration: 2592000u64,
        lp_tokens_vesting_duration: 2592000u64,
        init_timestamp: 100000u64,
        deposit_window: 2592000u64,
        withdrawal_window: 1592000u64,
    };

    let auction_contract_instance = app
        .instantiate_contract(
            auction_contract_code_id,
            Addr::unchecked(init_msg.owner.clone().unwrap()),
            &auction_init_msg,
            &[],
            String::from("MARS"),
            None,
        )
        .unwrap();

    let terra_merkle_roots =
        vec!["cdcdfad1c342f5f55a2639dcae7321a64cd000807fa24c2c4ddaa944fd52d34e".to_string()];

    let update_msg = ExecuteMsg::UpdateConfig {
        owner: None,
        auction_contract_address: Some(auction_contract_instance.to_string()),
        terra_merkle_roots: Some(terra_merkle_roots.clone()),
        evm_merkle_roots: None,
        from_timestamp: None,
        to_timestamp: None,
    };

    // Update Config :: should be a success
    app.execute_contract(
        Addr::unchecked(init_msg.owner.clone().unwrap()),
        airdrop_instance.clone(),
        &update_msg,
        &[],
    )
    .unwrap();

    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&airdrop_instance, &QueryMsg::Config {})
        .unwrap();

    // Check config and make sure all fields are updated
    assert_eq!(init_msg.owner.clone().unwrap(), resp.owner);
    assert_eq!(terra_merkle_roots, resp.terra_merkle_roots);
    assert_eq!(init_msg.evm_merkle_roots.unwrap(), resp.evm_merkle_roots);
    assert_eq!(init_msg.from_timestamp.unwrap(), resp.from_timestamp);
    assert_eq!(init_msg.to_timestamp, resp.to_timestamp);

    // Update Block to test successful claim
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_05)
    });

    // ################################
    // TERRA USER :: Claims not allowed. MARS Rewards will Not be transferred to the user
    // ################################

    let claim_msg = ExecuteMsg::ClaimByTerraUser {
        claim_amount: Uint128::from(250000000 as u64),
        merkle_proof: vec![
            "7719b79a65e5aa0bbfd144cf5373138402ab1c374d9049e490b5b61c23d90065".to_string(),
            "60368f2058e0fb961a7721a241f9b973c3dd6c57e10a627071cd81abca6aa490".to_string(),
        ],
        root_index: 0,
    };

    // **** User should successfully claim the Airdrop ****

    // Should be a success
    let success_ = app
        .execute_contract(
            Addr::unchecked("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string()),
            airdrop_instance.clone(),
            &claim_msg,
            &[],
        )
        .unwrap();

    assert_eq!(
        success_.events[1].attributes[1],
        attr("action", "Airdrop::ExecuteMsg::ClaimByTerraUser")
    );
    assert_eq!(
        success_.events[1].attributes[2],
        attr("claimer", "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp")
    );
    assert_eq!(
        success_.events[1].attributes[3],
        attr("airdrop", "250000000")
    );

    // **** "Total amount being delegated for boostrap auction cannot exceed your claimable airdrop balance" Error should be returned ****

    let claim_f = app
        .execute_contract(
            Addr::unchecked("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string()),
            airdrop_instance.clone(),
            &ExecuteMsg::DelegateMarsToBootstrapAuction {
                amount_to_delegate: Uint128::from(250000001u64),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(claim_f.to_string(), "Generic error: Total amount being delegated for boostrap auction cannot exceed your claimable airdrop balance");

    // **** Should successfully delegate MARS ****

    let delegation_res = app
        .execute_contract(
            Addr::unchecked("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string()),
            airdrop_instance.clone(),
            &ExecuteMsg::DelegateMarsToBootstrapAuction {
                amount_to_delegate: Uint128::from(250000000u64),
            },
            &[],
        )
        .unwrap();
    assert_eq!(
        delegation_res.events[1].attributes[1],
        attr(
            "action",
            "Airdrop::ExecuteMsg::DelegateMarsToBootstrapAuction"
        )
    );
    assert_eq!(
        delegation_res.events[1].attributes[2],
        attr("user", "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp")
    );
    assert_eq!(
        delegation_res.events[1].attributes[3],
        attr("amount_delegated", "250000000")
    );

    // Check :: Airdrop :: User state
    let user_info_query_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &airdrop_instance,
            &QueryMsg::UserInfo {
                address: "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(250000000u64),
        user_info_query_resp.airdrop_amount
    );
    assert_eq!(
        Uint128::from(250000000u64),
        user_info_query_resp.delegated_amount
    );
    assert_eq!(false, user_info_query_resp.tokens_withdrawn);

    // Check :: Airdrop :: Contract state
    let state_query_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&airdrop_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(
        Uint128::from(100_000_000_000u64),
        state_query_resp.total_airdrop_size
    );
    assert_eq!(
        Uint128::from(250000000u64),
        state_query_resp.total_delegated_amount
    );
    assert_eq!(
        Uint128::from(99750000000u64),
        state_query_resp.unclaimed_tokens
    );
}
