use cosmwasm_std::{
    entry_point, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint128,
};
use cw2::set_contract_version;
use cw20::{BalanceResponse, Cw20Coin, Cw20ReceiveMsg};
use cw20_base::allowances::{
    execute_decrease_allowance, execute_increase_allowance, query_allowance,
};
use cw20_base::contract::{
    execute_update_marketing, execute_upload_logo, query_balance, query_download_logo,
    query_marketing_info, query_minter, query_token_info,
};
use cw20_base::enumerable::{query_all_accounts, query_all_allowances};
use cw20_base::state::{BALANCES, TOKEN_INFO};
use cw20_base::ContractError;

use mars_core::cw20_core::instantiate_token_info_and_marketing;

use crate::allowances::{execute_burn_from, execute_send_from, execute_transfer_from};
use crate::core;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::snapshots::{
    capture_balance_snapshot, capture_total_supply_snapshot, get_balance_snapshot_value_at,
    get_total_supply_snapshot_value_at,
};
use crate::TotalSupplyResponse;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:xmars-token";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    msg.validate()?;
    let total_supply = create_accounts(&mut deps, &env, &msg.initial_balances)?;

    if total_supply > Uint128::zero() {
        capture_total_supply_snapshot(deps.storage, &env, total_supply)?;
    }

    instantiate_token_info_and_marketing(&mut deps, msg, total_supply)?;

    Ok(Response::default())
}

pub fn create_accounts(deps: &mut DepsMut, env: &Env, accounts: &[Cw20Coin]) -> StdResult<Uint128> {
    let mut total_supply = Uint128::zero();
    for row in accounts {
        let address = deps.api.addr_validate(&row.address)?;
        BALANCES.save(deps.storage, &address, &row.amount)?;
        capture_balance_snapshot(deps.storage, env, &address, row.amount)?;
        total_supply += row.amount;
    }
    Ok(total_supply)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Transfer { recipient, amount } => {
            execute_transfer(deps, env, info, recipient, amount)
        }
        ExecuteMsg::Burn { amount } => execute_burn(deps, env, info, amount),
        ExecuteMsg::Send {
            contract,
            amount,
            msg,
        } => execute_send(deps, env, info, contract, amount, msg),
        ExecuteMsg::Mint { recipient, amount } => execute_mint(deps, env, info, recipient, amount),
        ExecuteMsg::IncreaseAllowance {
            spender,
            amount,
            expires,
        } => execute_increase_allowance(deps, env, info, spender, amount, expires),
        ExecuteMsg::DecreaseAllowance {
            spender,
            amount,
            expires,
        } => execute_decrease_allowance(deps, env, info, spender, amount, expires),
        ExecuteMsg::TransferFrom {
            owner,
            recipient,
            amount,
        } => execute_transfer_from(deps, env, info, owner, recipient, amount),
        ExecuteMsg::BurnFrom { owner, amount } => execute_burn_from(deps, env, info, owner, amount),
        ExecuteMsg::SendFrom {
            owner,
            contract,
            amount,
            msg,
        } => execute_send_from(deps, env, info, owner, contract, amount, msg),
        ExecuteMsg::UpdateMarketing {
            project,
            description,
            marketing,
        } => execute_update_marketing(deps, env, info, project, description, marketing),
        ExecuteMsg::UploadLogo(logo) => execute_upload_logo(deps, env, info, logo),
    }
}

pub fn execute_transfer(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let recipient_addr = deps.api.addr_validate(&recipient)?;

    core::transfer(
        deps.storage,
        &env,
        Some(&info.sender),
        Some(&recipient_addr),
        amount,
    )?;

    let res = Response::new()
        .add_attribute("action", "transfer")
        .add_attribute("from", info.sender)
        .add_attribute("to", recipient)
        .add_attribute("amount", amount);
    Ok(res)
}

pub fn execute_burn(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    core::burn(deps.storage, &env, &info.sender, amount)?;

    let res = Response::new()
        .add_attribute("action", "burn")
        .add_attribute("user", info.sender)
        .add_attribute("amount", amount);
    Ok(res)
}

pub fn execute_mint(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    if amount.is_zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    let mut config = TOKEN_INFO.load(deps.storage)?;
    if config.mint.is_none() || config.mint.as_ref().unwrap().minter != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // update supply and enforce cap
    config.total_supply += amount;
    if let Some(limit) = config.get_cap() {
        if config.total_supply > limit {
            return Err(ContractError::CannotExceedCap {});
        }
    }
    TOKEN_INFO.save(deps.storage, &config)?;
    capture_total_supply_snapshot(deps.storage, &env, config.total_supply)?;

    // add amount to recipient balance
    let rcpt_addr = deps.api.addr_validate(&recipient)?;
    core::transfer(deps.storage, &env, None, Some(&rcpt_addr), amount)?;

    let res = Response::new()
        .add_attribute("action", "mint")
        .add_attribute("to", recipient)
        .add_attribute("amount", amount);
    Ok(res)
}

pub fn execute_send(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    contract: String,
    amount: Uint128,
    msg: Binary,
) -> Result<Response, ContractError> {
    let rcpt_addr = deps.api.addr_validate(&contract)?;

    // move the tokens to the contract
    core::transfer(
        deps.storage,
        &env,
        Some(&info.sender),
        Some(&rcpt_addr),
        amount,
    )?;

    let res = Response::new()
        .add_attribute("action", "send")
        .add_attribute("from", info.sender.to_string())
        .add_attribute("to", &contract)
        .add_attribute("amount", amount)
        .add_message(
            Cw20ReceiveMsg {
                sender: info.sender.to_string(),
                amount,
                msg,
            }
            .into_cosmos_msg(contract)?,
        );

    Ok(res)
}

// QUERY

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Balance { address } => to_binary(&query_balance(deps, address)?),
        QueryMsg::BalanceAt { address, block } => {
            to_binary(&query_balance_at(deps, address, block)?)
        }
        QueryMsg::TokenInfo {} => to_binary(&query_token_info(deps)?),
        QueryMsg::TotalSupplyAt { block } => to_binary(&query_total_supply_at(deps, block)?),
        QueryMsg::Minter {} => to_binary(&query_minter(deps)?),
        QueryMsg::Allowance { owner, spender } => {
            to_binary(&query_allowance(deps, owner, spender)?)
        }
        QueryMsg::AllAllowances {
            owner,
            start_after,
            limit,
        } => to_binary(&query_all_allowances(deps, owner, start_after, limit)?),
        QueryMsg::AllAccounts { start_after, limit } => {
            to_binary(&query_all_accounts(deps, start_after, limit)?)
        }
        QueryMsg::MarketingInfo {} => to_binary(&query_marketing_info(deps)?),
        QueryMsg::DownloadLogo {} => to_binary(&query_download_logo(deps)?),
    }
}

pub fn query_balance_at(deps: Deps, address: String, block: u64) -> StdResult<BalanceResponse> {
    let addr = deps.api.addr_validate(&address)?;
    let balance = get_balance_snapshot_value_at(deps.storage, &addr, block)?;
    Ok(BalanceResponse { balance })
}

pub fn query_total_supply_at(deps: Deps, block: u64) -> StdResult<TotalSupplyResponse> {
    let total_supply = get_total_supply_snapshot_value_at(deps.storage, block)?;
    Ok(TotalSupplyResponse { total_supply })
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, Addr, CosmosMsg, StdError, SubMsg, WasmMsg};
    use cw20::{
        Cw20Coin, Logo, LogoInfo, MarketingInfoResponse, MinterResponse, TokenInfoResponse,
    };
    use cw20_base::msg::InstantiateMarketingInfo;

    use mars_core::testing::MockEnvParams;

    use super::*;

    fn get_balance<T: Into<String>>(deps: Deps, address: T) -> Uint128 {
        query_balance(deps, address.into()).unwrap().balance
    }

    // this will set up the instantiation for other tests
    fn do_instantiate_with_minter(
        deps: DepsMut,
        addr: &str,
        amount: Uint128,
        minter: &str,
        cap: Option<Uint128>,
    ) -> TokenInfoResponse {
        _do_instantiate(
            deps,
            addr,
            amount,
            Some(MinterResponse {
                minter: minter.to_string(),
                cap,
            }),
        )
    }

    // this will set up the instantiation for other tests
    fn do_instantiate(deps: DepsMut, addr: &str, amount: Uint128) -> TokenInfoResponse {
        _do_instantiate(deps, addr, amount, None)
    }

    // this will set up the instantiation for other tests
    fn _do_instantiate(
        mut deps: DepsMut,
        addr: &str,
        amount: Uint128,
        mint: Option<MinterResponse>,
    ) -> TokenInfoResponse {
        let instantiate_msg = InstantiateMsg {
            name: "Auto Gen".to_string(),
            symbol: "AUTO".to_string(),
            decimals: 3,
            initial_balances: vec![Cw20Coin {
                address: addr.to_string(),
                amount,
            }],
            mint: mint.clone(),
            marketing: None,
        };
        let info = mock_info("creator", &[]);
        let env = mock_env();
        let res = instantiate(deps.branch(), env, info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        let meta = query_token_info(deps.as_ref()).unwrap();
        assert_eq!(
            meta,
            TokenInfoResponse {
                name: "Auto Gen".to_string(),
                symbol: "AUTO".to_string(),
                decimals: 3,
                total_supply: amount,
            }
        );
        assert_eq!(get_balance(deps.as_ref(), addr), amount);
        assert_eq!(query_minter(deps.as_ref()).unwrap(), mint,);
        meta
    }

    mod instantiate {
        use super::*;

        #[test]
        fn basic() {
            let mut deps = mock_dependencies(&[]);
            let amount = Uint128::from(11223344u128);
            let instantiate_msg = InstantiateMsg {
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![Cw20Coin {
                    address: String::from("addr0000"),
                    amount,
                }],
                mint: None,
                marketing: None,
            };
            let info = mock_info("creator", &[]);
            let env = mock_env();
            let res = instantiate(deps.as_mut(), env, info, instantiate_msg).unwrap();
            assert_eq!(0, res.messages.len());

            assert_eq!(
                query_token_info(deps.as_ref()).unwrap(),
                TokenInfoResponse {
                    name: "Cash Token".to_string(),
                    symbol: "CASH".to_string(),
                    decimals: 9,
                    total_supply: amount,
                }
            );
            assert_eq!(
                get_balance(deps.as_ref(), "addr0000"),
                Uint128::new(11223344)
            );
        }

        #[test]
        fn mintable() {
            let mut deps = mock_dependencies(&[]);
            let amount = Uint128::new(11223344);
            let minter = String::from("asmodat");
            let limit = Uint128::new(511223344);
            let instantiate_msg = InstantiateMsg {
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![Cw20Coin {
                    address: "addr0000".into(),
                    amount,
                }],
                mint: Some(MinterResponse {
                    minter: minter.clone(),
                    cap: Some(limit),
                }),
                marketing: None,
            };
            let info = mock_info("creator", &[]);
            let env = mock_env();
            let res = instantiate(deps.as_mut(), env, info, instantiate_msg).unwrap();
            assert_eq!(0, res.messages.len());

            assert_eq!(
                query_token_info(deps.as_ref()).unwrap(),
                TokenInfoResponse {
                    name: "Cash Token".to_string(),
                    symbol: "CASH".to_string(),
                    decimals: 9,
                    total_supply: amount,
                }
            );
            assert_eq!(
                get_balance(deps.as_ref(), "addr0000"),
                Uint128::new(11223344)
            );
            assert_eq!(
                query_minter(deps.as_ref()).unwrap(),
                Some(MinterResponse {
                    minter,
                    cap: Some(limit),
                }),
            );
        }

        #[test]
        fn mintable_over_cap() {
            let mut deps = mock_dependencies(&[]);
            let amount = Uint128::new(11223344);
            let minter = String::from("asmodat");
            let limit = Uint128::new(11223300);
            let instantiate_msg = InstantiateMsg {
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![Cw20Coin {
                    address: String::from("addr0000"),
                    amount,
                }],
                mint: Some(MinterResponse {
                    minter,
                    cap: Some(limit),
                }),
                marketing: None,
            };
            let info = mock_info("creator", &[]);
            let env = mock_env();
            let err = instantiate(deps.as_mut(), env, info, instantiate_msg).unwrap_err();
            assert_eq!(
                err,
                StdError::generic_err("Initial supply greater than cap").into()
            );
        }

        mod marketing {
            use super::*;

            #[test]
            fn basic() {
                let mut deps = mock_dependencies(&[]);
                let instantiate_msg = InstantiateMsg {
                    name: "Cash Token".to_string(),
                    symbol: "CASH".to_string(),
                    decimals: 9,
                    initial_balances: vec![],
                    mint: None,
                    marketing: Some(InstantiateMarketingInfo {
                        project: Some("Project".to_owned()),
                        description: Some("Description".to_owned()),
                        marketing: Some("marketing".to_owned()),
                        logo: Some(Logo::Url("url".to_owned())),
                    }),
                };

                let info = mock_info("creator", &[]);
                let env = mock_env();
                let res = instantiate(deps.as_mut(), env, info, instantiate_msg).unwrap();
                assert_eq!(0, res.messages.len());

                assert_eq!(
                    query_marketing_info(deps.as_ref()).unwrap(),
                    MarketingInfoResponse {
                        project: Some("Project".to_owned()),
                        description: Some("Description".to_owned()),
                        marketing: Some(Addr::unchecked("marketing")),
                        logo: Some(LogoInfo::Url("url".to_owned())),
                    }
                );

                let err = query_download_logo(deps.as_ref()).unwrap_err();
                assert!(
                    matches!(err, StdError::NotFound { .. }),
                    "Expected StdError::NotFound, received {}",
                    err
                );
            }

            #[test]
            fn invalid_marketing() {
                let mut deps = mock_dependencies(&[]);
                let instantiate_msg = InstantiateMsg {
                    name: "Cash Token".to_string(),
                    symbol: "CASH".to_string(),
                    decimals: 9,
                    initial_balances: vec![],
                    mint: None,
                    marketing: Some(InstantiateMarketingInfo {
                        project: Some("Project".to_owned()),
                        description: Some("Description".to_owned()),
                        marketing: Some("m".to_owned()),
                        logo: Some(Logo::Url("url".to_owned())),
                    }),
                };

                let info = mock_info("creator", &[]);
                let env = mock_env();
                instantiate(deps.as_mut(), env, info, instantiate_msg).unwrap_err();

                let err = query_download_logo(deps.as_ref()).unwrap_err();
                assert!(
                    matches!(err, StdError::NotFound { .. }),
                    "Expected StdError::NotFound, received {}",
                    err
                );
            }
        }
    }

    #[test]
    fn can_mint_by_minter() {
        let mut deps = mock_dependencies(&[]);

        let genesis = String::from("genesis");
        let amount = Uint128::new(11223344);
        let minter = String::from("asmodat");
        let limit = Uint128::new(511223344);
        do_instantiate_with_minter(deps.as_mut(), &genesis, amount, &minter, Some(limit));

        // minter can mint coins to some winner
        let winner = String::from("lucky");
        let prize = Uint128::new(222_222_222);
        let msg = ExecuteMsg::Mint {
            recipient: winner.clone(),
            amount: prize,
        };

        let info = mock_info(minter.as_ref(), &[]);
        let env = mock_env();
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(0, res.messages.len());
        assert_eq!(get_balance(deps.as_ref(), genesis), amount);
        assert_eq!(get_balance(deps.as_ref(), winner.clone()), prize);

        // but cannot mint nothing
        let msg = ExecuteMsg::Mint {
            recipient: winner.clone(),
            amount: Uint128::zero(),
        };
        let info = mock_info(minter.as_ref(), &[]);
        let env = mock_env();
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::InvalidZeroAmount {});

        // but if it exceeds cap (even over multiple rounds), it fails
        // cap is enforced
        let msg = ExecuteMsg::Mint {
            recipient: winner,
            amount: Uint128::new(333_222_222),
        };
        let info = mock_info(minter.as_ref(), &[]);
        let env = mock_env();
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::CannotExceedCap {});
    }

    #[test]
    fn others_cannot_mint() {
        let mut deps = mock_dependencies(&[]);
        do_instantiate_with_minter(
            deps.as_mut(),
            &String::from("genesis"),
            Uint128::new(1234),
            &String::from("minter"),
            None,
        );

        let msg = ExecuteMsg::Mint {
            recipient: String::from("lucky"),
            amount: Uint128::new(222),
        };
        let info = mock_info("anyone else", &[]);
        let env = mock_env();
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});
    }

    #[test]
    fn no_one_mints_if_minter_unset() {
        let mut deps = mock_dependencies(&[]);
        do_instantiate(deps.as_mut(), &String::from("genesis"), Uint128::new(1234));

        let msg = ExecuteMsg::Mint {
            recipient: String::from("lucky"),
            amount: Uint128::new(222),
        };
        let info = mock_info("genesis", &[]);
        let env = mock_env();
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});
    }

    #[test]
    fn instantiate_multiple_accounts() {
        let mut deps = mock_dependencies(&[]);
        let amount1 = Uint128::from(11223344u128);
        let addr1 = String::from("addr0001");
        let amount2 = Uint128::from(7890987u128);
        let addr2 = String::from("addr0002");
        let instantiate_msg = InstantiateMsg {
            name: "Bash Shell".to_string(),
            symbol: "BASH".to_string(),
            decimals: 6,
            initial_balances: vec![
                Cw20Coin {
                    address: addr1.clone(),
                    amount: amount1,
                },
                Cw20Coin {
                    address: addr2.clone(),
                    amount: amount2,
                },
            ],
            mint: None,
            marketing: None,
        };
        let info = mock_info("creator", &[]);
        let env = mock_env();
        let res = instantiate(deps.as_mut(), env, info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        assert_eq!(
            query_token_info(deps.as_ref()).unwrap(),
            TokenInfoResponse {
                name: "Bash Shell".to_string(),
                symbol: "BASH".to_string(),
                decimals: 6,
                total_supply: amount1 + amount2,
            }
        );
        assert_eq!(get_balance(deps.as_ref(), addr1), amount1);
        assert_eq!(get_balance(deps.as_ref(), addr2), amount2);
    }

    #[test]
    fn transfer() {
        let mut deps = mock_dependencies(&coins(2, "token"));
        let addr1 = String::from("addr0001");
        let addr2 = String::from("addr0002");
        let amount1 = Uint128::from(12340000u128);
        let transfer = Uint128::from(76543u128);
        let too_much = Uint128::from(12340321u128);

        do_instantiate(deps.as_mut(), &addr1, amount1);

        // cannot transfer nothing
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Transfer {
            recipient: addr2.clone(),
            amount: Uint128::zero(),
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::InvalidZeroAmount {});

        // cannot send more than we have
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Transfer {
            recipient: addr2.clone(),
            amount: too_much,
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert!(matches!(err, ContractError::Std(StdError::Overflow { .. })));

        // cannot send from empty account
        let info = mock_info(addr2.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Transfer {
            recipient: addr1.clone(),
            amount: transfer,
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert!(matches!(err, ContractError::Std(StdError::Overflow { .. })));

        // valid transfer
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mars_core::testing::mock_env(MockEnvParams {
            block_height: 100_000,
            ..Default::default()
        });
        let msg = ExecuteMsg::Transfer {
            recipient: addr2.clone(),
            amount: transfer,
        };
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(res.messages.len(), 0);

        let remainder = amount1.checked_sub(transfer).unwrap();
        assert_eq!(get_balance(deps.as_ref(), addr1.clone()), remainder);
        assert_eq!(get_balance(deps.as_ref(), addr2.clone()), transfer);
        assert_eq!(
            query_balance_at(deps.as_ref(), addr1, 100_000)
                .unwrap()
                .balance,
            remainder
        );
        assert_eq!(
            query_balance_at(deps.as_ref(), addr2, 100_000)
                .unwrap()
                .balance,
            transfer
        );
        assert_eq!(
            query_token_info(deps.as_ref()).unwrap().total_supply,
            amount1
        );
    }

    #[test]
    fn burn() {
        let mut deps = mock_dependencies(&coins(2, "token"));
        let addr1 = String::from("addr0001");
        let amount1 = Uint128::from(12340000u128);
        let burn = Uint128::from(76543u128);
        let too_much = Uint128::from(12340321u128);

        do_instantiate(deps.as_mut(), &addr1, amount1);

        // cannot burn nothing
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Burn {
            amount: Uint128::zero(),
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::InvalidZeroAmount {});
        assert_eq!(
            query_token_info(deps.as_ref()).unwrap().total_supply,
            amount1
        );

        // cannot burn more than we have
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Burn { amount: too_much };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert!(matches!(err, ContractError::Std(StdError::Overflow { .. })));
        assert_eq!(
            query_token_info(deps.as_ref()).unwrap().total_supply,
            amount1
        );

        // valid burn reduces total supply
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mars_core::testing::mock_env(MockEnvParams {
            block_height: 200_000,
            ..Default::default()
        });
        let msg = ExecuteMsg::Burn { amount: burn };
        execute(deps.as_mut(), env, info, msg).unwrap();

        let remainder = amount1.checked_sub(burn).unwrap();
        assert_eq!(get_balance(deps.as_ref(), addr1.clone()), remainder);
        assert_eq!(
            query_balance_at(deps.as_ref(), addr1, 200_000)
                .unwrap()
                .balance,
            remainder
        );
        assert_eq!(
            query_token_info(deps.as_ref()).unwrap().total_supply,
            remainder
        );
        assert_eq!(
            query_total_supply_at(deps.as_ref(), 200_000)
                .unwrap()
                .total_supply,
            remainder
        );
    }

    #[test]
    fn send() {
        let mut deps = mock_dependencies(&coins(2, "token"));
        let addr1 = String::from("addr0001");
        let contract = String::from("addr0002");
        let amount1 = Uint128::from(12340000u128);
        let transfer = Uint128::from(76543u128);
        let too_much = Uint128::from(12340321u128);
        let send_msg = Binary::from(r#"{"some":123}"#.as_bytes());

        do_instantiate(deps.as_mut(), &addr1, amount1);

        // cannot send nothing
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Send {
            contract: contract.clone(),
            amount: Uint128::zero(),
            msg: send_msg.clone(),
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::InvalidZeroAmount {});

        // cannot send more than we have
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Send {
            contract: contract.clone(),
            amount: too_much,
            msg: send_msg.clone(),
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert!(matches!(err, ContractError::Std(StdError::Overflow { .. })));

        // valid transfer
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Send {
            contract: contract.clone(),
            amount: transfer,
            msg: send_msg.clone(),
        };
        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
        assert_eq!(res.messages.len(), 1);

        // ensure proper send message sent
        // this is the message we want delivered to the other side
        let binary_msg = Cw20ReceiveMsg {
            sender: addr1.clone(),
            amount: transfer,
            msg: send_msg,
        }
        .into_binary()
        .unwrap();
        // and this is how it must be wrapped for the vm to process it
        assert_eq!(
            res.messages[0],
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract.clone(),
                msg: binary_msg,
                funds: vec![],
            }))
        );

        // ensure balance is properly transferred
        let remainder = amount1.checked_sub(transfer).unwrap();
        assert_eq!(get_balance(deps.as_ref(), addr1.clone()), remainder);
        assert_eq!(get_balance(deps.as_ref(), contract.clone()), transfer);
        assert_eq!(
            query_token_info(deps.as_ref()).unwrap().total_supply,
            amount1
        );
        assert_eq!(
            query_balance_at(deps.as_ref(), addr1, env.block.height)
                .unwrap()
                .balance,
            remainder
        );
        assert_eq!(
            query_balance_at(deps.as_ref(), contract, env.block.height)
                .unwrap()
                .balance,
            transfer
        );
    }

    #[test]
    fn snapshots_are_taken_and_retrieved_correctly() {
        let mut deps = mock_dependencies(&[]);

        let addr1 = String::from("addr1");
        let addr2 = String::from("addr2");

        let mut current_total_supply = Uint128::new(100_000);
        let mut current_block = 12_345;
        let mut current_addr1_balance = current_total_supply;
        let mut current_addr2_balance = Uint128::zero();

        let minter = String::from("minter");
        do_instantiate_with_minter(deps.as_mut(), &addr1, current_total_supply, &minter, None);

        let mut expected_total_supplies = vec![(current_block, current_total_supply)];
        let mut expected_addr1_balances = vec![(current_block, current_addr1_balance)];
        let mut expected_addr2_balances: Vec<(u64, Uint128)> = vec![];

        // Mint to addr2 3 times
        for _i in 0..3 {
            current_block += 100_000;

            let mint_amount = Uint128::new(20_000);
            current_total_supply += mint_amount;
            current_addr2_balance += mint_amount;

            let info = mock_info(minter.as_str(), &[]);
            let env = mars_core::testing::mock_env(MockEnvParams {
                block_height: current_block,
                ..Default::default()
            });

            let msg = ExecuteMsg::Mint {
                recipient: addr2.clone(),
                amount: mint_amount,
            };

            execute(deps.as_mut(), env, info, msg).unwrap();

            expected_total_supplies.push((current_block, current_total_supply));
            expected_addr2_balances.push((current_block, current_addr2_balance));
        }

        // Transfer from addr1 to addr2 4 times
        for _i in 0..4 {
            current_block += 60_000;

            let transfer_amount = Uint128::new(10_000);
            current_addr1_balance = current_addr1_balance - transfer_amount;
            current_addr2_balance += transfer_amount;

            let info = mock_info(addr1.as_str(), &[]);
            let env = mars_core::testing::mock_env(MockEnvParams {
                block_height: current_block,
                ..Default::default()
            });

            let msg = ExecuteMsg::Transfer {
                recipient: addr2.clone(),
                amount: transfer_amount,
            };

            execute(deps.as_mut(), env, info, msg).unwrap();

            expected_addr1_balances.push((current_block, current_addr1_balance));
            expected_addr2_balances.push((current_block, current_addr2_balance));
        }

        // Burn from addr2 3 times
        for _i in 0..3 {
            current_block += 50_000;

            let burn_amount = Uint128::new(20_000);
            current_total_supply = current_total_supply - burn_amount;
            current_addr2_balance = current_addr2_balance - burn_amount;

            let info = mock_info(addr2.as_str(), &[]);

            let env = mars_core::testing::mock_env(MockEnvParams {
                block_height: current_block,
                ..Default::default()
            });

            let msg = ExecuteMsg::Burn {
                amount: burn_amount,
            };

            execute(deps.as_mut(), env, info, msg).unwrap();

            expected_total_supplies.push((current_block, current_total_supply));
            expected_addr2_balances.push((current_block, current_addr2_balance));
        }

        // Check total supplies;
        let mut total_supply_previous_value = Uint128::zero();
        for (block, expected_total_supply) in expected_total_supplies {
            // block before gives previous value
            assert_eq!(
                query_total_supply_at(deps.as_ref(), block - 1)
                    .unwrap()
                    .total_supply,
                total_supply_previous_value
            );

            // block gives expected value
            assert_eq!(
                query_total_supply_at(deps.as_ref(), block)
                    .unwrap()
                    .total_supply,
                expected_total_supply,
            );

            // block after still gives expected value
            assert_eq!(
                query_total_supply_at(deps.as_ref(), block + 10)
                    .unwrap()
                    .total_supply,
                expected_total_supply,
            );

            total_supply_previous_value = expected_total_supply;
        }

        // Check addr1 balances
        let mut balance_previous_value = Uint128::zero();
        for (block, expected_balance) in expected_addr1_balances {
            // block before gives previous value
            assert_eq!(
                query_balance_at(deps.as_ref(), addr1.clone(), block - 10)
                    .unwrap()
                    .balance,
                balance_previous_value
            );

            // block gives expected value
            assert_eq!(
                query_balance_at(deps.as_ref(), addr1.clone(), block)
                    .unwrap()
                    .balance,
                expected_balance
            );

            // block after still gives expected value
            assert_eq!(
                query_balance_at(deps.as_ref(), addr1.clone(), block + 1)
                    .unwrap()
                    .balance,
                expected_balance
            );

            balance_previous_value = expected_balance;
        }

        // Check addr2 balances
        let mut balance_previous_value = Uint128::zero();
        for (block, expected_balance) in expected_addr2_balances {
            // block before gives previous value
            assert_eq!(
                query_balance_at(deps.as_ref(), addr2.clone(), block - 10)
                    .unwrap()
                    .balance,
                balance_previous_value
            );

            // block gives expected value
            assert_eq!(
                query_balance_at(deps.as_ref(), addr2.clone(), block)
                    .unwrap()
                    .balance,
                expected_balance
            );

            // block after still gives expected value
            assert_eq!(
                query_balance_at(deps.as_ref(), addr2.clone(), block + 1)
                    .unwrap()
                    .balance,
                expected_balance
            );

            balance_previous_value = expected_balance;
        }
    }
}
