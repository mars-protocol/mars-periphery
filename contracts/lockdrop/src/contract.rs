use cosmwasm_std::{
    entry_point, to_binary, Addr, Binary, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, QuerierWrapper, QueryRequest, Response, StdError, StdResult, Uint128, WasmMsg,
    WasmQuery,
};

use mars_core::address_provider::helpers::{query_address, query_addresses};
use mars_core::address_provider::MarsContract;
use mars_core::helpers::{option_string_to_addr, zero_address};
use mars_core::incentives::msg::QueryMsg::UserUnclaimedRewards;
use mars_core::tax::deduct_tax;
use mars_periphery::auction::Cw20HookMsg as AuctionCw20HookMsg;
use mars_periphery::helpers::{
    build_send_cw20_token_msg, build_send_native_asset_msg, build_transfer_cw20_token_msg,
    cw20_get_balance,
};
use mars_periphery::lockdrop::{
    CallbackMsg, ConfigResponse, ExecuteMsg, InstantiateMsg, LockUpInfoResponse, QueryMsg,
    StateResponse, UpdateConfigMsg, UserInfoResponse,
};

use crate::state::{Config, State, UserInfo, CONFIG, LOCKUP_INFO, STATE, USER_INFO};

const UUSD_DENOM: &str = "uusd";
//----------------------------------------------------------------------------------------
// Entry Points
//----------------------------------------------------------------------------------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    // CHECK :: init_timestamp needs to be valid
    if msg.init_timestamp < env.block.time.seconds() {
        return Err(StdError::generic_err(format!(
            "Invalid timestamp. Current timestamp : {}",
            env.block.time.seconds()
        )));
    }
    // CHECK :: deposit_window,withdrawal_window need to be valid (withdrawal_window < deposit_window)
    if msg.deposit_window == 0u64
        || msg.withdrawal_window == 0u64
        || msg.deposit_window <= msg.withdrawal_window
    {
        return Err(StdError::generic_err("Invalid deposit / withdraw window"));
    }

    // CHECK :: min_lock_duration , max_lock_duration need to be valid (min_lock_duration < max_lock_duration)
    if msg.max_duration <= msg.min_duration {
        return Err(StdError::generic_err("Invalid Lockup durations"));
    }

    let config = Config {
        owner: deps.api.addr_validate(&msg.owner)?,
        address_provider: option_string_to_addr(deps.api, msg.address_provider, zero_address())?,
        ma_ust_token: option_string_to_addr(deps.api, msg.ma_ust_token, zero_address())?,
        auction_contract_address: option_string_to_addr(
            deps.api,
            msg.auction_contract_address,
            zero_address(),
        )?,
        init_timestamp: msg.init_timestamp,
        deposit_window: msg.deposit_window,
        withdrawal_window: msg.withdrawal_window,
        min_lock_duration: msg.min_duration,
        max_lock_duration: msg.max_duration,
        seconds_per_week: msg.seconds_per_week,
        weekly_multiplier: msg.weekly_multiplier,
        weekly_divider: msg.weekly_divider,
        lockdrop_incentives: msg.lockdrop_incentives,
    };

    let state = State {
        final_ust_locked: Uint128::zero(),
        final_maust_locked: Uint128::zero(),
        total_ust_locked: Uint128::zero(),
        total_maust_locked: Uint128::zero(),
        total_deposits_weight: Uint128::zero(),
        total_mars_delegated: Uint128::zero(),
        are_claims_allowed: false,
        xmars_rewards_index: Decimal::zero(),
    };

    CONFIG.save(deps.storage, &config)?;
    STATE.save(deps.storage, &state)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::UpdateConfig { new_config } => update_config(deps, env, info, new_config),
        ExecuteMsg::DepositUst { duration } => try_deposit_ust(deps, env, info, duration),
        ExecuteMsg::WithdrawUst { duration, amount } => {
            try_withdraw_ust(deps, env, info, duration, amount)
        }
        ExecuteMsg::DepositMarsToAuction { amount } => {
            handle_deposit_mars_to_auction(deps, env, info, amount)
        }
        ExecuteMsg::EnableClaims {} => handle_enable_claims(deps, info),
        ExecuteMsg::DepositUstInRedBank {} => try_deposit_in_red_bank(deps, env, info),
        ExecuteMsg::ClaimRewardsAndUnlock {
            lockup_to_unlock_duration,
            forceful_unlock,
        } => handle_claim_rewards_and_unlock_position(
            deps,
            env,
            info,
            lockup_to_unlock_duration,
            forceful_unlock,
        ),
        ExecuteMsg::Callback(msg) => _handle_callback(deps, env, info, msg),
    }
}

fn _handle_callback(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: CallbackMsg,
) -> StdResult<Response> {
    // Callback functions can only be called this contract itself
    if info.sender != env.contract.address {
        return Err(StdError::generic_err(
            "callbacks cannot be invoked externally",
        ));
    }
    match msg {
        CallbackMsg::UpdateStateOnRedBankDeposit {
            prev_ma_ust_balance,
        } => update_state_on_red_bank_deposit(deps, env, prev_ma_ust_balance),
        CallbackMsg::UpdateStateOnClaim {
            user,
            prev_xmars_balance,
        } => update_state_on_claim(deps, env, user, prev_xmars_balance),
        CallbackMsg::DissolvePosition {
            user,
            duration,
            forceful_unlock,
        } => try_dissolve_position(deps, env, user, duration, forceful_unlock),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::UserInfo { address } => to_binary(&query_user_info(deps, env, address)?),
        QueryMsg::LockUpInfo { address, duration } => {
            to_binary(&query_lockup_info(deps, address, duration)?)
        }
        QueryMsg::LockUpInfoWithId { lockup_id } => {
            to_binary(&query_lockup_info_with_id(deps, lockup_id)?)
        }
    }
}

//----------------------------------------------------------------------------------------
// Handle Functions
//----------------------------------------------------------------------------------------

/// @dev ADMIN Function. Facilitates state update. Will be used to set address_provider / maUST token address most probably, based on deployment schedule
/// @params new_config : New configuration struct
pub fn update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_config: UpdateConfigMsg,
) -> StdResult<Response> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.owner {
        return Err(StdError::generic_err("Only owner can update configuration"));
    }

    // UPDATE :: ADDRESSES IF PROVIDED
    config.address_provider = option_string_to_addr(
        deps.api,
        new_config.address_provider,
        config.address_provider,
    )?;
    config.ma_ust_token =
        option_string_to_addr(deps.api, new_config.ma_ust_token, config.ma_ust_token)?;
    config.auction_contract_address = option_string_to_addr(
        deps.api,
        new_config.auction_contract_address,
        config.auction_contract_address,
    )?;
    config.owner = option_string_to_addr(deps.api, new_config.owner, config.owner)?;

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "lockdrop::ExecuteMsg::UpdateConfig"))
}

/// @dev Facilitates UST deposits locked for selected number of weeks
/// @param duration : Number of weeks for which UST will be locked
pub fn try_deposit_ust(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    duration: u64,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    let depositor_address = info.sender.clone();

    // CHECK :: Lockdrop deposit window open
    if !is_deposit_open(env.block.time.seconds(), &config) {
        return Err(StdError::generic_err("Deposit window closed"));
    }

    // Check if multiple native coins sent by the user
    if info.funds.len() > 1 {
        return Err(StdError::generic_err("Trying to deposit several coins"));
    }

    let native_token = info.funds.first().unwrap();
    if native_token.denom != *UUSD_DENOM {
        return Err(StdError::generic_err(
            "Only UST among native tokens accepted",
        ));
    }
    // CHECK ::: Amount needs to be valid
    if native_token.amount.is_zero() {
        return Err(StdError::generic_err("Amount must be greater than 0"));
    }

    // CHECK :: Valid Lockup Duration
    if duration > config.max_lock_duration || duration < config.min_lock_duration {
        return Err(StdError::generic_err(format!(
            "Lockup duration needs to be between {} and {}",
            config.min_lock_duration, config.max_lock_duration
        )));
    }

    // LOCKUP INFO :: RETRIEVE --> UPDATE
    let lockup_id = depositor_address.to_string() + &duration.to_string();
    let mut lockup_info = LOCKUP_INFO
        .may_load(deps.storage, lockup_id.as_bytes())?
        .unwrap_or_default();

    lockup_info.ust_locked += native_token.amount;

    // USER INFO :: RETRIEVE --> UPDATE
    let mut user_info = USER_INFO
        .may_load(deps.storage, &depositor_address)?
        .unwrap_or_default();

    user_info.total_ust_locked += native_token.amount;

    if lockup_info.duration == 0u64 {
        lockup_info.duration = duration;
        lockup_info.unlock_timestamp = calculate_unlock_timestamp(&config, duration);
        user_info.lockup_positions.push(lockup_id.clone());
    }

    // STATE :: UPDATE --> SAVE
    state.total_ust_locked += native_token.amount;
    state.total_deposits_weight += calculate_weight(native_token.amount, duration, &config);

    STATE.save(deps.storage, &state)?;
    LOCKUP_INFO.save(deps.storage, lockup_id.as_bytes(), &lockup_info)?;
    USER_INFO.save(deps.storage, &depositor_address, &user_info)?;

    Ok(Response::new().add_attributes(vec![
        ("action", "lockdrop::ExecuteMsg::lock_ust"),
        ("user", &depositor_address.to_string()),
        ("duration", duration.to_string().as_str()),
        ("ust_deposited", native_token.amount.to_string().as_str()),
    ]))
}

/// @dev Facilitates UST withdrawal from an existing Lockup position. Can only be called when deposit / withdrawal window is open
/// @param duration : Duration of the lockup position from which withdrawal is to be made
/// @param withdraw_amount :  UST amount to be withdrawn
pub fn try_withdraw_ust(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    duration: u64,
    withdraw_amount: Uint128,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // USER ADDRESS AND LOCKUP DETAILS
    let withdrawer_address = info.sender;
    let lockup_id = withdrawer_address.to_string() + &duration.to_string();
    let mut lockup_info = LOCKUP_INFO
        .may_load(deps.storage, lockup_id.as_bytes())?
        .unwrap_or_default();

    // CHECK :: Lockdrop withdrawal window open
    if !is_withdraw_open(env.block.time.seconds(), &config) {
        return Err(StdError::generic_err("Withdrawals not allowed"));
    }

    // CHECK :: Valid Lockup
    if lockup_info.ust_locked.is_zero() {
        return Err(StdError::generic_err("Lockup doesn't exist"));
    }

    // Check :: Amount should be within the allowed withdrawal limit bounds
    let max_withdrawal_percent = allowed_withdrawal_percent(env.block.time.seconds(), &config);
    let max_withdrawal_allowed = lockup_info.ust_locked * max_withdrawal_percent;
    if withdraw_amount > max_withdrawal_allowed {
        return Err(StdError::generic_err(format!(
            "Amount exceeds maximum allowed withdrawal limit of {} ",
            max_withdrawal_allowed
        )));
    }

    // Update withdrawal flag after the deposit window
    if env.block.time.seconds() >= config.init_timestamp + config.deposit_window {
        lockup_info.withdrawal_flag = true;
    }

    // LOCKUP INFO :: RETRIEVE --> UPDATE
    lockup_info.ust_locked -= withdraw_amount;

    // USER INFO :: RETRIEVE --> UPDATE
    let mut user_info = USER_INFO
        .may_load(deps.storage, &withdrawer_address)?
        .unwrap_or_default();

    user_info.total_ust_locked -= withdraw_amount;
    if lockup_info.ust_locked == Uint128::zero() {
        remove_lockup_pos_from_user_info(&mut user_info, lockup_id.clone());
    }

    // STATE :: UPDATE --> SAVE
    state.total_ust_locked -= withdraw_amount;
    state.total_deposits_weight -= calculate_weight(withdraw_amount, duration, &config);

    STATE.save(deps.storage, &state)?;
    LOCKUP_INFO.save(deps.storage, lockup_id.as_bytes(), &lockup_info)?;
    USER_INFO.save(deps.storage, &withdrawer_address, &user_info)?;

    // COSMOS_MSG ::TRANSFER WITHDRAWN UST
    let withdraw_msg = build_send_native_asset_msg(
        deps.as_ref(),
        withdrawer_address.clone(),
        UUSD_DENOM,
        withdraw_amount.into(),
    )?;

    Ok(Response::new()
        .add_messages(vec![withdraw_msg])
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::withdraw_ust"),
            ("user", &withdrawer_address.to_string()),
            ("duration", duration.to_string().as_str()),
            ("ust_withdrawn", withdraw_amount.to_string().as_str()),
        ]))
}

/// @dev Function callable only by Auction contract to enable MARS Claims by users. Called along-with Bootstrap Auction contract's LP Pool provide liquidity tx
pub fn handle_enable_claims(deps: DepsMut, info: MessageInfo) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // CHECK :: ONLY AUCTION CONTRACT CAN CALL THIS FUNCTION
    if info.sender != config.auction_contract_address {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // CHECK ::: Claims are only enabled once
    if state.are_claims_allowed {
        return Err(StdError::generic_err("Already allowed"));
    }
    state.are_claims_allowed = true;

    STATE.save(deps.storage, &state)?;
    Ok(Response::new().add_attribute("action", "Lockdrop::ExecuteMsg::EnableClaims"))
}

/// @dev Admin Function. Deposits all UST into the Red Bank
pub fn try_deposit_in_red_bank(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    // CHECK :: Only Owner can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // CHECK :: Lockdrop deposit window should be closed
    if env.block.time.seconds() < config.init_timestamp
        || is_deposit_open(env.block.time.seconds(), &config)
    {
        return Err(StdError::generic_err(
            "Lockdrop deposits haven't concluded yet",
        ));
    }

    // CHECK :: Revert in-case funds have already been deposited in red-bank
    if state.final_maust_locked > Uint128::zero() {
        return Err(StdError::generic_err("Already deposited"));
    }

    // FETCH CURRENT BALANCES (UST / maUST), PREPARE DEPOSIT MSG
    let red_bank = query_address(
        &deps.querier,
        config.address_provider,
        MarsContract::RedBank,
    )?;
    let ma_ust_balance = cw20_get_balance(
        &deps.querier,
        config.ma_ust_token,
        env.contract.address.clone(),
    )?;

    // COSMOS_MSG :: DEPOSIT UST IN RED BANK
    let deposit_msg = build_deposit_into_redbank_msg(
        deps.as_ref(),
        red_bank,
        UUSD_DENOM.to_string(),
        state.total_ust_locked,
    )?;

    // COSMOS_MSG :: UPDATE CONTRACT STATE
    let update_state_msg = CallbackMsg::UpdateStateOnRedBankDeposit {
        prev_ma_ust_balance: ma_ust_balance,
    }
    .to_cosmos_msg(&env.contract.address)?;

    Ok(Response::new()
        .add_messages(vec![deposit_msg, update_state_msg])
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::DepositInRedBank"),
            (
                "ust_deposited_in_red_bank",
                state.total_ust_locked.to_string().as_str(),
            ),
            ("timestamp", env.block.time.seconds().to_string().as_str()),
        ]))
}

// @dev Function to delegate part of the MARS rewards to be used for LP Bootstrapping via auction
/// @param amount : Number of MARS to delegate
pub fn handle_deposit_mars_to_auction(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let user_address = info.sender.clone();

    // CHECK :: Have the deposit / withdraw windows concluded
    if env.block.time.seconds()
        < (config.init_timestamp + config.deposit_window + config.withdrawal_window)
    {
        return Err(StdError::generic_err(
            "Deposit / withdraw windows not closed yet",
        ));
    }

    // CHECK :: Can users withdraw their MARS tokens ? -> if so, then delegation is no longer allowed
    if state.are_claims_allowed {
        return Err(StdError::generic_err("Auction deposits no longer possible"));
    }

    let mut user_info = USER_INFO
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    // CHECK :: User needs to have atleast 1 lockup position
    if user_info.lockup_positions.is_empty() {
        return Err(StdError::generic_err("No valid lockup positions"));
    }

    // Init response
    let mut response =
        Response::new().add_attribute("action", "Auction::ExecuteMsg::DelegateMarsToAuction");

    // If user's total maUST share == 0 :: We update it
    if user_info.total_maust_share.is_zero() {
        user_info.total_maust_share = calculate_ma_ust_share(
            user_info.total_ust_locked,
            state.final_ust_locked,
            state.final_maust_locked,
        );
        response = response.add_attribute(
            "user_total_maust_share",
            user_info.total_maust_share.to_string(),
        );
    }

    // If user's total MARS rewards == 0 :: We update all of the user's lockup positions to calculate MARS rewards
    if user_info.total_mars_incentives == Uint128::zero() {
        user_info.total_mars_incentives = update_mars_rewards_allocated_to_lockup_positions(
            deps.branch(),
            &config,
            &state,
            user_info.clone(),
        )?;
        response = response.add_attribute(
            "user_total_mars_incentives",
            user_info.total_mars_incentives.to_string(),
        );
    }

    // CHECK :: ASTRO to delegate cannot exceed user's unclaimed ASTRO balance
    if amount > (user_info.total_mars_incentives - user_info.delegated_mars_incentives) {
        return Err(StdError::generic_err(format!("Amount cannot exceed user's unclaimed MARS balance. MARS to delegate = {}, Max delegatable MARS = {} ",amount, (user_info.total_mars_incentives - user_info.delegated_mars_incentives))));
    }

    // UPDATE STATE
    user_info.delegated_mars_incentives += amount;
    state.total_mars_delegated += amount;

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;
    USER_INFO.save(deps.storage, &user_address, &user_info)?;

    let mars_token_address = query_address(
        &deps.querier,
        config.address_provider,
        MarsContract::MarsToken,
    )?;

    // COSMOS_MSG ::Delegate MARS to the LP Bootstrapping via Auction contract
    let delegate_msg = build_send_cw20_token_msg(
        config.auction_contract_address.to_string(),
        mars_token_address.to_string(),
        amount,
        to_binary(&AuctionCw20HookMsg::DepositMarsTokens {
            user_address: info.sender,
        })?,
    )?;
    response = response
        .add_message(delegate_msg)
        .add_attribute("user_address", &user_address.to_string())
        .add_attribute("delegated_mars", amount.to_string());

    Ok(response)
}

/// @dev Function to claim Rewards and optionally unlock a lockup position (either naturally or forcefully). Claims pending incentives (xMARS) internally and accounts for them via the index updates
/// @params lockup_to_unlock_duration : Duration of the lockup to be unlocked. If 0 then no lockup is to be unlocked
/// @params forceful_unlock : Boolean value indicating is the unlock is forceful or natural
pub fn handle_claim_rewards_and_unlock_position(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    lockup_to_unlock_duration: u64,
    forceful_unlock: bool,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let user_address = info.sender;
    let mut user_info = USER_INFO
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    let mut response = Response::new().add_attribute(
        "action",
        "Auction::ExecuteMsg::ClaimRewardsAndUnlockPosition",
    );

    // If a lockup is to be unlocked, then we check -
    // 1. Is it a valid lockup position
    // 2. Is is forceful unlock? If not, then can it be unlocked
    if lockup_to_unlock_duration > 0u64 {
        let lockup_id = user_address.to_string() + &lockup_to_unlock_duration.to_string();
        let lockup_info = LOCKUP_INFO
            .may_load(deps.storage, lockup_id.as_bytes())?
            .unwrap_or_default();

        if lockup_info.ust_locked == Uint128::zero() {
            return Err(StdError::generic_err("Invalid lockup"));
        }

        if !forceful_unlock && lockup_info.unlock_timestamp > env.block.time.seconds() {
            let time_remaining = lockup_info.unlock_timestamp - env.block.time.seconds();
            return Err(StdError::generic_err(format!(
                "{} seconds to Unlock",
                time_remaining
            )));
        }

        response = response
            .add_attribute("action", "unlock_position")
            .add_attribute("ust_amount", lockup_info.ust_locked.to_string())
            .add_attribute("duration", lockup_info.duration.to_string())
            .add_attribute("forceful_unlock", forceful_unlock.to_string())
    }

    // CHECKS ::
    // 2. Valid lockup positions available ?
    // 3. Are claims allowed
    if user_info.total_ust_locked == Uint128::zero() {
        return Err(StdError::generic_err("No lockup to claim rewards for"));
    }
    if !state.are_claims_allowed {
        return Err(StdError::generic_err("Claim not allowed"));
    }

    // If user's total maUST share == 0 :: We update it
    if user_info.total_maust_share.is_zero() {
        user_info.total_maust_share = calculate_ma_ust_share(
            user_info.total_ust_locked,
            state.final_ust_locked,
            state.final_maust_locked,
        );
        response = response.add_attribute(
            "user_total_maust_share",
            user_info.total_maust_share.to_string(),
        );
    }

    // If user's total MARS rewards == 0 :: We update all of the user's lockup positions to calculate MARS rewards
    if user_info.total_mars_incentives.is_zero() {
        user_info.total_mars_incentives = update_mars_rewards_allocated_to_lockup_positions(
            deps.branch(),
            &config,
            &state,
            user_info.clone(),
        )?;
        response = response.add_attribute(
            "user_total_mars_incentives",
            user_info.total_mars_incentives.to_string(),
        );
    }

    // QUERY:: XMARS & Incentives Contract addresses
    let mars_contracts = vec![MarsContract::Incentives, MarsContract::XMarsToken];
    let mut addresses_query = query_addresses(
        &deps.querier.clone(),
        config.address_provider,
        mars_contracts,
    )?;
    let xmars_address = addresses_query.pop().unwrap();
    let incentives_address = addresses_query.pop().unwrap();

    // MARS REWARDS :: Query if any rewards to claim and if so, claim them (we receive them as XMARS)
    let mars_unclaimed: Uint128 = query_pending_mars_to_be_claimed(
        &deps.querier,
        incentives_address.to_string(),
        env.contract.address.to_string(),
    )?;
    let xmars_balance =
        cw20_get_balance(&deps.querier, xmars_address, env.contract.address.clone())?;

    if !mars_unclaimed.is_zero() {
        let claim_xmars_msg = build_claim_xmars_rewards(incentives_address)?;
        response = response
            .add_message(claim_xmars_msg)
            .add_attribute("xmars_claimed", "true");
    }

    // CALLBACK ::  UPDATE STATE
    let callback_msg = CallbackMsg::UpdateStateOnClaim {
        user: user_address.clone(),
        prev_xmars_balance: xmars_balance,
    }
    .to_cosmos_msg(&env.contract.address)?;
    response = response.add_message(callback_msg);

    // CALLBACK MSG :: DISSOLVE LOCKUP POSITION
    if lockup_to_unlock_duration > 0u64 {
        let callback_dissolve_position_msg = CallbackMsg::DissolvePosition {
            user: user_address,
            duration: lockup_to_unlock_duration,
            forceful_unlock,
        }
        .to_cosmos_msg(&env.contract.address)?;
        response = response.add_message(callback_dissolve_position_msg);
    }

    Ok(response)
}

//----------------------------------------------------------------------------------------
// Callback Functions
//----------------------------------------------------------------------------------------

/// @dev Callback function. Updates state after UST is deposited in the Red Bank
/// @params prev_ma_ust_balance : Previous maUST Token balance
pub fn update_state_on_red_bank_deposit(
    deps: DepsMut,
    env: Env,
    prev_ma_ust_balance: Uint128,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    let cur_ma_ust_balance =
        cw20_get_balance(&deps.querier, config.ma_ust_token, env.contract.address)?;
    let m_ust_minted = cur_ma_ust_balance - prev_ma_ust_balance;

    // STATE :: UPDATE --> SAVE
    state.final_ust_locked = state.total_ust_locked;
    state.final_maust_locked = m_ust_minted;

    state.total_ust_locked = Uint128::zero();
    state.total_maust_locked = m_ust_minted;

    STATE.save(deps.storage, &state)?;

    Ok(Response::new().add_attributes(vec![
        ("action", "lockdrop::CallbackMsg::RedBankDeposit"),
        ("maUST_minted", m_ust_minted.to_string().as_str()),
    ]))
}

/// @dev Callback function. Updated indexes (if xMars is claimed), calculates user's Mars rewards (if not already done), and transfers rewards (MARS and xMars) to the user
/// @params user : User address
/// @params prev_xmars_balance : Previous xMars balance. Used to calculate how much xMars was claimed from the incentives contract
pub fn update_state_on_claim(
    deps: DepsMut,
    env: Env,
    user: Addr,
    prev_xmars_balance: Uint128,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?; // Index is updated
    let mut user_info = USER_INFO.may_load(deps.storage, &user)?.unwrap_or_default();

    // QUERY:: xMars and Mars Contract addresses
    let mars_contracts = vec![MarsContract::MarsToken, MarsContract::XMarsToken];
    let mut addresses_query =
        query_addresses(&deps.querier, config.address_provider, mars_contracts)?;
    let xmars_address = addresses_query.pop().unwrap();
    let mars_address = addresses_query.pop().unwrap();

    let mut response = Response::new().add_attribute("user_address", user.to_string());

    // Calculate XMARS Claimed as rewards
    let cur_xmars_balance =
        cw20_get_balance(&deps.querier, xmars_address.clone(), env.contract.address)?;
    let xmars_accured = cur_xmars_balance - prev_xmars_balance;
    response = response.add_attribute("total_xmars_claimed", xmars_accured.to_string());

    // UPDATE :: GLOBAL & USER INDEX (XMARS rewards tracker)
    if xmars_accured > Uint128::zero() {
        update_xmars_rewards_index(&mut state, xmars_accured);
    }

    // COSMOS MSG :: SEND X-MARS (DEPOSIT INCENTIVES) IF > 0
    let pending_xmars_rewards = compute_user_accrued_reward(&state, &mut user_info);
    if pending_xmars_rewards > Uint128::zero() {
        user_info.total_xmars_claimed += pending_xmars_rewards;

        let transfer_xmars_msg = build_transfer_cw20_token_msg(
            user.clone(),
            xmars_address.to_string(),
            pending_xmars_rewards,
        )?;

        response = response
            .add_message(transfer_xmars_msg)
            .add_attribute("user_xmars_claimed", pending_xmars_rewards.to_string());
    }

    // COSMOS MSG :: SEND MARS (LOCKDROP REWARD) IF > 0
    if !user_info.lockdrop_claimed {
        let mars_to_transfer =
            user_info.total_mars_incentives - user_info.delegated_mars_incentives;
        let transfer_mars_msg = build_transfer_cw20_token_msg(
            user.clone(),
            mars_address.to_string(),
            mars_to_transfer,
        )?;

        user_info.lockdrop_claimed = true;
        response = response
            .add_message(transfer_mars_msg)
            .add_attribute("user_mars_claimed", mars_to_transfer.to_string());
    }

    // SAVE UPDATED STATES
    STATE.save(deps.storage, &state)?;
    USER_INFO.save(deps.storage, &user, &user_info)?;

    Ok(response)
}

// CALLBACK :: CALLED BY try_unlock_position FUNCTION --> DELETES LOCKUP POSITION
/// @dev  Callback function. Unlocks a lockup position. Either naturally after duration expiration or forcefully by returning MARS (lockdrop incentives)
/// @params user : User address whose position is to be unlocked
/// @params duration :Lockup duration of the position to be unlocked
/// @params forceful_unlock : Boolean value indicating is the unlock is forceful or not
pub fn try_dissolve_position(
    deps: DepsMut,
    env: Env,
    user: Addr,
    duration: u64,
    forceful_unlock: bool,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let mut user_info = USER_INFO.may_load(deps.storage, &user)?.unwrap_or_default();

    let lockup_id = user.to_string() + &duration.to_string();
    let mut lockup_info = LOCKUP_INFO
        .may_load(deps.storage, lockup_id.as_bytes())?
        .unwrap_or_default();

    let maust_to_withdraw = calculate_ma_ust_share(
        lockup_info.ust_locked,
        state.final_ust_locked,
        state.final_maust_locked,
    );

    // UPDATE STATE
    state.total_maust_locked -= maust_to_withdraw;

    // UPDATE USER INFO
    // user_info.total_ust_locked = user_info.total_ust_locked - lockup_info.ust_locked;
    user_info.total_maust_share -= maust_to_withdraw;

    // DISSOLVE LOCKUP POSITION
    lockup_info.ust_locked = Uint128::zero();
    remove_lockup_pos_from_user_info(&mut user_info, lockup_id.clone());

    let mut cosmos_msgs = vec![];

    // If forceful unlock, user needs to return MARS Lockdrop rewards he received against this lockup position
    if forceful_unlock {
        // QUERY:: Mars Contract addresses
        let mars_token_address = query_address(
            &deps.querier,
            config.address_provider,
            MarsContract::MarsToken,
        )?;
        // COSMOS MSG :: Transfer MARS from user to itself
        cosmos_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: mars_token_address.to_string(),
            funds: vec![],
            msg: to_binary(&cw20::Cw20ExecuteMsg::TransferFrom {
                owner: user.to_string(),
                recipient: env.contract.address.to_string(),
                amount: lockup_info.lockdrop_reward,
            })?,
        }));
    }

    let maust_transfer_msg = build_transfer_cw20_token_msg(
        user.clone(),
        config.ma_ust_token.to_string(),
        maust_to_withdraw,
    )?;
    cosmos_msgs.push(maust_transfer_msg);

    STATE.save(deps.storage, &state)?;
    USER_INFO.save(deps.storage, &user, &user_info)?;
    LOCKUP_INFO.remove(deps.storage, lockup_id.as_bytes());

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attributes(vec![
            ("action", "lockdrop::Callback::DissolvePosition"),
            ("ma_ust_transferred", maust_to_withdraw.to_string().as_str()),
        ]))
}

//----------------------------------------------------------------------------------------
// Query Functions
//----------------------------------------------------------------------------------------

/// @dev Returns the contract's configuration
pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        owner: config.owner.to_string(),
        address_provider: config.address_provider.to_string(),
        ma_ust_token: config.ma_ust_token.to_string(),
        auction_contract_address: config.auction_contract_address.to_string(),
        init_timestamp: config.init_timestamp,
        deposit_window: config.deposit_window,
        withdrawal_window: config.withdrawal_window,
        min_duration: config.min_lock_duration,
        max_duration: config.max_lock_duration,
        weekly_multiplier: config.weekly_multiplier,
        weekly_divider: config.weekly_divider,
        lockdrop_incentives: config.lockdrop_incentives,
    })
}

/// @dev Returns the contract's Global State
pub fn query_state(deps: Deps) -> StdResult<StateResponse> {
    let state: State = STATE.load(deps.storage)?;
    Ok(StateResponse {
        final_ust_locked: state.final_ust_locked,
        final_maust_locked: state.final_maust_locked,
        total_ust_locked: state.total_ust_locked,
        total_maust_locked: state.total_maust_locked,
        total_mars_delegated: state.total_mars_delegated,
        are_claims_allowed: state.are_claims_allowed,
        total_deposits_weight: state.total_deposits_weight,
        xmars_rewards_index: state.xmars_rewards_index,
    })
}

/// @dev Returns summarized details regarding the user
/// @params user_address : User address whose state is being queries
pub fn query_user_info(deps: Deps, env: Env, user_address_: String) -> StdResult<UserInfoResponse> {
    let config = CONFIG.load(deps.storage)?;
    let user_address = deps.api.addr_validate(&user_address_)?;
    let mut state: State = STATE.load(deps.storage)?;
    let mut user_info = USER_INFO
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    // Calculate user's maUST share if not already done
    if user_info.total_maust_share == Uint128::zero() && state.final_maust_locked != Uint128::zero()
    {
        user_info.total_maust_share = calculate_ma_ust_share(
            user_info.total_ust_locked,
            state.final_ust_locked,
            state.final_maust_locked,
        );
    }

    // Calculate user's lockdrop incentive share if not finalized
    if user_info.total_mars_incentives == Uint128::zero() {
        for lockup_id in user_info.lockup_positions.clone().iter() {
            let lockup_info = LOCKUP_INFO
                .load(deps.storage, lockup_id.as_bytes())
                .unwrap();
            let position_rewards = calculate_mars_incentives_for_lockup(
                lockup_info.ust_locked,
                lockup_info.duration,
                &config,
                state.total_deposits_weight,
            );
            user_info.total_mars_incentives += position_rewards;
        }
    }

    // QUERY:: Contract addresses
    let mars_contracts = vec![MarsContract::Incentives];
    let mut addresses_query =
        query_addresses(&deps.querier, config.address_provider, mars_contracts)?;
    let incentives_address = addresses_query.pop().unwrap();

    // QUERY :: XMARS REWARDS TO BE CLAIMED  ?
    let xmars_accured: Uint128 = deps
        .querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: incentives_address.to_string(),
            msg: to_binary(&UserUnclaimedRewards {
                user_address: env.contract.address.to_string(),
            })
            .unwrap(),
        }))
        .unwrap();

    update_xmars_rewards_index(&mut state, xmars_accured);
    let pending_xmars_to_claim = compute_user_accrued_reward(&state, &mut user_info);

    Ok(UserInfoResponse {
        total_ust_locked: user_info.total_ust_locked,
        total_maust_share: user_info.total_maust_share,
        lockup_position_ids: user_info.lockup_positions,
        total_mars_incentives: user_info.total_mars_incentives,
        delegated_mars_incentives: user_info.delegated_mars_incentives,
        is_lockdrop_claimed: user_info.lockdrop_claimed,
        reward_index: user_info.reward_index,
        total_xmars_claimed: user_info.total_xmars_claimed,
        pending_xmars_to_claim,
    })
}

/// @dev Returns summarized details regarding the user
pub fn query_lockup_info(deps: Deps, user: String, duration: u64) -> StdResult<LockUpInfoResponse> {
    let lockup_id = user + &duration.to_string();
    query_lockup_info_with_id(deps, lockup_id)
}

/// @dev Returns summarized details regarding the user
pub fn query_lockup_info_with_id(deps: Deps, lockup_id: String) -> StdResult<LockUpInfoResponse> {
    let lockup_info = LOCKUP_INFO
        .may_load(deps.storage, lockup_id.as_bytes())?
        .unwrap_or_default();
    let state: State = STATE.load(deps.storage)?;

    let mut lockup_response = LockUpInfoResponse {
        duration: lockup_info.duration,
        ust_locked: lockup_info.ust_locked,
        maust_balance: calculate_ma_ust_share(
            lockup_info.ust_locked,
            state.final_ust_locked,
            state.final_maust_locked,
        ),
        lockdrop_reward: lockup_info.lockdrop_reward,
        unlock_timestamp: lockup_info.unlock_timestamp,
    };

    if lockup_response.lockdrop_reward == Uint128::zero() {
        let config = CONFIG.load(deps.storage)?;
        lockup_response.lockdrop_reward = calculate_mars_incentives_for_lockup(
            lockup_response.ust_locked,
            lockup_response.duration,
            &config,
            state.total_deposits_weight,
        );
    }

    Ok(lockup_response)
}

//----------------------------------------------------------------------------------------
// HELPERS
//----------------------------------------------------------------------------------------

/// @dev Returns true if deposits are allowed
fn is_deposit_open(current_timestamp: u64, config: &Config) -> bool {
    let deposits_opened_till = config.init_timestamp + config.deposit_window;
    (current_timestamp >= config.init_timestamp) && (deposits_opened_till >= current_timestamp)
}

/// @dev Returns true if withdrawals are allowed
fn is_withdraw_open(current_timestamp: u64, config: &Config) -> bool {
    let withdrawals_opened_till = config.init_timestamp + config.withdrawal_window;
    (current_timestamp >= config.init_timestamp) && (withdrawals_opened_till >= current_timestamp)
}

/// @dev Returns the timestamp when the lockup will get unlocked
fn calculate_unlock_timestamp(config: &Config, duration: u64) -> u64 {
    config.init_timestamp + config.deposit_window + (duration * config.seconds_per_week)
}

// /// @dev Returns true if the user_info stuct's lockup_positions vector contains the lockup_id
// /// @params lockup_id : Lockup Id which is to be checked if it is present in the list or not
// fn is_lockup_present_in_user_info(user_info: &UserInfo, lockup_id: String) -> bool {
//     if user_info.lockup_positions.iter().any(|id| id == &lockup_id) {
//         return true;
//     }
//     false
// }

/// @dev Removes lockup position id from user info's lockup position list
/// @params lockup_id : Lockup Id to be removed
fn remove_lockup_pos_from_user_info(user_info: &mut UserInfo, lockup_id: String) {
    let index = user_info
        .lockup_positions
        .iter()
        .position(|x| *x == lockup_id)
        .unwrap();
    user_info.lockup_positions.remove(index);
}

///  @dev Helper function to calculate maximum % of UST deposited that can be withdrawn
/// @params current_timestamp : Current block timestamp
/// @params config : Contract configuration
fn allowed_withdrawal_percent(current_timestamp: u64, config: &Config) -> Decimal {
    let withdrawal_cutoff_init_point = config.init_timestamp + config.deposit_window;

    // Deposit window :: 100% withdrawals allowed
    if current_timestamp < withdrawal_cutoff_init_point {
        return Decimal::from_ratio(100u32, 100u32);
    }

    let withdrawal_cutoff_second_point =
        withdrawal_cutoff_init_point + (config.withdrawal_window / 2u64);
    // Deposit window closed, 1st half of withdrawal window :: 50% withdrawals allowed
    if current_timestamp <= withdrawal_cutoff_second_point {
        return Decimal::from_ratio(50u32, 100u32);
    }

    // max withdrawal allowed decreasing linearly from 50% to 0% vs time elapsed
    let withdrawal_cutoff_final = withdrawal_cutoff_init_point + config.withdrawal_window;
    //  Deposit window closed, 2nd half of withdrawal window :: max withdrawal allowed decreases linearly from 50% to 0% vs time elapsed
    if current_timestamp < withdrawal_cutoff_final {
        let time_left = withdrawal_cutoff_final - current_timestamp;
        Decimal::from_ratio(
            50u64 * time_left,
            100u64 * (withdrawal_cutoff_final - withdrawal_cutoff_second_point),
        )
    }
    // Withdrawals not allowed
    else {
        Decimal::from_ratio(0u32, 100u32)
    }
}

//-----------------------------
// HELPER FUNCTIONS :: COMPUTATIONS
//-----------------------------

/// @dev Function to calculate & update MARS rewards allocated for each of the user position
/// @params config: configuration struct
/// @params state: state struct
/// @params user_info : user Info struct
/// Returns user's total MARS rewards
fn update_mars_rewards_allocated_to_lockup_positions(
    deps: DepsMut,
    config: &Config,
    state: &State,
    user_info: UserInfo,
) -> StdResult<Uint128> {
    let mut total_mars_rewards = Uint128::zero();

    for lockup_id in user_info.lockup_positions {
        // Retrieve mutable Lockup position
        let mut lockup_info = LOCKUP_INFO
            .load(deps.storage, lockup_id.as_bytes())
            .unwrap();

        let position_rewards = calculate_mars_incentives_for_lockup(
            lockup_info.ust_locked,
            lockup_info.duration,
            config,
            state.total_deposits_weight,
        );

        lockup_info.lockdrop_reward = position_rewards;
        total_mars_rewards += position_rewards;
        LOCKUP_INFO.save(deps.storage, lockup_id.as_bytes(), &lockup_info)?;
    }
    Ok(total_mars_rewards)
}

/// @dev Helper function to calculate MARS rewards for a particular Lockup position
/// @params deposited_ust : UST deposited to that particular Lockup position
/// @params duration : Duration of the lockup
/// @params config : Configuration struct
/// @params total_deposits_weight : Total calculated weight of all the UST deposited in the contract
fn calculate_mars_incentives_for_lockup(
    deposited_ust: Uint128,
    duration: u64,
    config: &Config,
    total_deposits_weight: Uint128,
) -> Uint128 {
    if total_deposits_weight == Uint128::zero() {
        return Uint128::zero();
    }
    let amount_weight = calculate_weight(deposited_ust, duration, config);
    config.lockdrop_incentives * Decimal::from_ratio(amount_weight, total_deposits_weight)
}

/// @dev Helper function. Returns effective weight for the amount to be used for calculating lockdrop rewards
/// @params amount : Number of LP tokens
/// @params duration : Number of weeks
/// @config : Config with weekly multiplier and divider
fn calculate_weight(amount: Uint128, duration: u64, config: &Config) -> Uint128 {
    let lock_weight = Decimal::one()
        + Decimal::from_ratio(
            (duration - 1) * config.weekly_multiplier,
            config.weekly_divider,
        );
    lock_weight * amount
}

/// @dev Accrue xMARS rewards by updating the reward index
/// @params state : Global state struct
/// @params xmars_accured : xMARS tokens claimed as rewards from the incentives contract
fn update_xmars_rewards_index(state: &mut State, xmars_accured: Uint128) {
    if state.total_maust_locked == Uint128::zero() {
        return;
    }
    let xmars_rewards_index_increment =
        Decimal::from_ratio(xmars_accured, state.total_maust_locked);
    state.xmars_rewards_index = state.xmars_rewards_index + xmars_rewards_index_increment;
}

/// @dev Accrue MARS reward for the user by updating the user reward index and and returns the pending rewards (xMars) to be claimed by the user
/// @params state : Global state struct
/// @params user_info : UserInfo struct
fn compute_user_accrued_reward(state: &State, user_info: &mut UserInfo) -> Uint128 {
    if state.final_ust_locked == Uint128::zero() {
        return Uint128::zero();
    }
    let pending_xmars = (user_info.total_maust_share * state.xmars_rewards_index)
        - (user_info.total_maust_share * user_info.reward_index);
    user_info.reward_index = state.xmars_rewards_index;
    pending_xmars
}

/// @dev Returns maUST Token share against UST amount. Calculated as =  (deposited UST / Final UST deposited) * Final maUST Locked
/// @params ust_locked_share : UST amount for which maUST share is to be calculated
/// @params final_ust_locked : Total UST amount which was deposited into Red Bank
/// @params final_maust_locked : Total maUST tokens minted againt the UST deposited into Red Bank
fn calculate_ma_ust_share(
    ust_locked_share: Uint128,
    final_ust_locked: Uint128,
    final_maust_locked: Uint128,
) -> Uint128 {
    if final_ust_locked == Uint128::zero() {
        return Uint128::zero();
    }
    final_maust_locked * Decimal::from_ratio(ust_locked_share, final_ust_locked)
}

//-----------------------------
// QUERY HELPERS
//-----------------------------

/// @dev Helper function. Queries pending xMars to be claimed from the incentives contract
/// @params incentives_address : Incentives contract address
/// @params contract_addr : Address for which pending xmars is to be queried
pub fn query_pending_mars_to_be_claimed(
    querier: &QuerierWrapper,
    incentives_address: String,
    contract_addr: String,
) -> StdResult<Uint128> {
    let response = querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: incentives_address,
            msg: to_binary(&UserUnclaimedRewards {
                user_address: contract_addr,
            })
            .unwrap(),
        }))
        .unwrap();
    Ok(response)
}

//-----------------------------
// COSMOS_MSGs
//-----------------------------

/// @dev Helper function. Returns CosmosMsg to deposit UST into the Red Bank
/// @params redbank_address : Red Bank contract address
/// @params denom_stable : uusd stable denom
/// @params amount : UST amount to be deposited
fn build_deposit_into_redbank_msg(
    deps: Deps,
    redbank_address: Addr,
    denom_stable: String,
    amount: Uint128,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: redbank_address.to_string(),
        funds: vec![deduct_tax(
            deps,
            Coin {
                denom: denom_stable.to_string(),
                amount,
            },
        )?],
        msg: to_binary(&mars_core::red_bank::msg::ExecuteMsg::DepositNative {
            denom: denom_stable,
        })?,
    }))
}

/// @dev Helper function. Returns CosmosMsg to claim xMars rewards from the incentives contract
fn build_claim_xmars_rewards(incentives_contract: Addr) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: incentives_contract.to_string(),
        funds: vec![],
        msg: to_binary(&mars_core::incentives::msg::ExecuteMsg::ClaimRewards {})?,
    }))
}
