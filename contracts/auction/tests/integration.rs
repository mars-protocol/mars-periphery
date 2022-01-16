use astroport::vesting::{
    Cw20HookMsg as VestingHookMsg, InstantiateMsg as VestingInstantiateMsg, VestingAccount,
    VestingSchedule, VestingSchedulePoint,
};
use cosmwasm_std::testing::{mock_env, MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{attr, to_binary, Addr, Coin, Decimal, Timestamp, Uint128, Uint64};
use cw20::Cw20ExecuteMsg;
use mars_periphery::auction::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, StateResponse,
    UpdateConfigMsg, UserInfoResponse,
};
use mars_periphery::lockdrop::LockupDurationParams;
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
        name: String::from("Mars token"),
        symbol: String::from("MARS"),
        decimals: 6,
        initial_balances: vec![],
        mint: Some(cw20::MinterResponse {
            minter: owner.to_string(),
            cap: None,
        }),
        marketing: None,
    };

    app.instantiate_contract(
        mars_token_code_id,
        owner,
        &msg,
        &[],
        String::from("MARS"),
        None,
    )
    .unwrap()
}

// Mints some TOKENS to "to" recipient
fn mint_some_tokens(app: &mut App, owner: Addr, token_instance: Addr, amount: Uint128, to: String) {
    let msg = cw20::Cw20ExecuteMsg::Mint {
        recipient: to.clone(),
        amount: amount,
    };
    let res = app
        .execute_contract(owner.clone(), token_instance.clone(), &msg, &[])
        .unwrap();
    assert_eq!(res.events[1].attributes[1], attr("action", "mint"));
    assert_eq!(res.events[1].attributes[2], attr("to", to));
    assert_eq!(res.events[1].attributes[3], attr("amount", amount));
}

// Instantiate Astroport's ASTRO, generator and vesting contracts
fn instantiate_astro_and_generator_and_vesting(
    mut app: &mut App,
    owner: Addr,
    astro_token_instance: Addr,
) -> (Addr, Addr, Addr) {
    // Vesting
    let vesting_contract = Box::new(ContractWrapper::new(
        astroport_vesting::contract::execute,
        astroport_vesting::contract::instantiate,
        astroport_vesting::contract::query,
    ));
    // let owner = Addr::unchecked(owner.clone());
    let vesting_code_id = app.store_code(vesting_contract);

    let init_msg = VestingInstantiateMsg {
        token_addr: astro_token_instance.to_string(),
    };

    let vesting_instance = app
        .instantiate_contract(
            vesting_code_id,
            owner.clone(),
            &init_msg,
            &[],
            "Vesting",
            None,
        )
        .unwrap();

    mint_some_tokens(
        &mut app,
        owner.clone(),
        astro_token_instance.clone(),
        Uint128::from(1_000_000_000_000000u64),
        owner.to_string(),
    );

    // Generator
    let generator_contract = Box::new(
        ContractWrapper::new(
            astroport_generator::contract::execute,
            astroport_generator::contract::instantiate,
            astroport_generator::contract::query,
        )
        .with_reply(astroport_generator::contract::reply),
    );

    let generator_code_id = app.store_code(generator_contract);

    let init_msg = astroport::generator::InstantiateMsg {
        owner: owner.to_string(),
        allowed_reward_proxies: vec![],
        start_block: Uint64::from(app.block_info().height),
        astro_token: astro_token_instance.to_string(),
        tokens_per_block: Uint128::new(10_000000),
        vesting_contract: vesting_instance.to_string(),
    };

    let generator_instance = app
        .instantiate_contract(
            generator_code_id,
            owner.clone(),
            &init_msg,
            &[],
            "Guage",
            None,
        )
        .unwrap();

    // vesting to generator:

    let current_block = app.block_info();

    let amount = Uint128::new(63072000_000000);

    let msg = Cw20ExecuteMsg::Send {
        contract: vesting_instance.to_string(),
        msg: to_binary(&VestingHookMsg::RegisterVestingAccounts {
            vesting_accounts: vec![VestingAccount {
                address: generator_instance.to_string(),
                schedules: vec![VestingSchedule {
                    start_point: VestingSchedulePoint {
                        time: current_block.time,
                        amount,
                    },
                    end_point: None,
                }],
            }],
        })
        .unwrap(),
        amount,
    };

    app.execute_contract(owner, astro_token_instance.clone(), &msg, &[])
        .unwrap();

    (astro_token_instance, generator_instance, vesting_instance)
}

// Instantiate AUCTION Contract
fn instantiate_auction_contract(
    app: &mut App,
    owner: Addr,
    mars_token_instance: Addr,
    astro_token_instance: Addr,
    airdrop_instance: Addr,
    lockdrop_instance: Addr,
    generator_instance: Addr,
) -> (Addr, InstantiateMsg) {
    let auction_contract = Box::new(ContractWrapper::new(
        mars_auction::contract::execute,
        mars_auction::contract::instantiate,
        mars_auction::contract::query,
    ));

    let auction_code_id = app.store_code(auction_contract);

    let auction_instantiate_msg = mars_periphery::auction::InstantiateMsg {
        owner: owner.to_string(),
        mars_token_address: mars_token_instance.clone().into_string(),
        astro_token_address: astro_token_instance.clone().into_string(),
        airdrop_contract_address: airdrop_instance.to_string(),
        lockdrop_contract_address: lockdrop_instance.to_string(),
        generator_contract: generator_instance.to_string(),
        mars_vesting_duration: 259200u64,
        lp_tokens_vesting_duration: 7776000u64,
        init_timestamp: 17_000_00,
        ust_deposit_window: 5_000_00,
        mars_deposit_window: 5_000_00,
        withdrawal_window: 2_000_00,
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
            amount: Uint128::from(10000000000000u64),
            contract: auction_instance.to_string(),
            msg: to_binary(&Cw20HookMsg::IncreaseMarsIncentives {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    (auction_instance, auction_instantiate_msg)
}

// Initiates Airdrop and lockdrop contracts
fn instantiate_airdrop_lockdrop_contracts(
    app: &mut App,
    owner: Addr,
    mars_token_instance: Addr,
) -> (Addr, Addr) {
    let airdrop_contract = Box::new(ContractWrapper::new(
        mars_airdrop::contract::execute,
        mars_airdrop::contract::instantiate,
        mars_airdrop::contract::query,
    ));

    let airdrop_code_id = app.store_code(airdrop_contract);

    let airdrop_msg = mars_periphery::airdrop::InstantiateMsg {
        owner: Some(owner.clone().to_string()),
        mars_token_address: mars_token_instance.clone().into_string(),
        merkle_roots: Some(vec!["merkle_roots".to_string()]),
        from_timestamp: Some(10_000_01),
        to_timestamp: 1000_000_00,
    };

    // Airdrop Instance
    let airdrop_instance = app
        .instantiate_contract(
            airdrop_code_id,
            owner.clone(),
            &airdrop_msg,
            &[],
            String::from("airdrop_instance"),
            None,
        )
        .unwrap();

    // mint MARS for to Owner
    mint_some_tokens(
        app,
        owner.clone(),
        mars_token_instance.clone(),
        Uint128::new(100_000_000_000),
        owner.clone().to_string(),
    );

    // Set MARS airdrop incentives
    app.execute_contract(
        owner.clone(),
        mars_token_instance.clone(),
        &Cw20ExecuteMsg::Send {
            amount: Uint128::new(100_000_000000),
            contract: airdrop_instance.to_string(),
            msg: to_binary(&Cw20HookMsg::IncreaseMarsIncentives {}).unwrap(),
        },
        &[],
    )
    .unwrap();

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

    // Update address_provider Config
    app.execute_contract(
        owner.clone(),
        mars_address_provider_instance.clone(),
        &mars_core::address_provider::msg::ExecuteMsg::UpdateConfig {
            config: mars_core::address_provider::msg::ConfigParams {
                owner: None,
                council_address: None,
                incentives_address: None,
                safety_fund_address: None,
                mars_token_address: Some(mars_token_instance.to_string()),
                oracle_address: None,
                protocol_admin_address: None,
                protocol_rewards_collector_address: None,
                red_bank_address: None,
                staking_address: None,
                treasury_address: None,
                vesting_address: None,
                xmars_token_address: None,
            },
        },
        &[],
    )
    .unwrap();

    // Lockdrop Contract
    let lockdrop_contract = Box::new(ContractWrapper::new(
        mars_lockdrop::contract::execute,
        mars_lockdrop::contract::instantiate,
        mars_lockdrop::contract::query,
    ));

    let lockdrop_code_id = app.store_code(lockdrop_contract);

    let lockdrop_msg = mars_periphery::lockdrop::InstantiateMsg {
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

    let lockdrop_instance = app
        .instantiate_contract(
            lockdrop_code_id,
            owner.clone(),
            &lockdrop_msg,
            &[],
            String::from("lockdrop_instance"),
            None,
        )
        .unwrap();

    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &mars_periphery::lockdrop::ExecuteMsg::UpdateConfig {
            new_config: mars_periphery::lockdrop::UpdateConfigMsg {
                owner: None,
                address_provider: Some(mars_address_provider_instance.clone().to_string()),
                ma_ust_token: None,
                auction_contract_address: None,
            },
        },
        &[],
    )
    .unwrap();

    mint_some_tokens(
        app,
        owner.clone(),
        mars_token_instance.clone(),
        Uint128::new(100000000000000u128),
        owner.to_string(),
    );

    // Send MARS to Lockdrop
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

    (airdrop_instance, lockdrop_instance)
}

// Initiates Auction contract with proper Config
fn init_auction_mars_contracts(app: &mut App) -> (Addr, Addr, Addr, Addr, Addr, InstantiateMsg) {
    let owner = Addr::unchecked("contract_owner");
    let mars_token_instance = instantiate_mars_token(app, owner.clone());

    // for successful deposit
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10_000_00)
    });

    // Instantiate Airdrop / Lockdrop Contracts
    let (airdrop_instance, lockdrop_instance) =
        instantiate_airdrop_lockdrop_contracts(app, owner.clone(), mars_token_instance.clone());

    let astro_token_contract = Box::new(ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    ));

    let astro_token_code_id = app.store_code(astro_token_contract);

    let msg = cw20_base::msg::InstantiateMsg {
        name: String::from("astroport token"),
        symbol: String::from("ASTRO"),
        decimals: 6,
        initial_balances: vec![],
        mint: Some(cw20::MinterResponse {
            minter: owner.clone().to_string(),
            cap: None,
        }),
        marketing: None,
    };

    let astro_token_instance = app
        .instantiate_contract(
            astro_token_code_id,
            owner.clone(),
            &msg,
            &[],
            String::from("astro"),
            None,
        )
        .unwrap();

    let (astro_token_instance, generator_instance, _) = instantiate_astro_and_generator_and_vesting(
        app,
        owner.clone(),
        astro_token_instance.clone(),
    );

    // Instantiate Auction Contract
    let (auction_instance, auction_instantiate_msg) = instantiate_auction_contract(
        app,
        owner.clone(),
        mars_token_instance.clone(),
        astro_token_instance.clone(),
        airdrop_instance.clone(),
        lockdrop_instance.clone(),
        generator_instance.clone(),
    );

    // Set Auction Contract address in lockdrop
    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &mars_periphery::lockdrop::ExecuteMsg::UpdateConfig {
            new_config: mars_periphery::lockdrop::UpdateConfigMsg {
                owner: None,
                address_provider: None,
                ma_ust_token: None,
                auction_contract_address: Some(auction_instance.clone().to_string()),
            },
        },
        &[],
    )
    .unwrap();

    // Set Auction Contract address in Airdrop
    app.execute_contract(
        owner.clone(),
        airdrop_instance.clone(),
        &mars_periphery::airdrop::ExecuteMsg::UpdateConfig {
            owner: None,
            auction_contract_address: Some(auction_instance.clone().to_string()),
            merkle_roots: None,
            from_timestamp: None,
            to_timestamp: None,
        },
        &[],
    )
    .unwrap();

    (
        airdrop_instance,
        lockdrop_instance,
        auction_instance,
        mars_token_instance,
        generator_instance,
        auction_instantiate_msg,
    )
}

// Initiates Astroport Pair for MARS-UST Pool
fn instantiate_pair(app: &mut App, owner: Addr, mars_token_instance: Addr) -> (Addr, Addr) {
    let factory_contract = Box::new(
        ContractWrapper::new(
            astroport_factory::contract::execute,
            astroport_factory::contract::instantiate,
            astroport_factory::contract::query,
        )
        .with_reply(astroport_factory::contract::reply),
    );

    let lp_token_contract = Box::new(ContractWrapper::new(
        astroport_token::contract::execute,
        astroport_token::contract::instantiate,
        astroport_token::contract::query,
    ));

    let pair_contract = Box::new(
        ContractWrapper::new(
            astroport_pair::contract::execute,
            astroport_pair::contract::instantiate,
            astroport_pair::contract::query,
        )
        .with_reply(astroport_pair::contract::reply),
    );

    let factory_code_id = app.store_code(factory_contract);
    let lp_token_code_id = app.store_code(lp_token_contract);
    let pair_code_id = app.store_code(pair_contract);

    let pair_configs = vec![astroport::factory::PairConfig {
        code_id: pair_code_id,
        pair_type: astroport::factory::PairType::Xyk {},
        total_fee_bps: 100,
        maker_fee_bps: 10,
        is_disabled: None,
    }];

    let msg = astroport::factory::InstantiateMsg {
        pair_configs: pair_configs.clone(),
        token_code_id: lp_token_code_id,
        fee_address: None,
        owner: owner.to_string(),
        generator_address: Some(String::from("generator")),
    };

    let factory_instance = app
        .instantiate_contract(factory_code_id, owner.clone(), &msg, &[], "factory", None)
        .unwrap();

    let asset_infos = [
        astroport::asset::AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
        astroport::asset::AssetInfo::Token {
            contract_addr: mars_token_instance.clone(),
        },
    ];

    let msg = astroport::factory::ExecuteMsg::CreatePair {
        asset_infos: asset_infos.clone(),
        pair_type: astroport::factory::PairType::Xyk {},
        init_params: None,
    };

    app.execute_contract(owner.clone(), factory_instance.clone(), &msg, &[])
        .unwrap();

    let resp: astroport::asset::PairInfo = app
        .wrap()
        .query_wasm_smart(
            &factory_instance.clone(),
            &astroport::factory::QueryMsg::Pair {
                asset_infos: asset_infos.clone(),
            },
        )
        .unwrap();

    let pair_instance = resp.contract_addr;
    let lp_token_instance = resp.liquidity_token;

    (pair_instance, lp_token_instance)
}

// Makes MARS & UST deposits into Auction contract
fn make_mars_ust_deposits(
    app: &mut App,
    auction_instance: Addr,
    auction_init_msg: InstantiateMsg,
    mars_token_instance: Addr,
) -> (Addr, Addr, Addr) {
    let user1_address = Addr::unchecked("user1");
    let user2_address = Addr::unchecked("user2");
    let user3_address = Addr::unchecked("user3");

    // open claim period for successful deposit
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(17_000_01)
    });

    // ######    SUCCESS :: MARS Successfully deposited     ######
    app.execute_contract(
        Addr::unchecked(auction_init_msg.lockdrop_contract_address.clone()),
        mars_token_instance.clone(),
        &Cw20ExecuteMsg::Send {
            contract: auction_instance.clone().to_string(),
            amount: Uint128::new(100000000),
            msg: to_binary(&Cw20HookMsg::DepositMarsTokens {
                user_address: Addr::unchecked(user1_address.to_string()),
            })
            .unwrap(),
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked(auction_init_msg.lockdrop_contract_address.clone()),
        mars_token_instance.clone(),
        &Cw20ExecuteMsg::Send {
            contract: auction_instance.clone().to_string(),
            amount: Uint128::new(65435340),
            msg: to_binary(&Cw20HookMsg::DepositMarsTokens {
                user_address: Addr::unchecked(user2_address.to_string()),
            })
            .unwrap(),
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked(auction_init_msg.lockdrop_contract_address.clone()),
        mars_token_instance.clone(),
        &Cw20ExecuteMsg::Send {
            contract: auction_instance.clone().to_string(),
            amount: Uint128::new(76754654),
            msg: to_binary(&Cw20HookMsg::DepositMarsTokens {
                user_address: Addr::unchecked(user3_address.to_string()),
            })
            .unwrap(),
        },
        &[],
    )
    .unwrap();

    // Set user balances
    app.init_bank_balance(
        &user1_address.clone(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(20000000u128),
        }],
    )
    .unwrap();
    app.init_bank_balance(
        &user2_address.clone(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(5435435u128),
        }],
    )
    .unwrap();
    app.init_bank_balance(
        &user3_address.clone(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(43534534u128),
        }],
    )
    .unwrap();

    // deposit UST Msg
    let deposit_ust_msg = &ExecuteMsg::DepositUst {};

    // ######    SUCCESS :: UST Successfully deposited     ######
    app.execute_contract(
        user1_address.clone(),
        auction_instance.clone(),
        &deposit_ust_msg,
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(432423u128),
        }],
    )
    .unwrap();

    app.execute_contract(
        user2_address.clone(),
        auction_instance.clone(),
        &deposit_ust_msg,
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(454353u128),
        }],
    )
    .unwrap();

    app.execute_contract(
        user3_address.clone(),
        auction_instance.clone(),
        &deposit_ust_msg,
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(5643543u128),
        }],
    )
    .unwrap();

    (user1_address, user2_address, user3_address)
}

#[test]
fn proper_initialization_only_auction_astro() {
    let mut app = mock_app();
    let (_, _, auction_instance, _, _, auction_init_msg) = init_auction_mars_contracts(&mut app);

    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::Config {})
        .unwrap();

    // Check config
    assert_eq!(Addr::unchecked(auction_init_msg.owner), resp.owner);
    assert_eq!(auction_init_msg.mars_token_address, resp.mars_token_address);
    assert_eq!(
        auction_init_msg.astro_token_address,
        resp.astro_token_address
    );
    assert_eq!(
        auction_init_msg.airdrop_contract_address,
        resp.airdrop_contract_address
    );
    assert_eq!(
        auction_init_msg.lockdrop_contract_address,
        resp.lockdrop_contract_address
    );
    assert_eq!(auction_init_msg.generator_contract, resp.generator_contract);
    assert_eq!(Uint128::from(10000000000000u64), resp.mars_rewards);
    assert_eq!(
        auction_init_msg.mars_vesting_duration,
        resp.mars_vesting_duration
    );
    assert_eq!(
        auction_init_msg.lp_tokens_vesting_duration,
        resp.lp_tokens_vesting_duration
    );
    assert_eq!(auction_init_msg.init_timestamp, resp.init_timestamp);
    assert_eq!(
        auction_init_msg.mars_deposit_window,
        resp.mars_deposit_window
    );
    assert_eq!(auction_init_msg.ust_deposit_window, resp.ust_deposit_window);
    assert_eq!(auction_init_msg.withdrawal_window, resp.withdrawal_window);

    // Check state
    let resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();

    assert!(resp.total_mars_deposited.is_zero());
    assert!(resp.total_ust_deposited.is_zero());
    assert!(resp.lp_shares_minted.is_zero());
    assert!(resp.lp_shares_withdrawn.is_zero());
    assert!(!resp.are_staked_for_single_incentives);
    assert!(!resp.are_staked_for_dual_incentives);
    assert_eq!(0u64, resp.pool_init_timestamp);
    assert!(resp.global_mars_reward_index.is_zero());
    assert!(resp.global_astro_reward_index.is_zero());
}

// #[test]
// fn proper_initialization_all_contracts() {
//     let mut app = mock_app();
//     let (auction_instance, _, _, _, _, _, auction_init_msg) = init_all_contracts(&mut app);

//     let resp: ConfigResponse = app
//         .wrap()
//         .query_wasm_smart(&auction_instance, &QueryMsg::Config {})
//         .unwrap();

//     // Check config
//     assert_eq!(auction_init_msg.owner, Some(resp.owner.to_string()));
//     assert_eq!(auction_init_msg.mars_token_address, resp.mars_token_address);
//     assert_eq!(
//         auction_init_msg.airdrop_contract_address,
//         resp.airdrop_contract_address
//     );
//     assert_eq!(
//         auction_init_msg.lockdrop_contract_address,
//         resp.lockdrop_contract_address
//     );
//     assert_eq!(auction_init_msg.init_timestamp, resp.init_timestamp);
//     assert_eq!(auction_init_msg.deposit_window, resp.deposit_window);
//     assert_eq!(auction_init_msg.withdrawal_window, resp.withdrawal_window);

//     // Check state
//     let resp: StateResponse = app
//         .wrap()
//         .query_wasm_smart(&auction_instance, &QueryMsg::State {})
//         .unwrap();

//     assert!(resp.total_mars_deposited.is_zero());
//     assert!(resp.total_ust_deposited.is_zero());
//     assert!(resp.lp_shares_minted.is_none());
//     assert!(!resp.is_lp_staked);
//     assert_eq!(0u64, resp.pool_init_timestamp);
//     assert!(resp.generator_mars_per_share.is_zero());
// }

#[test]
fn test_delegate_mars_tokens_from_airdrop() {
    let mut app = mock_app();
    let (airdrop_instance, _, auction_instance, mars_token_instance, _, auction_init_msg) =
        init_auction_mars_contracts(&mut app);

    // mint MARS for to Wrong Airdrop Contract
    mint_some_tokens(
        &mut app,
        Addr::unchecked(auction_init_msg.owner.clone()),
        mars_token_instance.clone(),
        Uint128::new(100_000_000_000),
        "not_airdrop_instance".to_string(),
    );

    // deposit MARS Msg
    let send_cw20_msg = &Cw20ExecuteMsg::Send {
        contract: auction_instance.clone().to_string(),
        amount: Uint128::new(100000000),
        msg: to_binary(&Cw20HookMsg::DepositMarsTokens {
            user_address: Addr::unchecked("airdrop_recipient".to_string()),
        })
        .unwrap(),
    };

    // ######    ERROR :: Unauthorized     ######

    let mut err = app
        .execute_contract(
            Addr::unchecked("not_airdrop_instance"),
            mars_token_instance.clone(),
            &send_cw20_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Unauthorized");

    // ######    ERROR :: Amount must be greater than 0     ######

    err = app
        .execute_contract(
            airdrop_instance.clone(),
            mars_token_instance.clone(),
            &Cw20ExecuteMsg::Send {
                contract: auction_instance.clone().to_string(),
                amount: Uint128::new(0),
                msg: to_binary(&Cw20HookMsg::DepositMarsTokens {
                    user_address: Addr::unchecked("airdrop_recipient".to_string()),
                })
                .unwrap(),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Invalid zero amount");

    // ######    ERROR :: Deposit window closed     ######
    err = app
        .execute_contract(
            airdrop_instance.clone(),
            mars_token_instance.clone(),
            &send_cw20_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: MARS delegation window closed"
    );

    // open claim period for successful deposit
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(17_000_01)
    });

    // // ######    SUCCESS :: MARS Successfully deposited     ######

    app.execute_contract(
        airdrop_instance.clone(),
        mars_token_instance.clone(),
        &send_cw20_msg,
        &[],
    )
    .unwrap();

    // Check state response
    let state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::from(100000000u64), state_resp.total_mars_deposited);
    assert_eq!(Uint128::from(0u64), state_resp.total_ust_deposited);
    assert_eq!(Uint128::zero(), state_resp.lp_shares_minted);
    assert!(!state_resp.are_staked_for_single_incentives);
    assert!(!state_resp.are_staked_for_dual_incentives);
    assert!(state_resp.global_mars_reward_index.is_zero());
    assert!(state_resp.global_astro_reward_index.is_zero());

    // Check user response
    let user_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: "airdrop_recipient".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(100000000u64), user_resp.mars_deposited);
    assert_eq!(Uint128::from(0u64), user_resp.ust_deposited);
    assert_eq!(Uint128::from(0u64), user_resp.lp_shares);
    assert_eq!(Uint128::from(0u64), user_resp.withdrawn_lp_shares);
    assert_eq!(Uint128::from(0u64), user_resp.withdrawable_lp_shares);
    assert_eq!(
        Uint128::from(10000000000000u64),
        user_resp.total_auction_incentives
    );
    assert_eq!(Uint128::from(0u64), user_resp.withdrawn_auction_incentives);
    assert_eq!(
        Uint128::from(0u64),
        user_resp.withdrawable_auction_incentives
    );
    assert_eq!(Decimal::zero(), user_resp.mars_reward_index);
    assert_eq!(Uint128::from(0u64), user_resp.withdrawable_mars_incentives);
    assert_eq!(Uint128::from(0u64), user_resp.withdrawn_mars_incentives);
    assert_eq!(Decimal::zero(), user_resp.astro_reward_index);
    assert_eq!(Uint128::from(0u64), user_resp.withdrawable_astro_incentives);
    assert_eq!(Uint128::from(0u64), user_resp.withdrawn_astro_incentives);

    // ######    SUCCESS :: MARS Successfully deposited again   ######
    app.execute_contract(
        airdrop_instance.clone(),
        mars_token_instance.clone(),
        &send_cw20_msg,
        &[],
    )
    .unwrap();

    // Check state response
    let state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::from(200000000u64), state_resp.total_mars_deposited);
    assert_eq!(Uint128::from(0u64), state_resp.total_ust_deposited);

    // Check user response
    let user_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: "airdrop_recipient".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(200000000u64), user_resp.mars_deposited);
    assert_eq!(Uint128::from(0u64), user_resp.ust_deposited);
    assert_eq!(
        Uint128::from(10000000000000u64),
        user_resp.total_auction_incentives
    );

    // ######    ERROR :: Deposit window closed     ######

    // finish claim period for deposit failure
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(24_000_01)
    });

    err = app
        .execute_contract(
            airdrop_instance,
            mars_token_instance.clone(),
            &send_cw20_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: MARS delegation window closed"
    );
}

#[test]
fn test_delegate_mars_tokens_from_lockdrop() {
    let mut app = mock_app();
    let (_, lockdrop_instance, auction_instance, mars_token_instance, _, auction_init_msg) =
        init_auction_mars_contracts(&mut app);

    // mint MARS for to Lockdrop Contract
    mint_some_tokens(
        &mut app,
        Addr::unchecked(auction_init_msg.owner.clone()),
        mars_token_instance.clone(),
        Uint128::new(100_000_000_000),
        lockdrop_instance.clone().to_string(),
    );

    // mint MARS for to Wrong Lockdrop Contract
    mint_some_tokens(
        &mut app,
        Addr::unchecked(auction_init_msg.owner.clone()),
        mars_token_instance.clone(),
        Uint128::new(100_000_000_000),
        "not_lockdrop_instance".to_string(),
    );

    // deposit MARS Msg
    let send_cw20_msg = &Cw20ExecuteMsg::Send {
        contract: auction_instance.clone().to_string(),
        amount: Uint128::new(100000000),
        msg: to_binary(&Cw20HookMsg::DepositMarsTokens {
            user_address: Addr::unchecked("lockdrop_participant".to_string()),
        })
        .unwrap(),
    };

    // ######    ERROR :: Unauthorized     ######
    let mut err = app
        .execute_contract(
            Addr::unchecked("not_lockdrop_instance"),
            mars_token_instance.clone(),
            &send_cw20_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Unauthorized");

    // ######    ERROR :: Amount must be greater than 0     ######
    err = app
        .execute_contract(
            lockdrop_instance.clone(),
            mars_token_instance.clone(),
            &Cw20ExecuteMsg::Send {
                contract: auction_instance.clone().to_string(),
                amount: Uint128::new(0),
                msg: to_binary(&Cw20HookMsg::DepositMarsTokens {
                    user_address: Addr::unchecked("lockdrop_participant".to_string()),
                })
                .unwrap(),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Invalid zero amount");

    // ######    ERROR :: Deposit window closed     ######
    err = app
        .execute_contract(
            lockdrop_instance.clone(),
            mars_token_instance.clone(),
            &send_cw20_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: MARS delegation window closed"
    );

    // open claim period for successful deposit
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(17_000_01)
    });

    // ######    SUCCESS :: MARS Successfully deposited     ######

    app.execute_contract(
        lockdrop_instance.clone(),
        mars_token_instance.clone(),
        &send_cw20_msg,
        &[],
    )
    .unwrap();

    // Check state response
    let state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::from(100000000u64), state_resp.total_mars_deposited);

    // Check user response
    let user_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: "lockdrop_participant".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(100000000u64), user_resp.mars_deposited);
    assert_eq!(Uint128::from(0u64), user_resp.ust_deposited);
    assert_eq!(
        Uint128::from(10000000000000u64),
        user_resp.total_auction_incentives
    );

    // ######    SUCCESS :: MARS Successfully deposited again   ######
    app.execute_contract(
        lockdrop_instance.clone(),
        mars_token_instance.clone(),
        &send_cw20_msg,
        &[],
    )
    .unwrap();
    // Check state response
    let state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::from(200000000u64), state_resp.total_mars_deposited);
    assert_eq!(Uint128::from(0u64), state_resp.total_ust_deposited);

    // Check user response
    let user_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: "lockdrop_participant".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(200000000u64), user_resp.mars_deposited);
    assert_eq!(
        Uint128::from(10000000000000u64),
        user_resp.total_auction_incentives
    );

    // ######    ERROR :: Deposit window closed     ######

    // finish claim period for deposit failure
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10100001)
    });
    err = app
        .execute_contract(
            lockdrop_instance,
            mars_token_instance.clone(),
            &send_cw20_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: MARS delegation window closed"
    );
}

#[test]
fn test_update_config() {
    let mut app = mock_app();
    let owner = Addr::unchecked("contract_owner");
    let (_, _, auction_instance, mars_token_instance, _, auction_init_msg) =
        init_auction_mars_contracts(&mut app);

    let (pool_instance, _) = instantiate_pair(&mut app, owner, mars_token_instance);

    let update_msg = UpdateConfigMsg {
        owner: Some("new_owner".to_string()),
        astroport_lp_pool: Some(pool_instance.to_string()),
        mars_lp_staking_contract: Some("mars_lp_staking_contract".to_string()),
        generator_contract: Some("generator_contract".to_string()),
    };

    // ######    ERROR :: Only owner can update configuration     ######
    let err = app
        .execute_contract(
            Addr::unchecked("wrong_owner"),
            auction_instance.clone(),
            &ExecuteMsg::UpdateConfig {
                new_config: update_msg.clone(),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Only owner can update configuration"
    );

    // ######    SUCCESS :: Should have successfully updated   ######
    app.execute_contract(
        Addr::unchecked(auction_init_msg.owner.clone()),
        auction_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: update_msg.clone(),
        },
        &[],
    )
    .unwrap();

    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::Config {})
        .unwrap();
    // Check config
    assert_eq!(update_msg.clone().owner.unwrap(), resp.owner);
    assert_eq!(
        update_msg.clone().astroport_lp_pool.unwrap(),
        resp.astroport_lp_pool.unwrap()
    );
    assert_eq!(
        update_msg.clone().mars_lp_staking_contract.unwrap(),
        resp.mars_lp_staking_contract.unwrap()
    );
    assert_eq!(
        update_msg.clone().generator_contract.unwrap(),
        resp.generator_contract
    );
}

#[test]
fn test_deposit_ust() {
    let mut app = mock_app();
    let (_, _, auction_instance, _, _, _) = init_auction_mars_contracts(&mut app);
    let user_address = Addr::unchecked("user");

    // Set user balances
    app.init_bank_balance(
        &user_address.clone(),
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

    // deposit UST Msg
    let deposit_ust_msg = &ExecuteMsg::DepositUst {};
    let coins = [Coin {
        denom: "uusd".to_string(),
        amount: Uint128::from(10000u128),
    }];

    // ######    ERROR :: Deposit window closed     ######
    let mut err = app
        .execute_contract(
            user_address.clone(),
            auction_instance.clone(),
            &deposit_ust_msg,
            &coins,
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: UST deposits window closed");

    // open claim period for successful deposit
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(17_000_01)
    });

    // ######    ERROR :: Trying to deposit several coins     ######
    err = app
        .execute_contract(
            user_address.clone(),
            auction_instance.clone(),
            &deposit_ust_msg,
            &[
                Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::from(10u128),
                },
                Coin {
                    denom: "uluna".to_string(),
                    amount: Uint128::new(2000u128),
                },
            ],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Trying to deposit several coins"
    );

    // ######    ERROR :: Only UST among native tokens accepted     ######
    err = app
        .execute_contract(
            user_address.clone(),
            auction_instance.clone(),
            &deposit_ust_msg,
            &[Coin {
                denom: "uluna".to_string(),
                amount: Uint128::from(10u128),
            }],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Only UST among native tokens accepted"
    );

    // ######    ERROR :: Deposit amount must be greater than 0    ######
    err = app
        .execute_contract(
            user_address.clone(),
            auction_instance.clone(),
            &deposit_ust_msg,
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(0u128),
            }],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Deposit amount must be greater than 0"
    );

    // ######    SUCCESS :: UST Successfully deposited     ######
    app.execute_contract(
        user_address.clone(),
        auction_instance.clone(),
        &deposit_ust_msg,
        &coins,
    )
    .unwrap();
    // Check state response
    let mut state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::from(00u64), state_resp.total_mars_deposited);
    assert_eq!(Uint128::from(10000u64), state_resp.total_ust_deposited);

    // Check user response
    let mut user_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: user_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(0u64), user_resp.mars_deposited);
    assert_eq!(Uint128::from(10000u64), user_resp.ust_deposited);
    assert_eq!(Uint128::zero(), user_resp.lp_shares);
    assert_eq!(Uint128::from(0u64), user_resp.total_auction_incentives);

    // ######    SUCCESS :: UST Successfully deposited again     ######
    app.execute_contract(
        user_address.clone(),
        auction_instance.clone(),
        &deposit_ust_msg,
        &coins,
    )
    .unwrap();
    // Check state response
    state_resp = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::from(00u64), state_resp.total_mars_deposited);
    assert_eq!(Uint128::from(20000u64), state_resp.total_ust_deposited);

    // Check user response
    user_resp = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: user_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(0u64), user_resp.mars_deposited);
    assert_eq!(Uint128::from(20000u64), user_resp.ust_deposited);
    assert_eq!(Uint128::zero(), user_resp.lp_shares);
    assert_eq!(Uint128::from(00u64), user_resp.total_auction_incentives);

    // finish claim period for deposit failure
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(25100001)
    });

    err = app
        .execute_contract(
            user_address.clone(),
            auction_instance.clone(),
            &deposit_ust_msg,
            &coins,
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: UST deposits window closed");
}

#[test]
fn test_withdraw_ust() {
    let mut app = mock_app();
    let (_, _, auction_instance, _, _, _) = init_auction_mars_contracts(&mut app);
    let user1_address = Addr::unchecked("user1");
    let user2_address = Addr::unchecked("user2");
    let user3_address = Addr::unchecked("user3");

    // Set user balances
    app.init_bank_balance(
        &user1_address.clone(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(20000000u128),
        }],
    )
    .unwrap();
    app.init_bank_balance(
        &user2_address.clone(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(20000000u128),
        }],
    )
    .unwrap();
    app.init_bank_balance(
        &user3_address.clone(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(20000000u128),
        }],
    )
    .unwrap();

    // deposit UST Msg
    let deposit_ust_msg = &ExecuteMsg::DepositUst {};
    let coins = [Coin {
        denom: "uusd".to_string(),
        amount: Uint128::from(10000u128),
    }];

    // open claim period for successful deposit
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(17_000_01)
    });

    // ######    SUCCESS :: UST Successfully deposited     ######
    app.execute_contract(
        user1_address.clone(),
        auction_instance.clone(),
        &deposit_ust_msg,
        &coins,
    )
    .unwrap();
    app.execute_contract(
        user2_address.clone(),
        auction_instance.clone(),
        &deposit_ust_msg,
        &coins,
    )
    .unwrap();
    app.execute_contract(
        user3_address.clone(),
        auction_instance.clone(),
        &deposit_ust_msg,
        &coins,
    )
    .unwrap();

    // ######    SUCCESS :: UST Successfully withdrawn (when withdrawals allowed)     ######
    app.execute_contract(
        user1_address.clone(),
        auction_instance.clone(),
        &ExecuteMsg::WithdrawUst {
            amount: Uint128::from(10000u64),
        },
        &[],
    )
    .unwrap();
    // Check state response
    let mut state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::from(20000u64), state_resp.total_ust_deposited);

    // Check user response
    let mut user_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(0u64), user_resp.ust_deposited);

    app.execute_contract(
        user1_address.clone(),
        auction_instance.clone(),
        &deposit_ust_msg,
        &coins,
    )
    .unwrap();

    // close deposit window. Max 50% withdrawals allowed now
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(22_000_10)
    });

    // ######    ERROR :: Amount exceeds maximum allowed withdrawal limit of {}   ######

    let mut err = app
        .execute_contract(
            user1_address.clone(),
            auction_instance.clone(),
            &ExecuteMsg::WithdrawUst {
                amount: Uint128::from(10000u64),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Amount exceeds maximum allowed withdrawal limit of 5000 uusd"
    );

    // ######    SUCCESS :: Withdraw 50% successfully   ######

    app.execute_contract(
        user1_address.clone(),
        auction_instance.clone(),
        &ExecuteMsg::WithdrawUst {
            amount: Uint128::from(5000u64),
        },
        &[],
    )
    .unwrap();
    // Check state response
    state_resp = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::from(25000u64), state_resp.total_ust_deposited);

    // Check user response
    user_resp = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(5000u64), user_resp.ust_deposited);

    // ######    ERROR :: Max 1 withdrawal allowed during current window   ######

    err = app
        .execute_contract(
            user1_address.clone(),
            auction_instance.clone(),
            &ExecuteMsg::WithdrawUst {
                amount: Uint128::from(10u64),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Max 1 withdrawal allowed during current window"
    );

    // 50% of withdrawal window over. Max withdrawal % decreasing linearly now
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(23_000_10)
    });

    // ######    ERROR :: Amount exceeds maximum allowed withdrawal limit of {}   ######

    let mut err = app
        .execute_contract(
            user2_address.clone(),
            auction_instance.clone(),
            &ExecuteMsg::WithdrawUst {
                amount: Uint128::from(10000u64),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Amount exceeds maximum allowed withdrawal limit of 4999 uusd"
    );

    // ######    SUCCESS :: Withdraw some UST successfully   ######

    app.execute_contract(
        user2_address.clone(),
        auction_instance.clone(),
        &ExecuteMsg::WithdrawUst {
            amount: Uint128::from(2000u64),
        },
        &[],
    )
    .unwrap();
    // Check state response
    state_resp = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::from(23000u64), state_resp.total_ust_deposited);

    // Check user response
    user_resp = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: user2_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(8000u64), user_resp.ust_deposited);

    // ######    ERROR :: Max 1 withdrawal allowed during current window   ######

    err = app
        .execute_contract(
            user2_address.clone(),
            auction_instance.clone(),
            &ExecuteMsg::WithdrawUst {
                amount: Uint128::from(10u64),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Max 1 withdrawal allowed during current window"
    );

    // finish deposit period for deposit failure
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(27_000_10)
    });

    err = app
        .execute_contract(
            user3_address.clone(),
            auction_instance.clone(),
            &ExecuteMsg::WithdrawUst {
                amount: Uint128::from(10u64),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Amount exceeds maximum allowed withdrawal limit of 0 uusd"
    );
}

#[test]
fn test_add_liquidity_to_astroport_pool() {
    let mut app = mock_app();
    let owner = Addr::unchecked("contract_owner");

    let (
        airdrop_instance,
        lockdrop_instance,
        auction_instance,
        mars_token_instance,
        _,
        auction_init_msg,
    ) = init_auction_mars_contracts(&mut app);
    let (pool_instance, _) = instantiate_pair(&mut app, owner, mars_token_instance.clone());

    // Set pool address to which liquidity will be deposited
    app.execute_contract(
        Addr::unchecked(auction_init_msg.owner.clone()),
        auction_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: UpdateConfigMsg {
                owner: None,
                astroport_lp_pool: Some(pool_instance.to_string()),
                mars_lp_staking_contract: None,
                generator_contract: None,
            },
        },
        &[],
    )
    .unwrap();

    // mint MARS to Lockdrop Contract
    mint_some_tokens(
        &mut app,
        Addr::unchecked(auction_init_msg.owner.clone()),
        mars_token_instance.clone(),
        Uint128::new(100_000_000_000),
        auction_init_msg.lockdrop_contract_address.to_string(),
    );

    let (user1_address, _, _) = make_mars_ust_deposits(
        &mut app,
        auction_instance.clone(),
        auction_init_msg.clone(),
        mars_token_instance.clone(),
    );

    // ######    ERROR :: Unauthorized   ######

    let mut err = app
        .execute_contract(
            Addr::unchecked("not_owner".to_string()),
            auction_instance.clone(),
            &ExecuteMsg::AddLiquidityToAstroportPool { slippage: None },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Unauthorized");

    // ######    ERROR :: Deposit/withdrawal windows are still open   ######

    err = app
        .execute_contract(
            Addr::unchecked(auction_init_msg.owner.clone()),
            auction_instance.clone(),
            &ExecuteMsg::AddLiquidityToAstroportPool { slippage: None },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Deposit/withdrawal windows are still open"
    );

    // finish deposit / withdraw period
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(24_000_10)
    });

    let success_ = app
        .execute_contract(
            Addr::unchecked(auction_init_msg.owner.clone()),
            auction_instance.clone(),
            &ExecuteMsg::AddLiquidityToAstroportPool { slippage: None },
            &[],
        )
        .unwrap();
    assert_eq!(
        success_.events[1].attributes[1],
        attr("action", "Auction::ExecuteMsg::AddLiquidityToAstroportPool")
    );
    assert_eq!(
        success_.events[1].attributes[2],
        attr("mars_deposited", "242189994")
    );
    assert_eq!(
        success_.events[1].attributes[3],
        attr("ust_deposited", "6530319")
    );

    // Auction :: Check state response
    let state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::from(242189994u64), state_resp.total_mars_deposited);
    assert_eq!(Uint128::from(6530319u64), state_resp.total_ust_deposited);
    assert_eq!(Uint128::from(39769057u64), state_resp.lp_shares_minted);
    assert_eq!(2400010u64, state_resp.pool_init_timestamp);

    // Astroport Pool :: Check response
    let pool_resp: astroport::pair::PoolResponse = app
        .wrap()
        .query_wasm_smart(&pool_instance, &astroport::pair::QueryMsg::Pool {})
        .unwrap();
    assert_eq!(Uint128::from(39769057u64), pool_resp.total_share);

    // Airdrop :: Check config for claims
    let airdrop_config_resp: mars_periphery::airdrop::ConfigResponse = app
        .wrap()
        .query_wasm_smart(
            &airdrop_instance,
            &mars_periphery::airdrop::QueryMsg::Config {},
        )
        .unwrap();
    assert_eq!(true, airdrop_config_resp.are_claims_allowed);

    // Lockdrop :: Check state for claims
    let lockdrop_config_resp: mars_periphery::lockdrop::StateResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &mars_periphery::lockdrop::QueryMsg::State {},
        )
        .unwrap();
    assert_eq!(true, lockdrop_config_resp.are_claims_allowed);

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(17911001)
    });

    // Auction :: Check user-1 state
    let user1info_resp: mars_periphery::auction::UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &mars_periphery::auction::QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(100000000u64), user1info_resp.mars_deposited);
    assert_eq!(Uint128::from(432423u64), user1info_resp.ust_deposited);
    assert_eq!(Uint128::from(9527010u64), user1info_resp.lp_shares);
    assert_eq!(
        Uint128::from(9527010u64),
        user1info_resp.withdrawable_lp_shares
    );
    assert_eq!(
        Uint128::from(4128989738527u64),
        user1info_resp.total_auction_incentives
    );
    assert_eq!(
        Uint128::from(4128989738527u64),
        user1info_resp.withdrawable_auction_incentives
    );

    // ######    ERROR :: Liquidity already added   ######
    // user1_address, user2_address, user3_address
    err = app
        .execute_contract(
            Addr::unchecked(auction_init_msg.owner.clone()),
            auction_instance.clone(),
            &ExecuteMsg::AddLiquidityToAstroportPool { slippage: None },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Liquidity already provided to pool"
    );
}

#[test]
fn test_stake_lp_tokens_and_claim_rewards() {
    let mut app = mock_app();
    let owner = Addr::unchecked("contract_owner");

    let (_, _, auction_instance, mars_token_instance, generator_instance, auction_init_msg) =
        init_auction_mars_contracts(&mut app);
    let (pool_instance, lp_token_instance) =
        instantiate_pair(&mut app, owner, mars_token_instance.clone());

    // Set pool address to which liquidity will be deposited
    app.execute_contract(
        Addr::unchecked(auction_init_msg.owner.clone()),
        auction_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: UpdateConfigMsg {
                owner: None,
                astroport_lp_pool: Some(pool_instance.to_string()),
                mars_lp_staking_contract: None,
                generator_contract: None,
            },
        },
        &[],
    )
    .unwrap();

    // mint MARS to Lockdrop Contract
    mint_some_tokens(
        &mut app,
        Addr::unchecked(auction_init_msg.owner.clone()),
        mars_token_instance.clone(),
        Uint128::new(100_000_000_000),
        auction_init_msg.lockdrop_contract_address.to_string(),
    );

    // mint MARS to Auction Contract
    mint_some_tokens(
        &mut app,
        Addr::unchecked(auction_init_msg.owner.clone()),
        mars_token_instance.clone(),
        Uint128::new(100_000_000_000),
        auction_instance.clone().to_string(),
    );

    // Instantiate LP staking contract
    let lp_staking_contract = Box::new(ContractWrapper::new(
        mars_lp_staking::contract::execute,
        mars_lp_staking::contract::instantiate,
        mars_lp_staking::contract::query,
    ));

    let lp_staking_code_id = app.store_code(lp_staking_contract);

    let lp_staking_instance = app
        .instantiate_contract(
            lp_staking_code_id,
            Addr::unchecked(auction_init_msg.owner.clone()),
            &mars_periphery::lp_staking::InstantiateMsg {
                owner: Some(auction_init_msg.owner.clone()),
                mars_token: mars_token_instance.clone().to_string(),
                staking_token: Some(lp_token_instance.to_string()),
                init_timestamp: 24_000_01,
                till_timestamp: 24_000_000,
                cycle_rewards: Some(Uint128::from(100_000000u64)),
                cycle_duration: 86400u64,
                reward_increase: Some(Decimal::from_ratio(2u64, 100u64)),
            },
            &[],
            String::from("lp_staking"),
            None,
        )
        .unwrap();

    // MARS to LP Staking contract
    mint_some_tokens(
        &mut app,
        Addr::unchecked(auction_init_msg.owner.clone()),
        mars_token_instance.clone(),
        Uint128::new(10000_000_000_000),
        lp_staking_instance.clone().to_string(),
    );

    let (user1_address, _, _) = make_mars_ust_deposits(
        &mut app,
        auction_instance.clone(),
        auction_init_msg.clone(),
        mars_token_instance.clone(),
    );

    // finish deposit / withdraw period
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(24_000_10)
    });

    // Initialize MARS - UST POOL
    app.execute_contract(
        Addr::unchecked(auction_init_msg.owner.clone()),
        auction_instance.clone(),
        &ExecuteMsg::AddLiquidityToAstroportPool { slippage: None },
        &[],
    )
    .unwrap();

    // finish deposit / withdraw period
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(24_000_30)
    });

    // ######    ERROR :: Unauthorized   ######

    let mut err = app
        .execute_contract(
            Addr::unchecked("not_owner".to_string()),
            auction_instance.clone(),
            &ExecuteMsg::StakeLpTokens {
                single_incentive_staking: true,
                dual_incentives_staking: false,
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Unauthorized");

    // ######    ERROR :: Invalid values provided   ######

    err = app
        .execute_contract(
            Addr::unchecked(auction_init_msg.owner.clone()),
            auction_instance.clone(),
            &ExecuteMsg::StakeLpTokens {
                single_incentive_staking: true,
                dual_incentives_staking: true,
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Invalid values provided");

    err = app
        .execute_contract(
            Addr::unchecked(auction_init_msg.owner.clone()),
            auction_instance.clone(),
            &ExecuteMsg::StakeLpTokens {
                single_incentive_staking: false,
                dual_incentives_staking: false,
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Invalid values provided");

    // ######    ERROR :: LP Staking address not set  ######

    err = app
        .execute_contract(
            Addr::unchecked(auction_init_msg.owner.clone()),
            auction_instance.clone(),
            &ExecuteMsg::StakeLpTokens {
                single_incentive_staking: true,
                dual_incentives_staking: false,
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: LP Staking not set");

    // Set LP Staking contract
    app.execute_contract(
        Addr::unchecked(auction_init_msg.owner.clone()),
        auction_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: UpdateConfigMsg {
                owner: None,
                astroport_lp_pool: None,
                mars_lp_staking_contract: Some(lp_staking_instance.clone().to_string()),
                generator_contract: None,
            },
        },
        &[],
    )
    .unwrap();

    // ********
    // ******** SUCCESSFULLY STAKED WITH LP STAKING CONTRACT ********
    // ********

    app.execute_contract(
        Addr::unchecked(auction_init_msg.owner.clone()),
        auction_instance.clone(),
        &ExecuteMsg::StakeLpTokens {
            single_incentive_staking: true,
            dual_incentives_staking: false,
        },
        &[],
    )
    .unwrap();

    // Check state
    let state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::from(242189994u64), state_resp.total_mars_deposited);
    assert_eq!(Uint128::from(6530319u64), state_resp.total_ust_deposited);
    assert_eq!(Uint128::from(39769057u64), state_resp.lp_shares_minted);
    assert!(state_resp.lp_shares_withdrawn.is_zero());
    assert_eq!(true, state_resp.are_staked_for_single_incentives);
    assert_eq!(false, state_resp.are_staked_for_dual_incentives);
    assert!(state_resp.global_mars_reward_index.is_zero());
    assert!(state_resp.global_astro_reward_index.is_zero());

    // Check user response :: Check vesting calculations
    let user_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: user1_address.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(9527010u64), user_resp.lp_shares);
    assert_eq!(Uint128::from(0u64), user_resp.withdrawn_lp_shares);
    assert_eq!(Uint128::from(24u64), user_resp.withdrawable_lp_shares);
    assert_eq!(
        Uint128::from(4128989738527u64),
        user_resp.total_auction_incentives
    );
    assert_eq!(Uint128::from(0u64), user_resp.withdrawn_auction_incentives);
    assert_eq!(
        Uint128::from(318594887u64),
        user_resp.withdrawable_auction_incentives
    );
    assert_eq!(Decimal::zero(), user_resp.mars_reward_index);
    assert_eq!(Uint128::from(0u64), user_resp.withdrawable_mars_incentives);
    assert_eq!(Uint128::from(0u64), user_resp.withdrawn_mars_incentives);
    assert_eq!(Decimal::zero(), user_resp.astro_reward_index);
    assert_eq!(Uint128::from(0u64), user_resp.withdrawable_astro_incentives);
    assert_eq!(Uint128::from(0u64), user_resp.withdrawn_astro_incentives);

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(24_000_90)
    });

    // Check user response :: Check LP staking calculations
    let user_resp_before_claim: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: user1_address.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(98u64),
        user_resp_before_claim.withdrawable_lp_shares
    );
    assert_eq!(
        Uint128::from(1274379548u64),
        user_resp_before_claim.withdrawable_auction_incentives
    );
    assert_eq!(
        Uint128::from(16635u64),
        user_resp_before_claim.withdrawable_mars_incentives
    );
    assert_eq!(
        Uint128::from(0u64),
        user_resp_before_claim.withdrawn_mars_incentives
    );
    assert_eq!(Decimal::zero(), user_resp_before_claim.astro_reward_index);

    // ********
    // ******** USER SUCCESSFULLY CLAIMS REWARDS (WITHOUT WITHDRAWING UNLOCKEDLP SHARES) ********
    // ********

    app.execute_contract(
        user1_address.clone(),
        auction_instance.clone(),
        &ExecuteMsg::ClaimRewards {
            withdraw_unlocked_shares: false,
        },
        &[],
    )
    .unwrap();

    // Check user response
    let user_resp_after_claim: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: user1_address.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(98u64),
        user_resp_after_claim.withdrawable_lp_shares
    );
    assert_eq!(
        Uint128::from(0u64),
        user_resp_after_claim.withdrawn_lp_shares
    );
    assert_eq!(
        Uint128::from(0u64),
        user_resp_after_claim.withdrawable_auction_incentives
    );
    assert_eq!(
        Uint128::from(1274379548u64),
        user_resp_after_claim.withdrawn_auction_incentives
    );
    assert_eq!(
        Uint128::from(0u64),
        user_resp_after_claim.withdrawable_mars_incentives
    );
    assert_eq!(
        Uint128::from(16635u64),
        user_resp_after_claim.withdrawn_mars_incentives
    );
    assert_eq!(Decimal::zero(), user_resp_after_claim.astro_reward_index);

    // ********
    // ******** USER SUCCESSFULLY WITHDRAWS UNLOCKED LP SHARES ********
    // ********

    app.execute_contract(
        user1_address.clone(),
        auction_instance.clone(),
        &ExecuteMsg::ClaimRewards {
            withdraw_unlocked_shares: true,
        },
        &[],
    )
    .unwrap();

    // Check state
    let state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::from(98u64), state_resp.lp_shares_withdrawn);

    // Check user response
    let user_resp_after_withdraw: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: user1_address.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(0u64),
        user_resp_after_withdraw.withdrawable_lp_shares
    );
    assert_eq!(
        Uint128::from(98u64),
        user_resp_after_withdraw.withdrawn_lp_shares
    );
    assert_eq!(
        Uint128::from(0u64),
        user_resp_after_claim.withdrawable_auction_incentives
    );
    assert_eq!(
        Uint128::from(1274379548u64),
        user_resp_after_claim.withdrawn_auction_incentives
    );
    assert_eq!(
        Uint128::from(0u64),
        user_resp_after_claim.withdrawable_mars_incentives
    );
    assert_eq!(
        Uint128::from(16635u64),
        user_resp_after_claim.withdrawn_mars_incentives
    );
    assert_eq!(Decimal::zero(), user_resp_after_claim.astro_reward_index);

    // ######    ERROR :: Invalid values provided   ######

    err = app
        .execute_contract(
            Addr::unchecked(auction_init_msg.owner.clone()),
            auction_instance.clone(),
            &ExecuteMsg::StakeLpTokens {
                single_incentive_staking: true,
                dual_incentives_staking: true,
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Invalid values provided");

    err = app
        .execute_contract(
            Addr::unchecked(auction_init_msg.owner.clone()),
            auction_instance.clone(),
            &ExecuteMsg::StakeLpTokens {
                single_incentive_staking: false,
                dual_incentives_staking: false,
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Invalid values provided");

    // ######    ERROR :: LP Tokens already staked with MARS LP Staking contract   ######

    err = app
        .execute_contract(
            Addr::unchecked(auction_init_msg.owner.clone()),
            auction_instance.clone(),
            &ExecuteMsg::StakeLpTokens {
                single_incentive_staking: true,
                dual_incentives_staking: false,
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: LP Tokens already staked with MARS LP Staking contract"
    );

    // ********
    // ******** SUCCESSFULLY UNSTAKED FROM LP STAKING CONTRACT AND STAKED WITH GENERATOR ********
    // ********

    // Instantiate Generator Proxy for MARS
    let generator_proxy_to_mars_contract = Box::new(ContractWrapper::new(
        generator_proxy_to_mars::contract::execute,
        generator_proxy_to_mars::contract::instantiate,
        generator_proxy_to_mars::contract::query,
    ));
    let generator_proxy_to_mars_code_id = app.store_code(generator_proxy_to_mars_contract);

    let generator_proxy_to_mars_instance = app
        .instantiate_contract(
            generator_proxy_to_mars_code_id,
            Addr::unchecked(auction_init_msg.owner.clone()),
            &astroport_generator_proxy::generator_proxy::InstantiateMsg {
                generator_contract_addr: generator_instance.clone().to_string(),
                pair_addr: pool_instance.clone().to_string(),
                lp_token_addr: lp_token_instance.clone().to_string(),
                reward_contract_addr: lp_staking_instance.clone().to_string(),
                reward_token_addr: mars_token_instance.clone().to_string(),
            },
            &[],
            String::from("MARS"),
            None,
        )
        .unwrap();

    // Set RewardProxyForMars with Generator
    app.execute_contract(
        Addr::unchecked(auction_init_msg.owner.clone()),
        generator_instance.clone(),
        &astroport::generator::ExecuteMsg::SetAllowedRewardProxies {
            proxies: vec![generator_proxy_to_mars_instance.clone().to_string()],
        },
        &[],
    )
    .unwrap();

    // Set RewardProxyForMars with Generator
    let msg = astroport::generator::ExecuteMsg::Add {
        alloc_point: Uint64::from(10u64),
        reward_proxy: Some(generator_proxy_to_mars_instance.clone().to_string()),
        lp_token: lp_token_instance.clone(),
    };
    app.execute_contract(
        Addr::unchecked(auction_init_msg.owner.clone()),
        generator_instance.clone(),
        &msg,
        &[],
    )
    .unwrap();

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(24_001_50)
    });

    // Unstake from LP Staking and stake with Generator
    app.execute_contract(
        Addr::unchecked(auction_init_msg.owner.clone()),
        auction_instance.clone(),
        &ExecuteMsg::StakeLpTokens {
            single_incentive_staking: false,
            dual_incentives_staking: true,
        },
        &[],
    )
    .unwrap();

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(24_002_50)
    });

    // Check state
    let state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::from(98u64), state_resp.lp_shares_withdrawn);
    assert_eq!(false, state_resp.are_staked_for_single_incentives);
    assert_eq!(true, state_resp.are_staked_for_dual_incentives);

    // Check user response
    let user_resp_before_claim: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: user1_address.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(98u64),
        user_resp_before_claim.withdrawn_lp_shares
    );
    assert_eq!(
        Uint128::from(196u64),
        user_resp_before_claim.withdrawable_lp_shares
    );
    assert_eq!(
        Uint128::from(1274379548u64),
        user_resp_before_claim.withdrawn_auction_incentives
    );
    assert_eq!(
        Uint128::from(2548759098u64),
        user_resp_before_claim.withdrawable_auction_incentives
    );
    assert_eq!(
        Uint128::from(44362u64),
        user_resp_before_claim.withdrawable_mars_incentives
    );
    assert_eq!(
        Uint128::from(16635u64),
        user_resp_before_claim.withdrawn_mars_incentives
    );
    assert_eq!(
        Uint128::from(41395360476u64),
        user_resp_before_claim.withdrawable_astro_incentives
    );
    assert_eq!(
        Uint128::from(0u64),
        user_resp_before_claim.withdrawn_astro_incentives
    );

    // ********
    // ******** USER SUCCESSFULLY CLAIMS DUAL REWARDS WITHDRAWS UNLOCKED LP SHARES ********
    // ********

    app.execute_contract(
        user1_address.clone(),
        auction_instance.clone(),
        &ExecuteMsg::ClaimRewards {
            withdraw_unlocked_shares: true,
        },
        &[],
    )
    .unwrap();

    // Check state
    let state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::from(294u64), state_resp.lp_shares_withdrawn);

    // Check user response
    let user_resp_after_claim: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: user1_address.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(0u64),
        user_resp_after_claim.withdrawable_lp_shares
    );
    assert_eq!(
        user_resp_before_claim.withdrawable_lp_shares + user_resp_before_claim.withdrawn_lp_shares,
        user_resp_after_claim.withdrawn_lp_shares
    );
    assert_eq!(
        Uint128::from(0u64),
        user_resp_after_claim.withdrawable_auction_incentives
    );
    assert_eq!(
        user_resp_before_claim.withdrawable_auction_incentives
            + user_resp_before_claim.withdrawn_auction_incentives,
        user_resp_after_claim.withdrawn_auction_incentives
    );
    assert_eq!(
        Uint128::from(0u64),
        user_resp_after_claim.withdrawable_mars_incentives
    );
    assert_eq!(
        user_resp_before_claim.withdrawable_mars_incentives
            + user_resp_before_claim.withdrawn_mars_incentives,
        user_resp_after_claim.withdrawn_mars_incentives
    );
    assert_eq!(
        Uint128::from(41395360476u64),
        user_resp_after_claim.withdrawn_astro_incentives
    );
    assert_eq!(
        user_resp_before_claim.withdrawable_astro_incentives
            + user_resp_before_claim.withdrawn_astro_incentives,
        user_resp_after_claim.withdrawn_astro_incentives
    );
    assert_eq!(
        Uint128::zero(),
        user_resp_after_claim.withdrawable_astro_incentives
    );
}
