use std::str::FromStr;

use cosmwasm_std::testing::{mock_env, MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{attr, to_binary, Addr, Coin, Decimal, Timestamp, Uint128};
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg};
use mars_periphery::lockdrop::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, LockupDurationParams,
    LockupInfoResponse, QueryMsg, StateResponse, UpdateConfigMsg, UserInfoResponse,
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

// Instantiate MARS Token Contract
fn instantiate_mars_token(app: &mut App, owner: Addr) -> Addr {
    let mars_token_contract = Box::new(ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    ));

    let mars_token_code_id = app.store_code(mars_token_contract);

    let msg = cw20_base::msg::InstantiateMsg {
        name: String::from("MARS token"),
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
    mars_token_instance
}

// Instantiate Red Bank Contract { return (mars_address_provider_instance, red_bank_incentives_instance, red_bank_xmars_instance) }
fn instantiate_red_bank(app: &mut App, owner: Addr) -> (Addr, Addr, Addr, Addr, Addr) {
    // RED BANK :: Address provider
    let mars_address_provider = Box::new(ContractWrapper::new(
        mars_address_provider::contract::execute,
        mars_address_provider::contract::instantiate,
        mars_address_provider::contract::query,
    ));

    let mars_address_provider_code_id = app.store_code(mars_address_provider);

    let mars_address_provider_instance = app
        .instantiate_contract(
            mars_address_provider_code_id,
            owner.clone(),
            &mars_core::address_provider::msg::InstantiateMsg {
                owner: owner.clone().to_string(),
            },
            &[],
            String::from("address_provider"),
            None,
        )
        .unwrap();

    // RED BANK ::maToken contract
    let ma_token_contract = Box::new(ContractWrapper::new(
        mars_ma_token::contract::execute,
        mars_ma_token::contract::instantiate,
        mars_ma_token::contract::query,
    ));

    let ma_token_contract_code_id = app.store_code(ma_token_contract);

    // RED BANK :: Money market contract
    let money_market_contract = Box::new(ContractWrapper::new(
        mars_red_bank::contract::execute,
        mars_red_bank::contract::instantiate,
        mars_red_bank::contract::query,
    ));

    let money_market_contract_code_id = app.store_code(money_market_contract);

    let money_market_contract_instance = app
        .instantiate_contract(
            money_market_contract_code_id,
            owner.clone(),
            &mars_core::red_bank::msg::InstantiateMsg {
                config: mars_core::red_bank::msg::CreateOrUpdateConfig {
                    owner: Some(owner.clone().to_string()),
                    address_provider_address: Some(
                        mars_address_provider_instance.clone().to_string(),
                    ),
                    ma_token_code_id: Some(ma_token_contract_code_id),
                    close_factor: Some(mars_core::math::decimal::Decimal::from_ratio(1u64, 1u64)),
                },
            },
            &[],
            String::from("money_market_contract"),
            None,
        )
        .unwrap();

    // RED BANK :: Staking Contract
    let mars_staking = Box::new(ContractWrapper::new(
        mars_staking::contract::execute,
        mars_staking::contract::instantiate,
        mars_staking::contract::query,
    ));

    let mars_staking_code_id = app.store_code(mars_staking);

    let mars_staking_instance = app
        .instantiate_contract(
            mars_staking_code_id,
            owner.clone(),
            &mars_core::staking::msg::InstantiateMsg {
                config: mars_core::staking::msg::CreateOrUpdateConfig {
                    owner: Some(owner.clone().to_string()),
                    address_provider_address: Some(
                        mars_address_provider_instance.clone().to_string(),
                    ),
                    astroport_factory_address: Some("astroport_factory_address".to_string()),
                    astroport_max_spread: Some(Decimal::from_ratio(10u64, 10u64)),
                    cooldown_duration: Some(1u64),
                },
            },
            &[],
            String::from("mars_staking"),
            None,
        )
        .unwrap();

    // RED BANK :: Incentives Contract
    let red_bank_incentives = Box::new(ContractWrapper::new(
        mars_incentives::contract::execute,
        mars_incentives::contract::instantiate,
        mars_incentives::contract::query,
    ));

    let red_bank_incentives_code_id = app.store_code(red_bank_incentives);

    let red_bank_incentives_instance = app
        .instantiate_contract(
            red_bank_incentives_code_id,
            owner.clone(),
            &mars_core::incentives::msg::InstantiateMsg {
                owner: owner.clone().to_string(),
                address_provider_address: mars_address_provider_instance.clone().to_string(),
            },
            &[],
            String::from("red_bank_incentives"),
            None,
        )
        .unwrap();

    // RED BANK :: XMARS Contract
    let red_bank_xmars = Box::new(ContractWrapper::new(
        mars_xmars_token::contract::execute,
        mars_xmars_token::contract::instantiate,
        mars_xmars_token::contract::query,
    ));

    let red_bank_xmars_code_id = app.store_code(red_bank_xmars);

    let red_bank_xmars_instance = app
        .instantiate_contract(
            red_bank_xmars_code_id,
            owner.clone(),
            &mars_core::xmars_token::msg::InstantiateMsg {
                name: String::from("XMARS token"),
                symbol: String::from("XMARS"),
                decimals: 6,
                initial_balances: vec![],
                mint: Some(cw20::MinterResponse {
                    minter: mars_staking_instance.to_string(),
                    cap: None,
                }),
                marketing: None,
            },
            &[],
            String::from("xmars_token"),
            None,
        )
        .unwrap();

    // MARS token
    let mars_token_instance = instantiate_mars_token(app, owner.clone());

    // Update address_provider Config
    app.execute_contract(
        owner.clone(),
        mars_address_provider_instance.clone(),
        &mars_core::address_provider::msg::ExecuteMsg::UpdateConfig {
            config: mars_core::address_provider::msg::ConfigParams {
                owner: None,
                council_address: None,
                incentives_address: Some(red_bank_incentives_instance.to_string()),
                safety_fund_address: None,
                mars_token_address: Some(mars_token_instance.to_string()),
                oracle_address: Some("oracle".to_string()),
                protocol_admin_address: Some("protocol_admin_address".to_string()),
                protocol_rewards_collector_address: Some("protocol_rewards_collector".to_string()),
                red_bank_address: Some(money_market_contract_instance.to_string()),
                staking_address: Some(mars_staking_instance.to_string()),
                treasury_address: None,
                vesting_address: None,
                xmars_token_address: Some(red_bank_xmars_instance.to_string()),
            },
        },
        &[],
    )
    .unwrap();

    // Initialize UST Money market pool
    app.execute_contract(
        owner.clone(),
        money_market_contract_instance.clone(),
        &mars_core::red_bank::msg::ExecuteMsg::InitAsset {
            asset: mars_core::asset::Asset::Native {
                denom: "uusd".to_string(),
            },
            asset_symbol: Some("uusd".to_string()),
            asset_params: mars_core::red_bank::msg::InitOrUpdateAssetParams {
                initial_borrow_rate: Some(
                    mars_core::math::decimal::Decimal::from_str(&"0.2".to_string()).unwrap(),
                ),
                reserve_factor: Some(
                    mars_core::math::decimal::Decimal::from_str(&"0.2".to_string()).unwrap(),
                ),
                max_loan_to_value: Some(
                    mars_core::math::decimal::Decimal::from_str(&"0.75".to_string()).unwrap(),
                ),
                liquidation_threshold: Some(
                    mars_core::math::decimal::Decimal::from_str(&"0.85".to_string()).unwrap(),
                ),
                liquidation_bonus: Some(
                    mars_core::math::decimal::Decimal::from_str(&"0.1".to_string()).unwrap(),
                ),
                interest_rate_model_params: Some(
                    mars_core::red_bank::interest_rate_models::InterestRateModelParams::Dynamic(
                        mars_core::red_bank::interest_rate_models::DynamicInterestRateModelParams {
                            min_borrow_rate: mars_core::math::decimal::Decimal::from_str(
                                &"0.0".to_string(),
                            )
                            .unwrap(),
                            max_borrow_rate: mars_core::math::decimal::Decimal::from_str(
                                &"2.0".to_string(),
                            )
                            .unwrap(),
                            optimal_utilization_rate: mars_core::math::decimal::Decimal::from_str(
                                &"0.7".to_string(),
                            )
                            .unwrap(),
                            kp_1: mars_core::math::decimal::Decimal::from_str(&"0.02".to_string())
                                .unwrap(),
                            kp_2: mars_core::math::decimal::Decimal::from_str(&"0.05".to_string())
                                .unwrap(),
                            kp_augmentation_threshold: mars_core::math::decimal::Decimal::from_str(
                                &"0.15".to_string(),
                            )
                            .unwrap(),
                            update_threshold_txs: 5u32,
                            update_threshold_seconds: 600u64,
                        },
                    ),
                ),
                active: Some(true),
                deposit_enabled: Some(true),
                borrow_enabled: Some(true),
            },
        },
        &[],
    )
    .unwrap();

    return (
        mars_address_provider_instance,
        money_market_contract_instance,
        red_bank_incentives_instance,
        red_bank_xmars_instance,
        mars_token_instance,
    );
}

// Instantiate AUCTION Contract
fn instantiate_auction_contract(
    app: &mut App,
    owner: Addr,
    mars_token_instance: Addr,
    airdrop_instance: Addr,
    lockdrop_instance: Addr,
) -> (Addr, mars_periphery::auction::InstantiateMsg) {
    let auction_contract = Box::new(ContractWrapper::new(
        mars_auction::contract::execute,
        mars_auction::contract::instantiate,
        mars_auction::contract::query,
    ));

    let auction_code_id = app.store_code(auction_contract);

    let auction_instantiate_msg = mars_periphery::auction::InstantiateMsg {
        owner: owner.clone().to_string(),
        mars_token_address: mars_token_instance.clone().into_string(),
        astro_token_address: "astro_token".to_string(),
        airdrop_contract_address: airdrop_instance.to_string(),
        lockdrop_contract_address: lockdrop_instance.to_string(),
        generator_contract: "generator_contract".to_string(),
        mars_vesting_duration: 7776000u64,
        lp_tokens_vesting_duration: 7776000u64,
        init_timestamp: 1700001,
        ust_deposit_window: 10_000_00,
        mars_deposit_window: 10_000_00,
        withdrawal_window: 5_000_00,
    };

    // Init contract
    let auction_instance = app
        .instantiate_contract(
            auction_code_id,
            owner.clone(),
            &auction_instantiate_msg,
            &[],
            "auction",
            None,
        )
        .unwrap();

    app.execute_contract(
        owner.clone(),
        mars_token_instance.clone(),
        &Cw20ExecuteMsg::Send {
            amount: Uint128::from(1000000000000u64),
            contract: auction_instance.clone().to_string(),
            msg: to_binary(&Cw20HookMsg::IncreaseMarsIncentives {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    (auction_instance, auction_instantiate_msg)
}

// Instantiate LOCKDROP Contract
fn instantiate_lockdrop_contract(
    app: &mut App,
    owner: Addr,
    address_provider: Option<Addr>,
    ma_ust_token: Option<Addr>,
) -> (Addr, InstantiateMsg) {
    let lockdrop_contract = Box::new(ContractWrapper::new(
        mars_lockdrop::contract::execute,
        mars_lockdrop::contract::instantiate,
        mars_lockdrop::contract::query,
    ));

    let lockdrop_code_id = app.store_code(lockdrop_contract);

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10_000_00)
    });

    let mut lockdrop_instantiate_msg = mars_periphery::lockdrop::InstantiateMsg {
        owner: owner.clone().to_string(),
        address_provider: None,
        ma_ust_token: None,
        init_timestamp: 10_000_01,
        deposit_window: 5_000_00,
        withdrawal_window: 2_000_00,
        lockup_durations: vec![
            LockupDurationParams {
                duration: 3,
                boost: Uint128::from(1_u64),
            },
            LockupDurationParams {
                duration: 6,
                boost: Uint128::from(2_u64),
            },
            LockupDurationParams {
                duration: 9,
                boost: Uint128::from(3_u64),
            },
            LockupDurationParams {
                duration: 12,
                boost: Uint128::from(4_u64),
            },
            LockupDurationParams {
                duration: 15,
                boost: Uint128::from(5_u64),
            },
        ],
        seconds_per_duration_unit: 7 * 86400 as u64,
    };
    if address_provider.is_some() {
        lockdrop_instantiate_msg.address_provider = Some(address_provider.unwrap().to_string());
    }
    if ma_ust_token.is_some() {
        lockdrop_instantiate_msg.ma_ust_token = Some(ma_ust_token.unwrap().to_string());
    }

    // Init contract
    let lockdrop_instance = app
        .instantiate_contract(
            lockdrop_code_id,
            owner.clone(),
            &lockdrop_instantiate_msg,
            &[],
            "auction",
            None,
        )
        .unwrap();

    (lockdrop_instance, lockdrop_instantiate_msg)
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

// Initiates Lockdrop Contract with properly configured Config
// fn init_all_contracts(app: &mut App, owner: Addr) -> (Addr, Addr, Addr, Addr, InstantiateMsg) {
//     let mars_token_instance = instantiate_mars_token(app, owner.clone());

//     let (lp_staking_instance, staking_token_instance, lp_staking_instantiate_msg) =
//         instantiate_lp_staking_contract(app, owner.clone(), mars_token_instance.clone());

//     return (
//         mars_token_instance,
//         lp_staking_instance,
//         staking_token_instance,
//         lp_staking_instantiate_msg,
//     );
// }

#[test]
fn test_proper_initialization() {
    let mut app = mock_app();
    let owner = Addr::unchecked("contract_owner");

    let (lockdrop_instance, init_msg) = instantiate_lockdrop_contract(
        &mut app,
        owner,
        Some(Addr::unchecked("address_provider")),
        Some(Addr::unchecked("ma_ust_token")),
    );

    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&lockdrop_instance, &QueryMsg::Config {})
        .unwrap();

    // Check config
    assert_eq!(init_msg.owner, resp.owner);
    assert_eq!(
        Some(Addr::unchecked(init_msg.address_provider.unwrap())),
        resp.address_provider
    );
    assert_eq!(
        Some(Addr::unchecked(init_msg.ma_ust_token.unwrap())),
        resp.ma_ust_token
    );
    assert_eq!(None, resp.auction_contract_address);
    assert_eq!(init_msg.init_timestamp, resp.init_timestamp);
    assert_eq!(init_msg.deposit_window, resp.deposit_window);
    assert_eq!(init_msg.withdrawal_window, resp.withdrawal_window);
    assert_eq!(init_msg.lockup_durations, resp.lockup_durations);
    assert_eq!(
        init_msg.seconds_per_duration_unit,
        resp.seconds_per_duration_unit
    );

    // Check state
    let resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&lockdrop_instance, &QueryMsg::State {})
        .unwrap();

    assert_eq!(Uint128::zero(), resp.final_ust_locked);
    assert_eq!(Uint128::zero(), resp.final_maust_locked);
    assert_eq!(Uint128::zero(), resp.total_ust_locked);
    assert_eq!(Uint128::zero(), resp.total_maust_locked);
    assert_eq!(Uint128::zero(), resp.total_mars_delegated);
    assert_eq!(false, resp.are_claims_allowed);
    assert_eq!(Uint128::zero(), resp.total_deposits_weight);
    assert_eq!(Decimal::zero(), resp.xmars_rewards_index);
}

#[test]
fn update_config() {
    let mut app = mock_app();
    let owner = Addr::unchecked("contract_owner");

    let (lockdrop_instance, _) = instantiate_lockdrop_contract(
        &mut app,
        owner.clone(),
        Some(Addr::unchecked("address_provider")),
        Some(Addr::unchecked("ma_ust_token")),
    );

    let update_config = UpdateConfigMsg {
        owner: Some("new_owner".to_string()),
        address_provider: Some("new_address_provider".to_string()),
        ma_ust_token: Some("new_ma_ust_token".to_string()),
        auction_contract_address: Some("new_auction_contract".to_string()),
    };

    // ******* Error ::: Only owner can update *******

    let err = app
        .execute_contract(
            Addr::unchecked("wrong_owner"),
            lockdrop_instance.clone(),
            &ExecuteMsg::UpdateConfig {
                new_config: update_config.clone(),
            },
            &[],
        )
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        "Generic error: Only owner can update configuration"
    );

    // ******* Successfully update  *******

    app.execute_contract(
        owner,
        lockdrop_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: update_config,
        },
        &[],
    )
    .unwrap();

    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&lockdrop_instance, &QueryMsg::Config {})
        .unwrap();

    // Check config and make sure all fields are updated
    assert_eq!("new_owner".to_string(), resp.owner);
    assert_eq!(
        Addr::unchecked("new_address_provider".to_string()),
        resp.address_provider.unwrap()
    );
    assert_eq!(
        Addr::unchecked("new_ma_ust_token".to_string()),
        resp.ma_ust_token.unwrap()
    );
    assert_eq!(
        Addr::unchecked("new_auction_contract".to_string()),
        resp.auction_contract_address.unwrap()
    );
}

#[test]
fn test_deposit_ust() {
    let mut app = mock_app();
    let owner = Addr::unchecked("contract_owner");
    let (lockdrop_instance, _) = instantiate_lockdrop_contract(&mut app, owner.clone(), None, None);

    let (address_provider_instance, _, _, _, mars_token_instance) =
        instantiate_red_bank(&mut app, owner.clone());

    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: UpdateConfigMsg {
                owner: None,
                address_provider: Some(address_provider_instance.to_string()),
                ma_ust_token: None,
                auction_contract_address: None,
            },
        },
        &[],
    )
    .unwrap();

    mint_some_mars(
        &mut app,
        owner.clone(),
        mars_token_instance.clone(),
        Uint128::new(900_000_0000_000),
        owner.to_string(),
    );

    app.execute_contract(
        owner.clone(),
        mars_token_instance.clone(),
        &Cw20ExecuteMsg::Send {
            amount: Uint128::from(1000000000000u64),
            contract: lockdrop_instance.to_string(),
            msg: to_binary(&Cw20HookMsg::IncreaseMarsIncentives {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    let user1_address = Addr::unchecked("user1");
    let user2_address = Addr::unchecked("user2");

    // Set user balances
    app.init_bank_balance(
        &user1_address.clone(),
        vec![
            Coin {
                denom: "uusd".to_string(),
                amount: Uint128::new(20000000u128),
            },
            Coin {
                denom: "uluna".to_string(),
                amount: Uint128::new(20000000u128),
            },
        ],
    )
    .unwrap();
    app.init_bank_balance(
        &user2_address.clone(),
        vec![
            Coin {
                denom: "uusd".to_string(),
                amount: Uint128::new(5435435u128),
            },
            Coin {
                denom: "uluna".to_string(),
                amount: Uint128::new(20000000u128),
            },
        ],
    )
    .unwrap();

    // ***
    // *** Test :: Error "Deposit window closed" Reason :: Deposit attempt before deposit window is open ***
    // ***

    let err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::DepositUst { duration: 3u64 },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Deposit window closed");

    // ***
    // *** Test :: Error "Deposit window closed" Reason :: Deposit attempt after deposit window is closed ***
    // ***
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(15_000_03)
    });
    let err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::DepositUst { duration: 3u64 },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Deposit window closed");

    // ***
    // *** Test :: Error "Trying to deposit several coins" ***
    // ***
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10_000_03)
    });

    let err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::DepositUst { duration: 3u64 },
            &[
                Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::from(432423u128),
                },
                Coin {
                    denom: "uluna".to_string(),
                    amount: Uint128::from(432423u128),
                },
            ],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Trying to deposit several coins"
    );

    // ***
    // *** Test :: Error "Only UST among native tokens accepted" ***
    // ***

    let err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::DepositUst { duration: 3u64 },
            &[Coin {
                denom: "uluna".to_string(),
                amount: Uint128::from(432423u128),
            }],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Only UST among native tokens accepted"
    );

    // ***
    // *** Test :: Error "Amount must be greater than 0" ***
    // ***

    let err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::DepositUst { duration: 3u64 },
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(0u128),
            }],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Amount must be greater than 0"
    );

    // ***
    // *** Test :: Error "{} lockup duration not supported" ***
    // ***

    let err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::DepositUst { duration: 1u64 },
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(10u128),
            }],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Boost not found for duration 1"
    );

    let err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::DepositUst { duration: 52u64 },
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(10u128),
            }],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Boost not found for duration 52"
    );

    // ***
    // *** Test #1 :: Successfully deposit UST  ***
    // ***

    app.execute_contract(
        user1_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::DepositUst { duration: 3u64 },
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(10000u128),
        }],
    )
    .unwrap();

    // let's verify the Lockdrop
    let mut lockdrop_resp: LockupInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::LockupInfo {
                address: user1_address.clone().to_string(),
                duration: 3u64,
            },
        )
        .unwrap();

    let lockup_query_data = lockdrop_resp.lockup_info.unwrap();
    assert_eq!(3u64, lockup_query_data.duration);
    assert_eq!(Uint128::from(10000u64), lockup_query_data.ust_locked);
    assert_eq!(Uint128::zero(), lockup_query_data.maust_balance);
    assert_eq!(
        Uint128::from(1000000000000u64),
        lockup_query_data.lockdrop_reward
    );
    assert_eq!(3514401u64, lockup_query_data.unlock_timestamp);

    // let's verify the User
    let mut user_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::UserInfo {
                address: user1_address.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(10000u64), user_resp.total_ust_locked);
    assert_eq!(Uint128::zero(), user_resp.total_maust_share);
    assert_eq!(vec!["user13".to_string()], user_resp.lockup_position_ids);
    assert_eq!(
        Uint128::from(1000000000000u64),
        user_resp.total_mars_incentives
    );
    assert_eq!(Uint128::zero(), user_resp.delegated_mars_incentives);
    assert_eq!(false, user_resp.is_lockdrop_claimed);
    assert_eq!(Decimal::zero(), user_resp.reward_index);
    assert_eq!(Uint128::zero(), user_resp.total_xmars_claimed);
    assert_eq!(Uint128::zero(), user_resp.pending_xmars_to_claim);

    // let's verify the state
    let mut state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&lockdrop_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::zero(), state_resp.final_ust_locked);
    assert_eq!(Uint128::zero(), state_resp.final_maust_locked);
    assert_eq!(Uint128::from(10000u64), state_resp.total_ust_locked);
    assert_eq!(Uint128::zero(), state_resp.total_maust_locked);
    assert_eq!(Uint128::from(10000u64), state_resp.total_deposits_weight);

    // ***
    // *** Test #2 :: Successfully deposit UST  ***
    // ***

    app.execute_contract(
        user1_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::DepositUst { duration: 15u64 },
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(10000u128),
        }],
    )
    .unwrap();

    // let's verify the Lockdrop
    lockdrop_resp = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::LockupInfo {
                address: user1_address.clone().to_string(),
                duration: 15u64,
            },
        )
        .unwrap();

    let lockup_query_data = lockdrop_resp.lockup_info.unwrap();
    assert_eq!(15u64, lockup_query_data.duration);
    assert_eq!(Uint128::from(10000u64), lockup_query_data.ust_locked);
    assert_eq!(Uint128::zero(), lockup_query_data.maust_balance);
    assert_eq!(
        Uint128::from(833333333333u64),
        lockup_query_data.lockdrop_reward
    );
    assert_eq!(10772001u64, lockup_query_data.unlock_timestamp);

    // let's verify the User
    user_resp = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::UserInfo {
                address: user1_address.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(20000u64), user_resp.total_ust_locked);
    assert_eq!(Uint128::zero(), user_resp.total_maust_share);
    assert_eq!(
        vec!["user13".to_string(), "user115".to_string()],
        user_resp.lockup_position_ids
    );
    assert_eq!(
        Uint128::from(999999999999u64),
        user_resp.total_mars_incentives
    );

    // let's verify the state
    state_resp = app
        .wrap()
        .query_wasm_smart(&lockdrop_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::zero(), state_resp.final_ust_locked);
    assert_eq!(Uint128::zero(), state_resp.final_maust_locked);
    assert_eq!(Uint128::from(20000u64), state_resp.total_ust_locked);
    assert_eq!(Uint128::zero(), state_resp.total_maust_locked);
    assert_eq!(Uint128::from(60000u64), state_resp.total_deposits_weight);
}

#[test]
fn test_withdraw_ust() {
    let mut app = mock_app();
    let owner = Addr::unchecked("contract_owner");
    let (lockdrop_instance, _) = instantiate_lockdrop_contract(&mut app, owner.clone(), None, None);

    let (address_provider_instance, _, _, _, mars_token_instance) =
        instantiate_red_bank(&mut app, owner.clone());

    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: UpdateConfigMsg {
                owner: None,
                address_provider: Some(address_provider_instance.to_string()),
                ma_ust_token: None,
                auction_contract_address: None,
            },
        },
        &[],
    )
    .unwrap();

    mint_some_mars(
        &mut app,
        owner.clone(),
        mars_token_instance.clone(),
        Uint128::new(900_000_0000_000),
        owner.to_string(),
    );

    app.execute_contract(
        owner.clone(),
        mars_token_instance.clone(),
        &Cw20ExecuteMsg::Send {
            amount: Uint128::from(1000000000000u64),
            contract: lockdrop_instance.to_string(),
            msg: to_binary(&Cw20HookMsg::IncreaseMarsIncentives {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    let user1_address = Addr::unchecked("user1");

    // Set user balances
    app.init_bank_balance(
        &user1_address.clone(),
        vec![
            Coin {
                denom: "uusd".to_string(),
                amount: Uint128::new(20000000u128),
            },
            Coin {
                denom: "uluna".to_string(),
                amount: Uint128::new(20000000u128),
            },
        ],
    )
    .unwrap();

    // for successful deposit
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10_000_03)
    });

    // ######    SUCCESS :: UST Successfully deposited     ######
    app.execute_contract(
        user1_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::DepositUst { duration: 6u64 },
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(10000u128),
        }],
    )
    .unwrap();

    app.execute_contract(
        user1_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::DepositUst { duration: 15u64 },
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(10000u128),
        }],
    )
    .unwrap();

    // ######    SUCCESS :: UST Successfully withdrawn (when withdrawals allowed)     ######
    app.execute_contract(
        user1_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::WithdrawUst {
            duration: 6u64,
            amount: Uint128::from(100u128),
        },
        &[],
    )
    .unwrap();

    // Check state response
    let mut state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&lockdrop_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::from(19900u64), state_resp.total_ust_locked);

    // Check user response
    let mut user_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(19900u64), user_resp.total_ust_locked);

    // let's verify the Lockdrop
    let mut lockdrop_resp: LockupInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::LockupInfo {
                address: user1_address.clone().to_string(),
                duration: 6u64,
            },
        )
        .unwrap();

    let lockup_query_data = lockdrop_resp.lockup_info.unwrap();
    assert_eq!(6u64, lockup_query_data.duration);
    assert_eq!(Uint128::from(9900u64), lockup_query_data.ust_locked);

    // close deposit window. Max 50% withdrawals allowed now
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(15_000_01)
    });

    // ######    ERROR :: Amount exceeds maximum allowed withdrawal limit of {}   ######

    let mut err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::WithdrawUst {
                amount: Uint128::from(9000u64),
                duration: 6u64,
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Amount exceeds maximum allowed withdrawal limit of 4950 "
    );

    // ######    SUCCESS :: Withdraw 50% successfully   ######

    app.execute_contract(
        user1_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::WithdrawUst {
            amount: Uint128::from(4950u64),
            duration: 6u64,
        },
        &[],
    )
    .unwrap();
    // Check state response
    state_resp = app
        .wrap()
        .query_wasm_smart(&lockdrop_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::from(14950u64), state_resp.total_ust_locked);

    // Check user response
    user_resp = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(14950u64), user_resp.total_ust_locked);

    // let's verify the Lockdrop
    lockdrop_resp = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::LockupInfo {
                address: user1_address.clone().to_string(),
                duration: 6u64,
            },
        )
        .unwrap();

    let lockup_query_data = lockdrop_resp.lockup_info.unwrap();
    assert_eq!(6u64, lockup_query_data.duration);
    assert_eq!(Uint128::from(4950u64), lockup_query_data.ust_locked);

    // ######    ERROR :: Max 1 withdrawal allowed during current window   ######

    err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::WithdrawUst {
                amount: Uint128::from(10u64),
                duration: 6u64,
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Max 1 withdrawal allowed");

    // 50% of withdrawal window over. Max withdrawal % decreasing linearly now

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(16_050_01)
    });

    // ######    ERROR :: Amount exceeds maximum allowed withdrawal limit of {}   ######

    err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::WithdrawUst {
                amount: Uint128::from(7000u64),
                duration: 15u64,
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Amount exceeds maximum allowed withdrawal limit of 4750 "
    );

    // ######    SUCCESS :: Withdraw some UST successfully   ######

    app.execute_contract(
        user1_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::WithdrawUst {
            amount: Uint128::from(750u64),
            duration: 15u64,
        },
        &[],
    )
    .unwrap();

    // Check state response
    state_resp = app
        .wrap()
        .query_wasm_smart(&lockdrop_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::from(14200u64), state_resp.total_ust_locked);

    // Check user response
    user_resp = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(14200u64), user_resp.total_ust_locked);

    // let's verify the Lockdrop
    lockdrop_resp = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::LockupInfo {
                address: user1_address.clone().to_string(),
                duration: 15u64,
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(9250u64),
        lockdrop_resp.lockup_info.unwrap().ust_locked
    );

    // // ######    ERROR :: Max 1 withdrawal allowed during current window   ######

    err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::WithdrawUst {
                amount: Uint128::from(50u64),
                duration: 15u64,
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Max 1 withdrawal allowed");

    // finish withdraw period for deposit failure

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(17_000_41)
    });

    err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::WithdrawUst {
                amount: Uint128::from(50u64),
                duration: 15u64,
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Withdrawals not allowed");
}

#[test]
fn test_deposit_mars_to_auction() {
    let mut app = mock_app();
    let owner = Addr::unchecked("contract_owner");
    let (lockdrop_instance, _) = instantiate_lockdrop_contract(&mut app, owner.clone(), None, None);

    // ******* Initialize Address Provider & Auction  *******

    let (address_provider_instance, _, _, _, mars_token_instance) =
        instantiate_red_bank(&mut app, owner.clone());

    mint_some_mars(
        &mut app,
        owner.clone(),
        mars_token_instance.clone(),
        Uint128::new(900_000_0000_000),
        owner.to_string(),
    );

    let (auction_instance, _) = instantiate_auction_contract(
        &mut app,
        owner.clone(),
        mars_token_instance.clone(),
        Addr::unchecked("airdrop_instance"),
        lockdrop_instance.clone(),
    );

    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: UpdateConfigMsg {
                owner: None,
                address_provider: Some(address_provider_instance.to_string()),
                ma_ust_token: None,
                auction_contract_address: None,
            },
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        owner.clone(),
        mars_token_instance.clone(),
        &Cw20ExecuteMsg::Send {
            amount: Uint128::from(1000000000000u64),
            contract: lockdrop_instance.to_string(),
            msg: to_binary(&Cw20HookMsg::IncreaseMarsIncentives {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    let user1_address = Addr::unchecked("user1");

    // Set user balances
    app.init_bank_balance(
        &user1_address.clone(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(20000000u128),
        }],
    )
    .unwrap();

    // for successful deposit
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10_000_03)
    });

    // ######    SUCCESS :: UST Successfully deposited     ######
    app.execute_contract(
        user1_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::DepositUst { duration: 6u64 },
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(10000u128),
        }],
    )
    .unwrap();

    app.execute_contract(
        user1_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::DepositUst { duration: 15u64 },
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(10000u128),
        }],
    )
    .unwrap();

    // ######    ERROR :: Deposit / withdraw windows not closed yet   ######

    let mut err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::DepositMarsToAuction {
                amount: Uint128::from(9000u64),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Deposit / withdraw windows not closed yet"
    );

    // ######    ERROR :: Auction contract address not set   ######
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(17_000_03)
    });

    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: UpdateConfigMsg {
                owner: None,
                address_provider: Some(address_provider_instance.clone().to_string()),
                ma_ust_token: None,
                auction_contract_address: None,
            },
        },
        &[],
    )
    .unwrap();

    err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::DepositMarsToAuction {
                amount: Uint128::from(9000u64),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Auction contract address not set"
    );

    // ######    ERROR :: "No valid lockup positions   ######
    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: UpdateConfigMsg {
                owner: None,
                address_provider: None,
                ma_ust_token: None,
                auction_contract_address: Some(auction_instance.clone().to_string()),
            },
        },
        &[],
    )
    .unwrap();

    err = app
        .execute_contract(
            Addr::unchecked("not_user".to_string()),
            lockdrop_instance.clone(),
            &ExecuteMsg::DepositMarsToAuction {
                amount: Uint128::from(9000u64),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: No valid lockup positions");

    // ######    SUCCESSFULLY DELEGATE TO AUCTION   ######

    // Update Lockdrop Config :: Set auction contract address
    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: UpdateConfigMsg {
                owner: None,
                address_provider: None,
                ma_ust_token: None,
                auction_contract_address: Some(auction_instance.clone().to_string()),
            },
        },
        &[],
    )
    .unwrap();

    // Lockdrop's MARS balance (before delegation)
    let mars_balance_lockdrop_before: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &mars_token_instance,
            &Cw20QueryMsg::Balance {
                address: lockdrop_instance.clone().to_string(),
            },
        )
        .unwrap();

    // Auction's MARS balance (before delegation)
    let mars_balance_auction_before: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &mars_token_instance,
            &Cw20QueryMsg::Balance {
                address: auction_instance.clone().to_string(),
            },
        )
        .unwrap();

    // Delegate MARS to auction
    app.execute_contract(
        user1_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::DepositMarsToAuction {
            amount: Uint128::from(9000u64),
        },
        &[],
    )
    .unwrap();

    // Check state response
    let state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&lockdrop_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::from(9000u64), state_resp.total_mars_delegated);

    // Check user response
    let user_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(999999999999u128),
        user_resp.total_mars_incentives
    );
    assert_eq!(Uint128::from(9000u64), user_resp.delegated_mars_incentives);

    // Lockdrop's MARS balance (after delegation)
    let mars_balance_lockdrop_after: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &mars_token_instance,
            &Cw20QueryMsg::Balance {
                address: lockdrop_instance.clone().to_string(),
            },
        )
        .unwrap();

    // Auction's MARS balance (after delegation)
    let mars_balance_auction_after: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &mars_token_instance,
            &Cw20QueryMsg::Balance {
                address: auction_instance.clone().to_string(),
            },
        )
        .unwrap();

    // Check MARS tokens were transferred correctly from Lockdrop to Auction contract
    assert_eq!(
        mars_balance_lockdrop_before.balance - mars_balance_lockdrop_after.balance,
        mars_balance_auction_after.balance - mars_balance_auction_before.balance
    );
    assert_eq!(
        mars_balance_lockdrop_before.balance - mars_balance_lockdrop_after.balance,
        Uint128::from(9000u64)
    );
    assert_eq!(Uint128::from(9000u64), user_resp.delegated_mars_incentives);

    // ######    ERROR :: Amount cannot exceed user's unclaimed MARS balance   ######

    // Delegate MARS to auction
    err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::DepositMarsToAuction {
                amount: Uint128::from(900000_0000000000u64),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Amount cannot exceed user's unclaimed MARS balance. MARS to delegate = 9000000000000000, Max delegatable MARS = 999999990999 ");
}

#[test]
fn test_deposit_ust_in_red_bank() {
    let mut app = mock_app();
    let owner = Addr::unchecked("contract_owner");
    let (lockdrop_instance, _) = instantiate_lockdrop_contract(&mut app, owner.clone(), None, None);

    // ******* Initialize Address Provider & Auction  *******

    let (address_provider_instance, red_bank_instance, _, _, mars_token_instance) =
        instantiate_red_bank(&mut app, owner.clone());

    mint_some_mars(
        &mut app,
        owner.clone(),
        mars_token_instance.clone(),
        Uint128::new(900_000_0000_000),
        owner.to_string(),
    );

    let (_, _) = instantiate_auction_contract(
        &mut app,
        owner.clone(),
        mars_token_instance.clone(),
        Addr::unchecked("airdrop_instance"),
        lockdrop_instance.clone(),
    );

    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: UpdateConfigMsg {
                owner: None,
                address_provider: Some(address_provider_instance.to_string()),
                ma_ust_token: None,
                auction_contract_address: None,
            },
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        owner.clone(),
        mars_token_instance.clone(),
        &Cw20ExecuteMsg::Send {
            amount: Uint128::from(1000000000000u64),
            contract: lockdrop_instance.to_string(),
            msg: to_binary(&Cw20HookMsg::IncreaseMarsIncentives {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    let user1_address = Addr::unchecked("user1");

    // Set user balances
    app.init_bank_balance(
        &user1_address.clone(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(20000000u128),
        }],
    )
    .unwrap();

    // for successful deposit
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10_000_03)
    });

    // ######    SUCCESS :: UST Successfully deposited     ######
    app.execute_contract(
        user1_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::DepositUst { duration: 6u64 },
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(10000u128),
        }],
    )
    .unwrap();

    app.execute_contract(
        user1_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::DepositUst { duration: 15u64 },
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(10000u128),
        }],
    )
    .unwrap();

    // ***
    // *** Test :: Error " Only Owner can call this function" ***
    // ***

    let err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::DepositUstInRedBank {},
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Unauthorized");

    // ***
    // *** Test :: Error " maUST address should be set" ***
    // ***

    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: UpdateConfigMsg {
                owner: None,
                address_provider: Some(address_provider_instance.clone().to_string()),
                ma_ust_token: None,
                auction_contract_address: None,
            },
        },
        &[],
    )
    .unwrap();

    let err = app
        .execute_contract(
            owner.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::DepositUstInRedBank {},
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: maUST not set");

    // ***
    // *** Test :: Error " maUST address should be set" ***
    // ***

    // Query maUST Money-market info
    let ma_ust_market: mars_core::red_bank::Market = app
        .wrap()
        .query_wasm_smart(
            &red_bank_instance,
            &mars_core::red_bank::msg::QueryMsg::Market {
                asset: mars_core::asset::Asset::Native {
                    denom: "uusd".to_string(),
                },
            },
        )
        .unwrap();

    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: UpdateConfigMsg {
                owner: None,
                address_provider: None,
                ma_ust_token: Some(ma_ust_market.ma_token_address.clone().to_string()),
                auction_contract_address: None,
            },
        },
        &[],
    )
    .unwrap();

    let err = app
        .execute_contract(
            owner.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::DepositUstInRedBank {},
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Lockdrop deposits haven't concluded yet"
    );

    // ######    SUCCESSFULLY DEPOSITED IN RED BANK ######

    // Check state response
    let state_resp_before: StateResponse = app
        .wrap()
        .query_wasm_smart(&lockdrop_instance, &QueryMsg::State {})
        .unwrap();

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(17_000_03)
    });

    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::DepositUstInRedBank {},
        &[],
    )
    .unwrap();

    // Check state response
    let state_resp_after: StateResponse = app
        .wrap()
        .query_wasm_smart(&lockdrop_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(
        state_resp_before.total_ust_locked,
        state_resp_after.final_ust_locked
    );
    assert_eq!(
        Uint128::from(20000000000u64),
        state_resp_after.final_maust_locked
    );
    assert_eq!(
        state_resp_after.final_ust_locked,
        state_resp_before.total_ust_locked
    );
    assert_eq!(
        state_resp_after.final_maust_locked,
        state_resp_after.total_maust_locked
    );
    assert_eq!(Uint128::from(0u64), state_resp_after.total_mars_delegated);
    assert_eq!(false, state_resp_after.are_claims_allowed);
    assert_eq!(
        Uint128::from(70000u64),
        state_resp_after.total_deposits_weight
    );
    assert_eq!(Decimal::zero(), state_resp_after.xmars_rewards_index);

    // maUST balance
    let ma_ust_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &ma_ust_market.ma_token_address.clone().to_string(),
            &Cw20QueryMsg::Balance {
                address: lockdrop_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(state_resp_after.final_maust_locked, ma_ust_balance.balance);

    // Check user response
    let user_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(20000000000u64), user_resp.total_maust_share);
    assert_eq!(Uint128::from(20000u64), user_resp.total_ust_locked);
    assert_eq!(
        Uint128::from(999999999999u128),
        user_resp.total_mars_incentives
    );

    // ***
    // *** Test :: Error " Already deposited" ***
    // ***

    let err = app
        .execute_contract(
            owner.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::DepositUstInRedBank {},
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Already deposited");
}

#[test]
fn test_enable_claims() {
    let mut app = mock_app();
    let owner = Addr::unchecked("contract_owner");
    let (lockdrop_instance, _) = instantiate_lockdrop_contract(&mut app, owner.clone(), None, None);

    // ******* Initialize Address Provider & Auction  *******

    let (address_provider_instance, red_bank_instance, _, _, mars_token_instance) =
        instantiate_red_bank(&mut app, owner.clone());

    mint_some_mars(
        &mut app,
        owner.clone(),
        mars_token_instance.clone(),
        Uint128::new(900_000_0000_000),
        owner.to_string(),
    );

    let (auction_instance, _) = instantiate_auction_contract(
        &mut app,
        owner.clone(),
        mars_token_instance.clone(),
        Addr::unchecked("airdrop_instance"),
        lockdrop_instance.clone(),
    );

    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: UpdateConfigMsg {
                owner: None,
                address_provider: Some(address_provider_instance.to_string()),
                ma_ust_token: None,
                auction_contract_address: None,
            },
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        owner.clone(),
        mars_token_instance.clone(),
        &Cw20ExecuteMsg::Send {
            amount: Uint128::from(1000000000000u64),
            contract: lockdrop_instance.to_string(),
            msg: to_binary(&Cw20HookMsg::IncreaseMarsIncentives {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    let user1_address = Addr::unchecked("user1");

    // Set user balances
    app.init_bank_balance(
        &user1_address.clone(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(20000000u128),
        }],
    )
    .unwrap();

    // for successful deposit
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10_000_03)
    });

    // ######    SUCCESS :: UST Successfully deposited     ######
    app.execute_contract(
        user1_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::DepositUst { duration: 6u64 },
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(10000u128),
        }],
    )
    .unwrap();

    app.execute_contract(
        user1_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::DepositUst { duration: 15u64 },
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(10000u128),
        }],
    )
    .unwrap();

    // Query maUST Money-market info
    let ma_ust_market: mars_core::red_bank::Market = app
        .wrap()
        .query_wasm_smart(
            &red_bank_instance,
            &mars_core::red_bank::msg::QueryMsg::Market {
                asset: mars_core::asset::Asset::Native {
                    denom: "uusd".to_string(),
                },
            },
        )
        .unwrap();

    // ***
    // *** Test :: Error "Auction address in lockdrop not set" ***
    // ***

    let err = app
        .execute_contract(
            Addr::unchecked("not_auction".to_string()),
            lockdrop_instance.clone(),
            &ExecuteMsg::EnableClaims {},
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Auction address in lockdrop not set"
    );

    // Set address provider and other addresses
    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: UpdateConfigMsg {
                owner: None,
                address_provider: Some(address_provider_instance.clone().to_string()),
                ma_ust_token: Some(ma_ust_market.ma_token_address.clone().to_string()),
                auction_contract_address: Some(auction_instance.clone().to_string()),
            },
        },
        &[],
    )
    .unwrap();

    // ***
    // *** Test :: Error " Only Auction contract can call this function" ***
    // ***

    let err = app
        .execute_contract(
            Addr::unchecked("not_auction".to_string()),
            lockdrop_instance.clone(),
            &ExecuteMsg::EnableClaims {},
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Unauthorized");

    // ######    SUCCESSFULLY ENABLED CLAIMS ######

    // Check state response
    let state_resp_before: StateResponse = app
        .wrap()
        .query_wasm_smart(&lockdrop_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(false, state_resp_before.are_claims_allowed);

    app.execute_contract(
        auction_instance.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::EnableClaims {},
        &[],
    )
    .unwrap();

    // Check state response
    let state_resp_after: StateResponse = app
        .wrap()
        .query_wasm_smart(&lockdrop_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(true, state_resp_after.are_claims_allowed);

    // ***
    // *** Test :: Error " Already allowed" ***
    // ***

    let err = app
        .execute_contract(
            auction_instance.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::EnableClaims {},
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Already allowed");
}

#[test]
fn test_claim_rewards_and_unlock() {
    let mut app = mock_app();
    let owner = Addr::unchecked("contract_owner");
    let (lockdrop_instance, _) = instantiate_lockdrop_contract(&mut app, owner.clone(), None, None);

    // ******* Initialize Address Provider & Auction  *******

    let (
        address_provider_instance,
        red_bank_instance,
        incentives_instance,
        xmars_token_instance,
        mars_token_instance,
    ) = instantiate_red_bank(&mut app, owner.clone());

    mint_some_mars(
        &mut app,
        owner.clone(),
        mars_token_instance.clone(),
        Uint128::new(900_000_0000_000),
        owner.to_string(),
    );

    let (auction_instance, _) = instantiate_auction_contract(
        &mut app,
        owner.clone(),
        mars_token_instance.clone(),
        Addr::unchecked("airdrop_instance"),
        lockdrop_instance.clone(),
    );

    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: UpdateConfigMsg {
                owner: None,
                address_provider: Some(address_provider_instance.to_string()),
                ma_ust_token: None,
                auction_contract_address: None,
            },
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        owner.clone(),
        mars_token_instance.clone(),
        &Cw20ExecuteMsg::Send {
            amount: Uint128::from(1000000000000u64),
            contract: lockdrop_instance.to_string(),
            msg: to_binary(&Cw20HookMsg::IncreaseMarsIncentives {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    let user1_address = Addr::unchecked("user1");
    let user2_address = Addr::unchecked("user2");
    let user3_address = Addr::unchecked("user3");

    // Set user balances
    app.init_bank_balance(
        &user1_address.clone(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(51000000000u128),
        }],
    )
    .unwrap();
    app.init_bank_balance(
        &user2_address.clone(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(51000000000u128),
        }],
    )
    .unwrap();
    app.init_bank_balance(
        &user3_address.clone(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(51000000000u128),
        }],
    )
    .unwrap();

    // for successful deposit
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10_000_03)
    });

    // ######    SUCCESS :: UST Successfully deposited     ######
    app.execute_contract(
        user1_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::DepositUst { duration: 6u64 },
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(10000u128),
        }],
    )
    .unwrap();

    app.execute_contract(
        user1_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::DepositUst { duration: 15u64 },
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(10000u128),
        }],
    )
    .unwrap();

    app.execute_contract(
        user2_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::DepositUst { duration: 12u64 },
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000000u128),
        }],
    )
    .unwrap();

    app.execute_contract(
        user2_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::DepositUst { duration: 9u64 },
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000000u128),
        }],
    )
    .unwrap();

    app.execute_contract(
        user3_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::DepositUst { duration: 6u64 },
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000000u128),
        }],
    )
    .unwrap();

    app.execute_contract(
        user3_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::DepositUst { duration: 12u64 },
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(10000000u128),
        }],
    )
    .unwrap();

    // *** Update Configuration ***

    // Query maUST Money-market info
    let ma_ust_market: mars_core::red_bank::Market = app
        .wrap()
        .query_wasm_smart(
            &red_bank_instance,
            &mars_core::red_bank::msg::QueryMsg::Market {
                asset: mars_core::asset::Asset::Native {
                    denom: "uusd".to_string(),
                },
            },
        )
        .unwrap();

    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: UpdateConfigMsg {
                owner: None,
                address_provider: Some(address_provider_instance.clone().to_string()),
                ma_ust_token: Some(ma_ust_market.ma_token_address.clone().to_string()),
                auction_contract_address: Some(auction_instance.clone().to_string()),
            },
        },
        &[],
    )
    .unwrap();

    // ***
    // *** Test :: Error "Invalid lockup" ***
    // ***

    let err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::ClaimRewardsAndUnlock {
                lockup_to_unlock_duration: Some(3u64),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Invalid lockup");

    // ***
    // *** Test :: Error "Invalid lockup" ***
    // ***

    let err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::ClaimRewardsAndUnlock {
                lockup_to_unlock_duration: Some(6u64),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: 4328798 seconds to Unlock");

    // ***
    // *** Test :: Error "No lockup to claim rewards for" ***
    // ***

    let err = app
        .execute_contract(
            Addr::unchecked("not_user".to_string()),
            lockdrop_instance.clone(),
            &ExecuteMsg::ClaimRewardsAndUnlock {
                lockup_to_unlock_duration: None,
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: No lockup to claim rewards for"
    );

    // ***
    // *** Test :: Error "Claim not allowed" ***
    // ***

    let err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::ClaimRewardsAndUnlock {
                lockup_to_unlock_duration: None,
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Claim not allowed");

    // Enable claims
    app.execute_contract(
        auction_instance.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::EnableClaims {},
        &[],
    )
    .unwrap();

    // ######    SUCCESS :: MARS Lockdrop rewards successfully claimed      ######

    // Check user response
    let user_resp_before: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(1426533522u64),
        user_resp_before.total_mars_incentives
    );

    app.execute_contract(
        user1_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::ClaimRewardsAndUnlock {
            lockup_to_unlock_duration: None,
        },
        &[],
    )
    .unwrap();

    // Check user response
    let user_resp_after: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        user_resp_after.total_mars_incentives,
        user_resp_before.total_mars_incentives
    );
    assert_eq!(true, user_resp_after.is_lockdrop_claimed);
    assert_eq!(
        user_resp_before.pending_xmars_to_claim,
        user_resp_after.pending_xmars_to_claim
    );
    assert_eq!(Uint128::zero(), user_resp_after.pending_xmars_to_claim);

    let user1_mars_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &mars_token_instance.clone().to_string(),
            &Cw20QueryMsg::Balance {
                address: user1_address.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        user_resp_before.total_mars_incentives,
        user1_mars_balance.balance
    );

    // ######    SUCCESS :: MARS Lockdrop rewards (+ xMARS) successfully claimed (After UST is deposited in Red Bank)     ######

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(17_000_03)
    });

    // mint MARS  to Incentives Contract
    mint_some_mars(
        &mut app,
        owner.clone(),
        mars_token_instance.clone(),
        Uint128::new(351199080000000),
        incentives_instance.clone().to_string(),
    );

    // Set Incentives
    app.execute_contract(
        owner.clone(),
        incentives_instance.clone(),
        &mars_core::incentives::msg::ExecuteMsg::SetAssetIncentive {
            ma_token_address: ma_ust_market.ma_token_address.clone().to_string(),
            emission_per_second: Uint128::from(10000000u64),
        },
        &[],
    )
    .unwrap();

    // Deposit UST in Red Bank
    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::DepositUstInRedBank {},
        &[],
    )
    .unwrap();

    // Check incentives
    let asset_incentive_response: mars_core::incentives::AssetIncentiveResponse = app
        .wrap()
        .query_wasm_smart(
            &incentives_instance,
            &mars_core::incentives::msg::QueryMsg::AssetIncentive {
                ma_token_address: ma_ust_market.ma_token_address.clone().to_string(),
            },
        )
        .unwrap();

    assert_eq!(
        Uint128::from(10000000u64),
        asset_incentive_response
            .asset_incentive
            .clone()
            .unwrap()
            .emission_per_second
    );
    assert_eq!(
        mars_core::math::decimal::Decimal::zero(),
        asset_incentive_response
            .asset_incentive
            .clone()
            .unwrap()
            .index
    );
    assert_eq!(
        1700003u64,
        asset_incentive_response
            .asset_incentive
            .clone()
            .unwrap()
            .last_updated
    );

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(17_000_93)
    });

    // Check user response
    let user_resp_before: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(1382488u64),
        user_resp_before.pending_xmars_to_claim
    );

    // xMars to be claimed (Lockdrop)
    let xmars_pending: Uint128 = app
        .wrap()
        .query_wasm_smart(
            &incentives_instance.clone().to_string(),
            &mars_core::incentives::msg::QueryMsg::UserUnclaimedRewards {
                user_address: lockdrop_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(899999999u64), xmars_pending);

    // Check user response : before
    let user_resp_before: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(1382488u64),
        user_resp_before.pending_xmars_to_claim
    );

    // Check user's MARS Balance
    let user1_mars_balance_before: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &mars_token_instance.clone().to_string(),
            &Cw20QueryMsg::Balance {
                address: user1_address.clone().to_string(),
            },
        )
        .unwrap();

    // claim rewards : xMars (Mars already claimed)
    app.execute_contract(
        user1_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::ClaimRewardsAndUnlock {
            lockup_to_unlock_duration: None,
        },
        &[],
    )
    .unwrap();

    // Check user response : after
    let user_resp_after: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(0u64), user_resp_after.pending_xmars_to_claim);

    // Check user's xMARS Balance
    let user1_xmars_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &xmars_token_instance.clone().to_string(),
            &Cw20QueryMsg::Balance {
                address: user1_address.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        user_resp_before.pending_xmars_to_claim,
        user1_xmars_balance.balance
    );

    // Check lockdrop contract's xMARS Balance
    let lockdrop_xmars_balance: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &xmars_token_instance.clone().to_string(),
            &Cw20QueryMsg::Balance {
                address: lockdrop_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        xmars_pending,
        lockdrop_xmars_balance.balance + user_resp_before.pending_xmars_to_claim
    );

    // Check user's MARS Balance
    let user1_mars_balance_after: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &mars_token_instance.clone().to_string(),
            &Cw20QueryMsg::Balance {
                address: user1_address.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        user1_mars_balance_before.balance,
        user1_mars_balance_after.balance
    );

    // ######    SUCCESS :: Unlock Position : Claim MARS Lockdrop rewards (+ xMARS) successfully   ######

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(16820001)
    });

    // Check user response : before
    let user_resp_before: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::UserInfo {
                address: user2_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(23225803379404u64),
        user_resp_before.pending_xmars_to_claim
    );
    assert_eq!(
        Uint128::from(142653352353u64),
        user_resp_before.total_mars_incentives
    );
    assert_eq!(false, user_resp_before.is_lockdrop_claimed);

    // Check user's MARS Balance
    let user2_mars_balance_before: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &mars_token_instance.clone().to_string(),
            &Cw20QueryMsg::Balance {
                address: user2_address.clone().to_string(),
            },
        )
        .unwrap();

    // Check user's xMARS Balance
    let user2_xmars_balance_before: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &mars_token_instance.clone().to_string(),
            &Cw20QueryMsg::Balance {
                address: user2_address.clone().to_string(),
            },
        )
        .unwrap();

    // Check lockup
    let lockup_response_before: LockupInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance.clone().to_string(),
            &QueryMsg::LockupInfo {
                address: user2_address.clone().to_string(),
                duration: 9u64,
            },
        )
        .unwrap();

    let lockup_before = lockup_response_before.lockup_info.unwrap();

    // claim rewards & Unlock Position : xMars
    app.execute_contract(
        user2_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::ClaimRewardsAndUnlock {
            lockup_to_unlock_duration: Some(9u64),
        },
        &[],
    )
    .unwrap();

    // Check user response : after
    let user_resp_after: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::UserInfo {
                address: user2_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(0u64), user_resp_after.pending_xmars_to_claim);
    assert_eq!(true, user_resp_after.is_lockdrop_claimed);
    assert_eq!(
        vec!["user212".to_string()],
        user_resp_after.lockup_position_ids
    );

    // Check user's xMARS Balance
    let user2_xmars_balance_after: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &xmars_token_instance.clone().to_string(),
            &Cw20QueryMsg::Balance {
                address: user2_address.clone().to_string(),
            },
        )
        .unwrap();
    let user2_mars_balance_after: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &mars_token_instance.clone().to_string(),
            &Cw20QueryMsg::Balance {
                address: user2_address.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        user2_xmars_balance_before.balance + user_resp_before.pending_xmars_to_claim,
        user2_xmars_balance_after.balance
    );
    assert_eq!(
        user2_mars_balance_before.balance + user_resp_before.total_mars_incentives,
        user2_mars_balance_after.balance
    );

    // Check lockup
    let lockup_after_response: LockupInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance.clone().to_string(),
            &QueryMsg::LockupInfo {
                address: user2_address.clone().to_string(),
                duration: 9u64,
            },
        )
        .unwrap();

    assert_eq!(lockup_after_response.lockup_info, None);

    let user2_ma_token_balance_after: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &ma_ust_market.ma_token_address.clone().to_string(),
            &Cw20QueryMsg::Balance {
                address: user2_address.clone().to_string(),
            },
        )
        .unwrap();

    assert_eq!(
        lockup_before.maust_balance,
        user2_ma_token_balance_after.balance
    );

    // ######    SUCCESS :: Forcefully Unlock Position : Claim MARS Lockdrop rewards (+ xMARS) successfully   ######

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(16820003)
    });

    // mint MARS for to User-3 Contract
    mint_some_mars(
        &mut app,
        owner.clone(),
        mars_token_instance.clone(),
        Uint128::new(100_000_000_000),
        user3_address.clone().to_string(),
    );
    // Check user response : before
    let user_resp_before: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::UserInfo {
                address: user3_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(127741935483858u64),
        user_resp_before.pending_xmars_to_claim
    );
    assert_eq!(
        Uint128::from(855920114122u64),
        user_resp_before.total_mars_incentives
    );
    assert_eq!(false, user_resp_before.is_lockdrop_claimed);

    // Check user's MARS Balance
    let user3_mars_balance_before: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &mars_token_instance.clone().to_string(),
            &Cw20QueryMsg::Balance {
                address: user3_address.clone().to_string(),
            },
        )
        .unwrap();

    // Check user's xMARS Balance
    let user3_xmars_balance_before: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &xmars_token_instance.clone().to_string(),
            &Cw20QueryMsg::Balance {
                address: user3_address.clone().to_string(),
            },
        )
        .unwrap();

    // Increase allowance
    app.execute_contract(
        user3_address.clone(),
        mars_token_instance.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: lockdrop_instance.clone().to_string(),
            amount: Uint128::from(1000000000000000000u64),
            expires: None,
        },
        &[],
    )
    .unwrap();

    // claim rewards & Forcefully Unlock Position
    app.execute_contract(
        user3_address.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::ClaimRewardsAndUnlock {
            lockup_to_unlock_duration: Some(12u64),
        },
        &[],
    )
    .unwrap();

    // Check user response : after
    let user_resp_after: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::UserInfo {
                address: user3_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(0u64), user_resp_after.pending_xmars_to_claim);
    assert_eq!(true, user_resp_after.is_lockdrop_claimed);
    assert_eq!(
        vec!["user36".to_string()],
        user_resp_after.lockup_position_ids
    );

    // Check user's xMARS Balance
    let user3_xmars_balance_after: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &xmars_token_instance.clone().to_string(),
            &Cw20QueryMsg::Balance {
                address: user3_address.clone().to_string(),
            },
        )
        .unwrap();
    let user3_mars_balance_after: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &mars_token_instance.clone().to_string(),
            &Cw20QueryMsg::Balance {
                address: user3_address.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        user3_xmars_balance_before.balance + user_resp_before.pending_xmars_to_claim,
        user3_xmars_balance_after.balance
    );
    assert_eq!(
        user3_mars_balance_before.balance + user_resp_before.total_mars_incentives,
        user3_mars_balance_after.balance
    );
}
