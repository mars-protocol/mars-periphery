// use crate::contract::{
//     accumulate_prices, assert_max_spread, execute, get_fee_info, instantiate, query_pair_info,
//     query_pool, query_reverse_simulation, query_share, query_simulation,
// };
// use crate::error::ContractError;
// use crate::mock_querier::mock_dependencies;

// use crate::state::Config;
// use astroport::asset::{Asset, AssetInfo, PairInfo};
// use astroport::factory::PairType;
// use astroport::hook::InitHook;
// use astroport::pair::{
//     Cw20HookMsg, ExecuteMsg, InstantiateMsg, PoolResponse, ReverseSimulationResponse,
//     SimulationResponse,
// };
// use astroport::token::InstantiateMsg as TokenInstantiateMsg;
// use cosmwasm_bignumber::Decimal256;
// use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
// use cosmwasm_std::{
//     attr, to_binary, Addr, BankMsg, BlockInfo, Coin, CosmosMsg, Decimal, Env, ReplyOn, Response,
//     StdError, SubMsg, Timestamp, Uint128, WasmMsg,
// };
// use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, MinterResponse};
// use std::str::FromStr;

// #[test]
// fn test_get_fee_info() {
//     let deps = mock_dependencies(&[]);
//     let fee_info = get_fee_info(
//         deps.as_ref(),
//         Config {
//             pair_info: PairInfo {
//                 asset_infos: [
//                     AssetInfo::NativeToken {
//                         denom: "uusd".to_string(),
//                     },
//                     AssetInfo::Token {
//                         contract_addr: Addr::unchecked("asset0000"),
//                     },
//                 ],
//                 contract_addr: Addr::unchecked("contract"),
//                 liquidity_token: Addr::unchecked("token"),
//                 pair_type: PairType::Xyk {},
//             },
//             factory_addr: Addr::unchecked("factory"),
//             block_time_last: 0,
//             price0_cumulative_last: Default::default(),
//             price1_cumulative_last: Default::default(),
//         },
//     )
//     .unwrap();

//     assert_eq!(fee_info.total_fee_rate, Decimal::from_str("0.003").unwrap());
//     assert_eq!(fee_info.maker_fee_rate, Decimal::from_str("0.166").unwrap());
// }

// #[test]
// fn proper_initialization() {
//     let mut deps = mock_dependencies(&[]);

//     let msg = InstantiateMsg {
//         factory_addr: Addr::unchecked("factory"),
//         asset_infos: [
//             AssetInfo::NativeToken {
//                 denom: "uusd".to_string(),
//             },
//             AssetInfo::Token {
//                 contract_addr: Addr::unchecked("asset0000"),
//             },
//         ],
//         token_code_id: 10u64,
//         init_hook: Some(InitHook {
//             contract_addr: String::from("factory0000"),
//             msg: to_binary(&Uint128::new(1000000u128)).unwrap(),
//         }),
//         pair_type: PairType::Xyk {},
//     };

//     let sender = "addr0000";
//     // we can just call .unwrap() to assert this was a success
//     let env = mock_env();
//     let info = mock_info(sender, &[]);
//     let res = instantiate(deps.as_mut(), env, info, msg).unwrap();
//     assert_eq!(
//         res.messages,
//         vec![
//             SubMsg {
//                 msg: WasmMsg::Instantiate {
//                     code_id: 10u64,
//                     msg: to_binary(&TokenInstantiateMsg {
//                         name: "Astroport LP token".to_string(),
//                         symbol: "uLP".to_string(),
//                         decimals: 6,
//                         initial_balances: vec![],
//                         mint: Some(MinterResponse {
//                             minter: String::from(MOCK_CONTRACT_ADDR),
//                             cap: None,
//                         }),
//                         init_hook: Some(InitHook {
//                             msg: to_binary(&ExecuteMsg::PostInitialize {}).unwrap(),
//                             contract_addr: String::from(MOCK_CONTRACT_ADDR),
//                         }),
//                     })
//                     .unwrap(),
//                     funds: vec![],
//                     admin: Some(sender.to_string()),
//                     label: String::from("Astroport LP token"),
//                 }
//                 .into(),
//                 id: 0,
//                 gas_limit: None,
//                 reply_on: ReplyOn::Never
//             },
//             SubMsg {
//                 msg: WasmMsg::Execute {
//                     contract_addr: String::from("factory0000"),
//                     msg: to_binary(&Uint128::new(1000000u128)).unwrap(),
//                     funds: vec![],
//                 }
//                 .into(),
//                 id: 0,
//                 gas_limit: None,
//                 reply_on: ReplyOn::Never
//             }
//         ]
//     );

//     // post initalize
//     let msg = ExecuteMsg::PostInitialize {};
//     let env = mock_env();
//     let info = mock_info("liquidity0000", &[]);
//     let _res = execute(deps.as_mut(), env, info, msg).unwrap();

//     // cannot change it after post intialization
//     let msg = ExecuteMsg::PostInitialize {};
//     let env = mock_env();
//     let info = mock_info("liquidity0001", &[]);
//     let _res = execute(deps.as_mut(), env, info, msg).unwrap_err();

//     // // it worked, let's query the state
//     let pair_info: PairInfo = query_pair_info(deps.as_ref()).unwrap();
//     assert_eq!(Addr::unchecked("liquidity0000"), pair_info.liquidity_token);
//     assert_eq!(
//         pair_info.asset_infos,
//         [
//             AssetInfo::NativeToken {
//                 denom: "uusd".to_string(),
//             },
//             AssetInfo::Token {
//                 contract_addr: Addr::unchecked("asset0000")
//             }
//         ]
//     );
// }

// #[test]
// fn provide_liquidity() {
//     let mut deps = mock_dependencies(&[Coin {
//         denom: "uusd".to_string(),
//         amount: Uint128::new(200_000000000000000000u128),
//     }]);

//     deps.querier.with_token_balances(&[(
//         &String::from("liquidity0000"),
//         &[(&String::from(MOCK_CONTRACT_ADDR), &Uint128::new(0))],
//     )]);

//     let msg = InstantiateMsg {
//         asset_infos: [
//             AssetInfo::NativeToken {
//                 denom: "uusd".to_string(),
//             },
//             AssetInfo::Token {
//                 contract_addr: Addr::unchecked("asset0000"),
//             },
//         ],
//         token_code_id: 10u64,
//         init_hook: None,
//         factory_addr: Addr::unchecked("factory"),
//         pair_type: PairType::Xyk {},
//     };

//     let env = mock_env();
//     let info = mock_info("addr0000", &[]);
//     // we can just call .unwrap() to assert this was a success
//     let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

//     // post initalize
//     let msg = ExecuteMsg::PostInitialize {};
//     let env = mock_env();
//     let info = mock_info("liquidity0000", &[]);
//     let _res = execute(deps.as_mut(), env, info, msg).unwrap();

//     // successfully provide liquidity for the exist pool
//     let msg = ExecuteMsg::ProvideLiquidity {
//         assets: [
//             Asset {
//                 info: AssetInfo::Token {
//                     contract_addr: Addr::unchecked("asset0000"),
//                 },
//                 amount: Uint128::from(100_000000000000000000u128),
//             },
//             Asset {
//                 info: AssetInfo::NativeToken {
//                     denom: "uusd".to_string(),
//                 },
//                 amount: Uint128::from(100_000000000000000000u128),
//             },
//         ],
//         slippage_tolerance: None,
//         auto_stack: None,
//     };

//     let env = mock_env();
//     let info = mock_info(
//         "addr0000",
//         &[Coin {
//             denom: "uusd".to_string(),
//             amount: Uint128::from(100_000000000000000000u128),
//         }],
//     );
//     let res = execute(deps.as_mut(), env.clone().clone(), info, msg).unwrap();
//     let transfer_from_msg = res.messages.get(0).expect("no message");
//     let mint_msg = res.messages.get(1).expect("no message");
//     assert_eq!(
//         transfer_from_msg,
//         &SubMsg {
//             msg: WasmMsg::Execute {
//                 contract_addr: String::from("asset0000"),
//                 msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
//                     owner: String::from("addr0000"),
//                     recipient: String::from(MOCK_CONTRACT_ADDR),
//                     amount: Uint128::from(100_000000000000000000u128),
//                 })
//                 .unwrap(),
//                 funds: vec![],
//             }
//             .into(),
//             id: 0,
//             gas_limit: None,
//             reply_on: ReplyOn::Never
//         }
//     );
//     assert_eq!(
//         mint_msg,
//         &SubMsg {
//             msg: WasmMsg::Execute {
//                 contract_addr: String::from("liquidity0000"),
//                 msg: to_binary(&Cw20ExecuteMsg::Mint {
//                     recipient: String::from("addr0000"),
//                     amount: Uint128::from(100_000000000000000000u128),
//                 })
//                 .unwrap(),
//                 funds: vec![],
//             }
//             .into(),
//             id: 0,
//             gas_limit: None,
//             reply_on: ReplyOn::Never,
//         }
//     );

//     // provide more liquidity 1:2, which is not propotional to 1:1,
//     // then it must accept 1:1 and treat left amount as donation
//     deps.querier.with_balance(&[(
//         &String::from(MOCK_CONTRACT_ADDR),
//         &[Coin {
//             denom: "uusd".to_string(),
//             amount: Uint128::new(200_000000000000000000 + 200_000000000000000000 /* user deposit must be pre-applied */),
//         }],
//     )]);

//     deps.querier.with_token_balances(&[
//         (
//             &String::from("liquidity0000"),
//             &[(
//                 &String::from(MOCK_CONTRACT_ADDR),
//                 &Uint128::new(100_000000000000000000),
//             )],
//         ),
//         (
//             &String::from("asset0000"),
//             &[(
//                 &String::from(MOCK_CONTRACT_ADDR),
//                 &Uint128::new(200_000000000000000000),
//             )],
//         ),
//     ]);

//     let msg = ExecuteMsg::ProvideLiquidity {
//         assets: [
//             Asset {
//                 info: AssetInfo::Token {
//                     contract_addr: Addr::unchecked("asset0000"),
//                 },
//                 amount: Uint128::from(100_000000000000000000u128),
//             },
//             Asset {
//                 info: AssetInfo::NativeToken {
//                     denom: "uusd".to_string(),
//                 },
//                 amount: Uint128::from(200_000000000000000000u128),
//             },
//         ],
//         slippage_tolerance: None,
//         auto_stack: None,
//     };

//     let env = mock_env_with_block_time(env.block.time.seconds() + 1000);
//     let info = mock_info(
//         "addr0000",
//         &[Coin {
//             denom: "uusd".to_string(),
//             amount: Uint128::from(200_000000000000000000u128),
//         }],
//     );

//     // only accept 100, then 50 share will be generated with 100 * (100 / 200)
//     let res: Response = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
//     let transfer_from_msg = res.messages.get(0).expect("no message");
//     let mint_msg = res.messages.get(1).expect("no message");
//     assert_eq!(
//         transfer_from_msg,
//         &SubMsg {
//             msg: WasmMsg::Execute {
//                 contract_addr: String::from("asset0000"),
//                 msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
//                     owner: String::from("addr0000"),
//                     recipient: String::from(MOCK_CONTRACT_ADDR),
//                     amount: Uint128::from(100_000000000000000000u128),
//                 })
//                 .unwrap(),
//                 funds: vec![],
//             }
//             .into(),
//             id: 0,
//             gas_limit: None,
//             reply_on: ReplyOn::Never,
//         }
//     );
//     assert_eq!(
//         mint_msg,
//         &SubMsg {
//             msg: WasmMsg::Execute {
//                 contract_addr: String::from("liquidity0000"),
//                 msg: to_binary(&Cw20ExecuteMsg::Mint {
//                     recipient: String::from("addr0000"),
//                     amount: Uint128::from(50_000000000000000000u128),
//                 })
//                 .unwrap(),
//                 funds: vec![],
//             }
//             .into(),
//             id: 0,
//             gas_limit: None,
//             reply_on: ReplyOn::Never,
//         }
//     );

//     // check wrong argument
//     let msg = ExecuteMsg::ProvideLiquidity {
//         assets: [
//             Asset {
//                 info: AssetInfo::Token {
//                     contract_addr: Addr::unchecked("asset0000"),
//                 },
//                 amount: Uint128::from(100_000000000000000000u128),
//             },
//             Asset {
//                 info: AssetInfo::NativeToken {
//                     denom: "uusd".to_string(),
//                 },
//                 amount: Uint128::from(50_000000000000000000u128),
//             },
//         ],
//         slippage_tolerance: None,
//         auto_stack: None,
//     };

//     let env = mock_env();
//     let info = mock_info(
//         "addr0000",
//         &[Coin {
//             denom: "uusd".to_string(),
//             amount: Uint128::from(100_000000000000000000u128),
//         }],
//     );
//     let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
//     match res {
//         ContractError::Std(StdError::GenericErr { msg, .. }) => assert_eq!(
//             msg,
//             "Native token balance mismatch between the argument and the transferred".to_string()
//         ),
//         _ => panic!("Must return generic error"),
//     }

//     // initialize token balance to 1:1
//     deps.querier.with_balance(&[(
//         &String::from(MOCK_CONTRACT_ADDR),
//         &[Coin {
//             denom: "uusd".to_string(),
//             amount: Uint128::new(100_000000000000000000 + 100_000000000000000000 /* user deposit must be pre-applied */),
//         }],
//     )]);

//     deps.querier.with_token_balances(&[
//         (
//             &String::from("liquidity0000"),
//             &[(
//                 &String::from(MOCK_CONTRACT_ADDR),
//                 &Uint128::new(100_000000000000000000),
//             )],
//         ),
//         (
//             &String::from("asset0000"),
//             &[(
//                 &String::from(MOCK_CONTRACT_ADDR),
//                 &Uint128::new(100_000000000000000000),
//             )],
//         ),
//     ]);

//     // failed because the price is under slippage_tolerance
//     let msg = ExecuteMsg::ProvideLiquidity {
//         assets: [
//             Asset {
//                 info: AssetInfo::Token {
//                     contract_addr: Addr::unchecked("asset0000"),
//                 },
//                 amount: Uint128::from(98_000000000000000000u128),
//             },
//             Asset {
//                 info: AssetInfo::NativeToken {
//                     denom: "uusd".to_string(),
//                 },
//                 amount: Uint128::from(100_000000000000000000u128),
//             },
//         ],
//         slippage_tolerance: Some(Decimal::percent(1)),
//         auto_stack: None,
//     };

//     let env = mock_env_with_block_time(env.block.time.seconds() + 1000);
//     let info = mock_info(
//         "addr0001",
//         &[Coin {
//             denom: "uusd".to_string(),
//             amount: Uint128::from(100_000000000000000000u128),
//         }],
//     );
//     let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
//     assert_eq!(res, ContractError::MaxSlippageAssertion {});

//     // initialize token balance to 1:1
//     deps.querier.with_balance(&[(
//         &String::from(MOCK_CONTRACT_ADDR),
//         &[Coin {
//             denom: "uusd".to_string(),
//             amount: Uint128::new(100_000000000000000000 + 98_000000000000000000 /* user deposit must be pre-applied */),
//         }],
//     )]);

//     // failed because the price is under slippage_tolerance
//     let msg = ExecuteMsg::ProvideLiquidity {
//         assets: [
//             Asset {
//                 info: AssetInfo::Token {
//                     contract_addr: Addr::unchecked("asset0000"),
//                 },
//                 amount: Uint128::from(100_000000000000000000u128),
//             },
//             Asset {
//                 info: AssetInfo::NativeToken {
//                     denom: "uusd".to_string(),
//                 },
//                 amount: Uint128::from(98_000000000000000000u128),
//             },
//         ],
//         slippage_tolerance: Some(Decimal::percent(1)),
//         auto_stack: None,
//     };

//     let env = mock_env_with_block_time(env.block.time.seconds() + 1000);
//     let info = mock_info(
//         "addr0001",
//         &[Coin {
//             denom: "uusd".to_string(),
//             amount: Uint128::from(98_000000000000000000u128),
//         }],
//     );
//     let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
//     assert_eq!(res, ContractError::MaxSlippageAssertion {});

//     // initialize token balance to 1:1
//     deps.querier.with_balance(&[(
//         &String::from(MOCK_CONTRACT_ADDR),
//         &[Coin {
//             denom: "uusd".to_string(),
//             amount: Uint128::new(100_000000000000000000 + 100_000000000000000000 /* user deposit must be pre-applied */),
//         }],
//     )]);

//     // successfully provides
//     let msg = ExecuteMsg::ProvideLiquidity {
//         assets: [
//             Asset {
//                 info: AssetInfo::Token {
//                     contract_addr: Addr::unchecked("asset0000"),
//                 },
//                 amount: Uint128::from(99_000000000000000000u128),
//             },
//             Asset {
//                 info: AssetInfo::NativeToken {
//                     denom: "uusd".to_string(),
//                 },
//                 amount: Uint128::from(100_000000000000000000u128),
//             },
//         ],
//         slippage_tolerance: Some(Decimal::percent(1)),
//         auto_stack: None,
//     };

//     let env = mock_env_with_block_time(env.block.time.seconds() + 1000);
//     let info = mock_info(
//         "addr0001",
//         &[Coin {
//             denom: "uusd".to_string(),
//             amount: Uint128::from(100_000000000000000000u128),
//         }],
//     );
//     let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

//     // initialize token balance to 1:1
//     deps.querier.with_balance(&[(
//         &String::from(MOCK_CONTRACT_ADDR),
//         &[Coin {
//             denom: "uusd".to_string(),
//             amount: Uint128::new(100_000000000000000000 + 99_000000000000000000 /* user deposit must be pre-applied */),
//         }],
//     )]);

//     // successfully provides
//     let msg = ExecuteMsg::ProvideLiquidity {
//         assets: [
//             Asset {
//                 info: AssetInfo::Token {
//                     contract_addr: Addr::unchecked("asset0000"),
//                 },
//                 amount: Uint128::from(100_000000000000000000u128),
//             },
//             Asset {
//                 info: AssetInfo::NativeToken {
//                     denom: "uusd".to_string(),
//                 },
//                 amount: Uint128::from(99_000000000000000000u128),
//             },
//         ],
//         slippage_tolerance: Some(Decimal::percent(1)),
//         auto_stack: None,
//     };

//     let env = mock_env_with_block_time(env.block.time.seconds() + 1000);
//     let info = mock_info(
//         "addr0001",
//         &[Coin {
//             denom: "uusd".to_string(),
//             amount: Uint128::from(99_000000000000000000u128),
//         }],
//     );
//     let _res = execute(deps.as_mut(), env, info, msg).unwrap();
// }

// #[test]
// fn withdraw_liquidity() {
//     let mut deps = mock_dependencies(&[Coin {
//         denom: "uusd".to_string(),
//         amount: Uint128::new(100u128),
//     }]);

//     deps.querier.with_tax(
//         Decimal::zero(),
//         &[(&"uusd".to_string(), &Uint128::from(1000000u128))],
//     );
//     deps.querier.with_token_balances(&[
//         (
//             &String::from("liquidity0000"),
//             &[(&String::from("addr0000"), &Uint128::new(100u128))],
//         ),
//         (
//             &String::from("asset0000"),
//             &[(&String::from(MOCK_CONTRACT_ADDR), &Uint128::new(100u128))],
//         ),
//     ]);

//     let msg = InstantiateMsg {
//         asset_infos: [
//             AssetInfo::NativeToken {
//                 denom: "uusd".to_string(),
//             },
//             AssetInfo::Token {
//                 contract_addr: Addr::unchecked("asset0000"),
//             },
//         ],
//         token_code_id: 10u64,
//         init_hook: None,

//         factory_addr: Addr::unchecked("factory"),
//         pair_type: PairType::Xyk {},
//     };

//     let env = mock_env();
//     let info = mock_info("addr0000", &[]);
//     // we can just call .unwrap() to assert this was a success
//     let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

//     // post initalize
//     let msg = ExecuteMsg::PostInitialize {};
//     let env = mock_env();
//     let info = mock_info("liquidity0000", &[]);
//     let _res = execute(deps.as_mut(), env, info, msg).unwrap();

//     // withdraw liquidity
//     let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
//         sender: String::from("addr0000"),
//         msg: to_binary(&Cw20HookMsg::WithdrawLiquidity {}).unwrap(),
//         amount: Uint128::new(100u128),
//     });

//     let env = mock_env();
//     let info = mock_info("liquidity0000", &[]);
//     let res = execute(deps.as_mut(), env, info, msg).unwrap();
//     let log_withdrawn_share = res.attributes.get(1).expect("no log");
//     let log_refund_assets = res.attributes.get(2).expect("no log");
//     let msg_refund_0 = res.messages.get(0).expect("no message");
//     let msg_refund_1 = res.messages.get(1).expect("no message");
//     let msg_burn_liquidity = res.messages.get(2).expect("no message");
//     assert_eq!(
//         msg_refund_0,
//         &SubMsg {
//             msg: CosmosMsg::Bank(BankMsg::Send {
//                 to_address: String::from("addr0000"),
//                 amount: vec![Coin {
//                     denom: "uusd".to_string(),
//                     amount: Uint128::from(100u128),
//                 }],
//             }),
//             id: 0,
//             gas_limit: None,
//             reply_on: ReplyOn::Never,
//         }
//     );
//     assert_eq!(
//         msg_refund_1,
//         &SubMsg {
//             msg: WasmMsg::Execute {
//                 contract_addr: String::from("asset0000"),
//                 msg: to_binary(&Cw20ExecuteMsg::Transfer {
//                     recipient: String::from("addr0000"),
//                     amount: Uint128::from(100u128),
//                 })
//                 .unwrap(),
//                 funds: vec![],
//             }
//             .into(),
//             id: 0,
//             gas_limit: None,
//             reply_on: ReplyOn::Never,
//         }
//     );
//     assert_eq!(
//         msg_burn_liquidity,
//         &SubMsg {
//             msg: WasmMsg::Execute {
//                 contract_addr: String::from("liquidity0000"),
//                 msg: to_binary(&Cw20ExecuteMsg::Burn {
//                     amount: Uint128::from(100u128),
//                 })
//                 .unwrap(),
//                 funds: vec![],
//             }
//             .into(),
//             id: 0,
//             gas_limit: None,
//             reply_on: ReplyOn::Never,
//         }
//     );

//     assert_eq!(
//         log_withdrawn_share,
//         &attr("withdrawn_share", 100u128.to_string())
//     );
//     assert_eq!(
//         log_refund_assets,
//         &attr("refund_assets", "100uusd, 100asset0000")
//     );
// }

// #[test]
// fn try_native_to_token() {
//     let total_share = Uint128::new(30000000000u128);
//     let asset_pool_amount = Uint128::new(20000000000u128);
//     let collateral_pool_amount = Uint128::new(30000000000u128);
//     let price = Decimal::from_ratio(collateral_pool_amount, asset_pool_amount);
//     let exchange_rate = Decimal::from(Decimal256::one() / Decimal256::from(price));
//     let offer_amount = Uint128::new(1500000000u128);

//     let mut deps = mock_dependencies(&[Coin {
//         denom: "uusd".to_string(),
//         amount: collateral_pool_amount + offer_amount, /* user deposit must be pre-applied */
//     }]);

//     deps.querier.with_tax(
//         Decimal::zero(),
//         &[(&"uusd".to_string(), &Uint128::from(1000000u128))],
//     );

//     deps.querier.with_token_balances(&[
//         (
//             &String::from("liquidity0000"),
//             &[(&String::from(MOCK_CONTRACT_ADDR), &total_share)],
//         ),
//         (
//             &String::from("asset0000"),
//             &[(&String::from(MOCK_CONTRACT_ADDR), &asset_pool_amount)],
//         ),
//     ]);

//     let msg = InstantiateMsg {
//         asset_infos: [
//             AssetInfo::NativeToken {
//                 denom: "uusd".to_string(),
//             },
//             AssetInfo::Token {
//                 contract_addr: Addr::unchecked("asset0000"),
//             },
//         ],
//         token_code_id: 10u64,
//         init_hook: None,
//         factory_addr: Addr::unchecked("factory"),
//         pair_type: PairType::Xyk {},
//     };

//     let env = mock_env();
//     let info = mock_info("addr0000", &[]);
//     // we can just call .unwrap() to assert this was a success
//     let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

//     // post initalize
//     let msg = ExecuteMsg::PostInitialize {};
//     let env = mock_env();
//     let info = mock_info("liquidity0000", &[]);
//     let _res = execute(deps.as_mut(), env, info, msg).unwrap();

//     // normal swap
//     let msg = ExecuteMsg::Swap {
//         offer_asset: Asset {
//             info: AssetInfo::NativeToken {
//                 denom: "uusd".to_string(),
//             },
//             amount: offer_amount,
//         },
//         belief_price: None,
//         max_spread: None,
//         to: None,
//     };
//     let env = mock_env_with_block_time(1000);
//     let info = mock_info(
//         "addr0000",
//         &[Coin {
//             denom: "uusd".to_string(),
//             amount: offer_amount,
//         }],
//     );

//     let res = execute(deps.as_mut(), env, info, msg).unwrap();
//     let msg_transfer = res.messages.get(0).expect("no message");

//     // current price is 1.5, so expected return without spread is 1000
//     // 952.380953 = 20000 - 20000 * 30000 / (30000 + 1500)
//     let expected_ret_amount = Uint128::new(952_380_953u128);
//     let expected_spread_amount = (offer_amount * exchange_rate)
//         .checked_sub(expected_ret_amount)
//         .unwrap();
//     let expected_commission_amount = expected_ret_amount.multiply_ratio(3u128, 1000u128); // 0.3%
//     let expected_maker_fee_amount = expected_commission_amount.multiply_ratio(166u128, 1000u128); // 0.166

//     let expected_return_amount = expected_ret_amount
//         .checked_sub(expected_commission_amount)
//         .unwrap();
//     let expected_tax_amount = Uint128::zero(); // no tax for token

//     // check simulation res
//     deps.querier.with_balance(&[(
//         &String::from(MOCK_CONTRACT_ADDR),
//         &[Coin {
//             denom: "uusd".to_string(),
//             amount: collateral_pool_amount, /* user deposit must be pre-applied */
//         }],
//     )]);

//     let simulation_res: SimulationResponse = query_simulation(
//         deps.as_ref(),
//         Asset {
//             info: AssetInfo::NativeToken {
//                 denom: "uusd".to_string(),
//             },
//             amount: offer_amount,
//         },
//     )
//     .unwrap();
//     assert_eq!(expected_return_amount, simulation_res.return_amount);
//     assert_eq!(expected_commission_amount, simulation_res.commission_amount);
//     assert_eq!(expected_spread_amount, simulation_res.spread_amount);

//     // check reverse simulation res
//     let reverse_simulation_res: ReverseSimulationResponse = query_reverse_simulation(
//         deps.as_ref(),
//         Asset {
//             info: AssetInfo::Token {
//                 contract_addr: Addr::unchecked("asset0000"),
//             },
//             amount: expected_return_amount,
//         },
//     )
//     .unwrap();
//     assert_eq!(
//         (offer_amount.u128() as i128 - reverse_simulation_res.offer_amount.u128() as i128).abs()
//             < 5i128,
//         true
//     );
//     assert_eq!(
//         (expected_commission_amount.u128() as i128
//             - reverse_simulation_res.commission_amount.u128() as i128)
//             .abs()
//             < 5i128,
//         true
//     );
//     assert_eq!(
//         (expected_spread_amount.u128() as i128
//             - reverse_simulation_res.spread_amount.u128() as i128)
//             .abs()
//             < 5i128,
//         true
//     );

//     assert_eq!(
//         res.attributes,
//         vec![
//             attr("action", "swap"),
//             attr("offer_asset", "uusd"),
//             attr("ask_asset", "asset0000"),
//             attr("offer_amount", offer_amount.to_string()),
//             attr("return_amount", expected_return_amount.to_string()),
//             attr("tax_amount", expected_tax_amount.to_string()),
//             attr("spread_amount", expected_spread_amount.to_string()),
//             attr("commission_amount", expected_commission_amount.to_string()),
//             attr("maker_fee_amount", expected_maker_fee_amount.to_string()),
//         ]
//     );

//     assert_eq!(
//         &SubMsg {
//             msg: WasmMsg::Execute {
//                 contract_addr: String::from("asset0000"),
//                 msg: to_binary(&Cw20ExecuteMsg::Transfer {
//                     recipient: String::from("addr0000"),
//                     amount: Uint128::from(expected_return_amount),
//                 })
//                 .unwrap(),
//                 funds: vec![],
//             }
//             .into(),
//             id: 0,
//             gas_limit: None,
//             reply_on: ReplyOn::Never,
//         },
//         msg_transfer,
//     );
// }

// #[test]
// fn try_token_to_native() {
//     let total_share = Uint128::new(20000000000u128);
//     let asset_pool_amount = Uint128::new(30000000000u128);
//     let collateral_pool_amount = Uint128::new(20000000000u128);
//     let price = Decimal::from_ratio(collateral_pool_amount, asset_pool_amount);
//     let exchange_rate = price;
//     let offer_amount = Uint128::new(1500000000u128);

//     let mut deps = mock_dependencies(&[Coin {
//         denom: "uusd".to_string(),
//         amount: collateral_pool_amount,
//     }]);
//     deps.querier.with_tax(
//         Decimal::percent(1),
//         &[(&"uusd".to_string(), &Uint128::from(1000000u128))],
//     );
//     deps.querier.with_token_balances(&[
//         (
//             &String::from("liquidity0000"),
//             &[(&String::from(MOCK_CONTRACT_ADDR), &total_share)],
//         ),
//         (
//             &String::from("asset0000"),
//             &[(
//                 &String::from(MOCK_CONTRACT_ADDR),
//                 &(asset_pool_amount + offer_amount),
//             )],
//         ),
//     ]);

//     let msg = InstantiateMsg {
//         asset_infos: [
//             AssetInfo::NativeToken {
//                 denom: "uusd".to_string(),
//             },
//             AssetInfo::Token {
//                 contract_addr: Addr::unchecked("asset0000"),
//             },
//         ],
//         token_code_id: 10u64,
//         init_hook: None,
//         factory_addr: Addr::unchecked("factory"),
//         pair_type: PairType::Xyk {},
//     };

//     let env = mock_env();
//     let info = mock_info("addr0000", &[]);
//     // we can just call .unwrap() to assert this was a success
//     let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

//     // post initalize
//     let msg = ExecuteMsg::PostInitialize {};
//     let env = mock_env();
//     let info = mock_info("liquidity0000", &[]);
//     let _res = execute(deps.as_mut(), env, info, msg).unwrap();

//     // unauthorized access; can not execute swap directy for token swap
//     let msg = ExecuteMsg::Swap {
//         offer_asset: Asset {
//             info: AssetInfo::Token {
//                 contract_addr: Addr::unchecked("asset0000"),
//             },
//             amount: offer_amount,
//         },
//         belief_price: None,
//         max_spread: None,
//         to: None,
//     };
//     let env = mock_env_with_block_time(1000);
//     let info = mock_info("addr0000", &[]);
//     let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
//     assert_eq!(res, ContractError::Unauthorized {});

//     // normal sell
//     let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
//         sender: String::from("addr0000"),
//         amount: offer_amount,
//         msg: to_binary(&Cw20HookMsg::Swap {
//             belief_price: None,
//             max_spread: None,
//             to: None,
//         })
//         .unwrap(),
//     });
//     let env = mock_env_with_block_time(1000);
//     let info = mock_info("asset0000", &[]);

//     let res = execute(deps.as_mut(), env, info, msg).unwrap();
//     let msg_transfer = res.messages.get(0).expect("no message");

//     // current price is 1.5, so expected return without spread is 1000
//     // 952.380953 = 20000 - 20000 * 30000 / (30000 + 1500)
//     let expected_ret_amount = Uint128::new(952_380_953u128);
//     let expected_spread_amount = (offer_amount * exchange_rate)
//         .checked_sub(expected_ret_amount)
//         .unwrap();
//     let expected_commission_amount = expected_ret_amount.multiply_ratio(3u128, 1000u128); // 0.3%
//     let expected_maker_fee_amount = expected_commission_amount.multiply_ratio(166u128, 1000u128);
//     let expected_return_amount = expected_ret_amount
//         .checked_sub(expected_commission_amount)
//         .unwrap();
//     let expected_tax_amount = std::cmp::min(
//         Uint128::new(1000000u128),
//         expected_return_amount
//             .checked_sub(
//                 expected_return_amount.multiply_ratio(Uint128::new(100u128), Uint128::new(101u128)),
//             )
//             .unwrap(),
//     );
//     // check simulation res
//     // return asset token balance as normal
//     deps.querier.with_token_balances(&[
//         (
//             &String::from("liquidity0000"),
//             &[(&String::from(MOCK_CONTRACT_ADDR), &total_share)],
//         ),
//         (
//             &String::from("asset0000"),
//             &[(&String::from(MOCK_CONTRACT_ADDR), &(asset_pool_amount))],
//         ),
//     ]);

//     let simulation_res: SimulationResponse = query_simulation(
//         deps.as_ref(),
//         Asset {
//             amount: offer_amount,
//             info: AssetInfo::Token {
//                 contract_addr: Addr::unchecked("asset0000"),
//             },
//         },
//     )
//     .unwrap();
//     assert_eq!(expected_return_amount, simulation_res.return_amount);
//     assert_eq!(expected_commission_amount, simulation_res.commission_amount);
//     assert_eq!(expected_spread_amount, simulation_res.spread_amount);

//     // check reverse simulation res
//     let reverse_simulation_res: ReverseSimulationResponse = query_reverse_simulation(
//         deps.as_ref(),
//         Asset {
//             amount: expected_return_amount,
//             info: AssetInfo::NativeToken {
//                 denom: "uusd".to_string(),
//             },
//         },
//     )
//     .unwrap();
//     assert_eq!(
//         (offer_amount.u128() as i128 - reverse_simulation_res.offer_amount.u128() as i128).abs()
//             < 5i128,
//         true
//     );
//     assert_eq!(
//         (expected_commission_amount.u128() as i128
//             - reverse_simulation_res.commission_amount.u128() as i128)
//             .abs()
//             < 5i128,
//         true
//     );
//     assert_eq!(
//         (expected_spread_amount.u128() as i128
//             - reverse_simulation_res.spread_amount.u128() as i128)
//             .abs()
//             < 5i128,
//         true
//     );

//     assert_eq!(
//         res.attributes,
//         vec![
//             attr("action", "swap"),
//             attr("offer_asset", "asset0000"),
//             attr("ask_asset", "uusd"),
//             attr("offer_amount", offer_amount.to_string()),
//             attr("return_amount", expected_return_amount.to_string()),
//             attr("tax_amount", expected_tax_amount.to_string()),
//             attr("spread_amount", expected_spread_amount.to_string()),
//             attr("commission_amount", expected_commission_amount.to_string()),
//             attr("maker_fee_amount", expected_maker_fee_amount.to_string()),
//         ]
//     );

//     assert_eq!(
//         &SubMsg {
//             msg: CosmosMsg::Bank(BankMsg::Send {
//                 to_address: String::from("addr0000"),
//                 amount: vec![Coin {
//                     denom: "uusd".to_string(),
//                     amount: expected_return_amount
//                         .checked_sub(expected_tax_amount)
//                         .unwrap(),
//                 }],
//             })
//             .into(),
//             id: 0,
//             gas_limit: None,
//             reply_on: ReplyOn::Never,
//         },
//         msg_transfer,
//     );

//     // failed due to non asset token contract try to execute sell
//     let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
//         sender: String::from("addr0000"),
//         amount: offer_amount,
//         msg: to_binary(&Cw20HookMsg::Swap {
//             belief_price: None,
//             max_spread: None,
//             to: None,
//         })
//         .unwrap(),
//     });
//     let env = mock_env_with_block_time(1000);
//     let info = mock_info("liquidtity0000", &[]);
//     let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
//     assert_eq!(res, ContractError::Unauthorized {});
// }

// #[test]
// fn test_max_spread() {
//     assert_max_spread(
//         Some(Decimal::from_ratio(1200u128, 1u128)),
//         Some(Decimal::percent(1)),
//         Uint128::from(1200000000u128),
//         Uint128::from(989999u128),
//         Uint128::zero(),
//     )
//     .unwrap_err();

//     assert_max_spread(
//         Some(Decimal::from_ratio(1200u128, 1u128)),
//         Some(Decimal::percent(1)),
//         Uint128::from(1200000000u128),
//         Uint128::from(990000u128),
//         Uint128::zero(),
//     )
//     .unwrap();

//     assert_max_spread(
//         None,
//         Some(Decimal::percent(1)),
//         Uint128::zero(),
//         Uint128::from(989999u128),
//         Uint128::from(10001u128),
//     )
//     .unwrap_err();

//     assert_max_spread(
//         None,
//         Some(Decimal::percent(1)),
//         Uint128::zero(),
//         Uint128::from(990000u128),
//         Uint128::from(10000u128),
//     )
//     .unwrap();
// }

// #[test]
// fn test_deduct() {
//     let mut deps = mock_dependencies(&[]);

//     let tax_rate = Decimal::percent(2);
//     let tax_cap = Uint128::from(1_000_000u128);
//     deps.querier.with_tax(
//         Decimal::percent(2),
//         &[(&"uusd".to_string(), &Uint128::from(1000000u128))],
//     );

//     let amount = Uint128::new(1000_000_000u128);
//     let expected_after_amount = std::cmp::max(
//         amount.checked_sub(amount * tax_rate).unwrap(),
//         amount.checked_sub(tax_cap).unwrap(),
//     );

//     let after_amount = (Asset {
//         info: AssetInfo::NativeToken {
//             denom: "uusd".to_string(),
//         },
//         amount,
//     })
//     .deduct_tax(&deps.as_ref().querier)
//     .unwrap();

//     assert_eq!(expected_after_amount, after_amount.amount);
// }

// #[test]
// fn test_query_pool() {
//     let total_share_amount = Uint128::from(111u128);
//     let asset_0_amount = Uint128::from(222u128);
//     let asset_1_amount = Uint128::from(333u128);
//     let mut deps = mock_dependencies(&[Coin {
//         denom: "uusd".to_string(),
//         amount: asset_0_amount,
//     }]);

//     deps.querier.with_token_balances(&[
//         (
//             &String::from("asset0000"),
//             &[(&String::from(MOCK_CONTRACT_ADDR), &asset_1_amount)],
//         ),
//         (
//             &String::from("liquidity0000"),
//             &[(&String::from(MOCK_CONTRACT_ADDR), &total_share_amount)],
//         ),
//     ]);

//     let msg = InstantiateMsg {
//         asset_infos: [
//             AssetInfo::NativeToken {
//                 denom: "uusd".to_string(),
//             },
//             AssetInfo::Token {
//                 contract_addr: Addr::unchecked("asset0000"),
//             },
//         ],
//         token_code_id: 10u64,
//         init_hook: None,
//         factory_addr: Addr::unchecked("factory"),
//         pair_type: PairType::Xyk {},
//     };

//     let env = mock_env();
//     let info = mock_info("addr0000", &[]);
//     // we can just call .unwrap() to assert this was a success
//     let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

//     // post initalize
//     let msg = ExecuteMsg::PostInitialize {};
//     let env = mock_env();
//     let info = mock_info("liquidity0000", &[]);
//     let _res = execute(deps.as_mut(), env, info, msg).unwrap();

//     let res: PoolResponse = query_pool(deps.as_ref()).unwrap();

//     assert_eq!(
//         res.assets,
//         [
//             Asset {
//                 info: AssetInfo::NativeToken {
//                     denom: "uusd".to_string(),
//                 },
//                 amount: asset_0_amount
//             },
//             Asset {
//                 info: AssetInfo::Token {
//                     contract_addr: Addr::unchecked("asset0000"),
//                 },
//                 amount: asset_1_amount
//             }
//         ]
//     );
//     assert_eq!(res.total_share, total_share_amount);
// }

// #[test]
// fn test_query_share() {
//     let total_share_amount = Uint128::from(500u128);
//     let asset_0_amount = Uint128::from(250u128);
//     let asset_1_amount = Uint128::from(1000u128);
//     let mut deps = mock_dependencies(&[Coin {
//         denom: "uusd".to_string(),
//         amount: asset_0_amount,
//     }]);

//     deps.querier.with_token_balances(&[
//         (
//             &String::from("asset0000"),
//             &[(&String::from(MOCK_CONTRACT_ADDR), &asset_1_amount)],
//         ),
//         (
//             &String::from("liquidity0000"),
//             &[(&String::from(MOCK_CONTRACT_ADDR), &total_share_amount)],
//         ),
//     ]);

//     let msg = InstantiateMsg {
//         asset_infos: [
//             AssetInfo::NativeToken {
//                 denom: "uusd".to_string(),
//             },
//             AssetInfo::Token {
//                 contract_addr: Addr::unchecked("asset0000"),
//             },
//         ],
//         token_code_id: 10u64,
//         init_hook: None,
//         factory_addr: Addr::unchecked("factory"),
//         pair_type: PairType::Xyk {},
//     };

//     let env = mock_env();
//     let info = mock_info("addr0000", &[]);
//     // we can just call .unwrap() to assert this was a success
//     let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

//     // post initalize
//     let msg = ExecuteMsg::PostInitialize {};
//     let env = mock_env();
//     let info = mock_info("liquidity0000", &[]);
//     let _res = execute(deps.as_mut(), env, info, msg).unwrap();

//     let res = query_share(deps.as_ref(), Uint128::new(250)).unwrap();

//     assert_eq!(res[0].amount, Uint128::new(125));
//     assert_eq!(res[1].amount, Uint128::new(500));
// }

// #[test]
// fn test_accumulate_prices() {
//     struct Case {
//         block_time: u64,
//         block_time_last: u64,
//         last0: u128,
//         last1: u128,
//         x_amount: u128,
//         y_amount: u128,
//     }

//     struct Result {
//         block_time_last: u64,
//         price_x: u128,
//         price_y: u128,
//         is_some: bool,
//     }

//     let test_cases: Vec<(Case, Result)> = vec![
//         (
//             Case {
//                 block_time: 1000,
//                 block_time_last: 0,
//                 last0: 0,
//                 last1: 0,
//                 x_amount: 250,
//                 y_amount: 500,
//             },
//             Result {
//                 block_time_last: 1000,
//                 price_x: 2000, // 500/250*1000
//                 price_y: 500,  // 250/500*1000
//                 is_some: true,
//             },
//         ),
//         // Same block height, no changes
//         (
//             Case {
//                 block_time: 1000,
//                 block_time_last: 1000,
//                 last0: 1,
//                 last1: 2,
//                 x_amount: 250,
//                 y_amount: 500,
//             },
//             Result {
//                 block_time_last: 1000,
//                 price_x: 1,
//                 price_y: 2,
//                 is_some: false,
//             },
//         ),
//         (
//             Case {
//                 block_time: 1500,
//                 block_time_last: 1000,
//                 last0: 500,
//                 last1: 2000,
//                 x_amount: 250,
//                 y_amount: 500,
//             },
//             Result {
//                 block_time_last: 1500,
//                 price_x: 1500, // 500 + (500/250*500)
//                 price_y: 2250, // 2000 + (250/500*500)
//                 is_some: true,
//             },
//         ),
//     ];

//     for test_case in test_cases {
//         let (case, result) = test_case;

//         let env = mock_env_with_block_time(case.block_time);
//         let config = accumulate_prices(
//             env,
//             &Config {
//                 pair_info: PairInfo {
//                     asset_infos: [
//                         AssetInfo::NativeToken {
//                             denom: "uusd".to_string(),
//                         },
//                         AssetInfo::Token {
//                             contract_addr: Addr::unchecked("asset0000"),
//                         },
//                     ],
//                     contract_addr: Addr::unchecked("pair"),
//                     liquidity_token: Addr::unchecked("lp_token"),
//                     pair_type: PairType::Xyk {}, // Implemented in mock querier
//                 },
//                 factory_addr: Addr::unchecked("factory"),
//                 block_time_last: case.block_time_last,
//                 price0_cumulative_last: Uint128::new(case.last0),
//                 price1_cumulative_last: Uint128::new(case.last1),
//             },
//             Uint128::new(case.x_amount),
//             Uint128::new(case.y_amount),
//         );

//         assert_eq!(result.is_some, config.is_some());

//         if let Some(config) = config {
//             assert_eq!(config.2, result.block_time_last);
//             assert_eq!(config.0, Uint128::new(result.price_x));
//             assert_eq!(config.1, Uint128::new(result.price_y));
//         }
//     }
// }

// fn mock_env_with_block_time(time: u64) -> Env {
//     let mut env = mock_env();
//     env.block = BlockInfo {
//         height: 1,
//         time: Timestamp::from_seconds(time),
//         chain_id: "columbus".to_string(),
//     };
//     env
// }
