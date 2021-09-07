#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_bignumber::{Decimal256, Uint256};

use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, Response, StdError, StdResult, WasmMsg,
};

use crate::msg::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg,
    StakerInfoResponse, StateResponse,TimeResponse, UpdateConfigMsg
};

use mars::address_provider::helpers::{query_address};
use mars::address_provider::msg::MarsContract;
use mars::helpers::{option_string_to_addr, zero_address};


use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use crate::state::{Config, CONFIG, State, STATE, StakerInfo , STAKER_INFO};

//----------------------------------------------------------------------------------------
// Entry Points
//----------------------------------------------------------------------------------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(deps: DepsMut, env: Env, _info: MessageInfo, msg: InstantiateMsg, ) -> StdResult<Response> {

    if msg.init_timestamp < env.block.time.seconds() || msg.till_timestamp < msg.init_timestamp {
        return Err(StdError::generic_err("Invalid timestamps"));
    }

    if msg.cycle_duration == 0u64 {
        return Err(StdError::generic_err("Invalid cycle duration"));
    }

    let config = Config {
        owner: deps.api.addr_validate(&msg.owner.unwrap())?,
        address_provider: option_string_to_addr(deps.api, msg.address_provider, zero_address())?, 
        staking_token: option_string_to_addr(deps.api, msg.staking_token, zero_address())?, 
        init_timestamp: msg.init_timestamp ,
        till_timestamp: msg.till_timestamp ,
        cycle_duration: msg.cycle_duration ,
        reward_increase: msg.reward_increase.unwrap_or(Decimal256::zero()) ,
    };

    config.validate()?;
    CONFIG.save( deps.storage, &config)?;

    STATE.save(
        deps.storage,
        &State {
            current_cycle: 0 as u64,
            current_cycle_rewards: msg.cycle_rewards.unwrap_or(Uint256::zero()),
            last_distributed: env.block.time.seconds(),
            total_bond_amount: Uint256::zero(),
            global_reward_index: Decimal256::zero(),
        }
    )?;

    Ok(Response::default())
}



#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::UpdateConfig {new_config} => update_config(deps, env,info, new_config),
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::Unbond { amount, withdraw_pending_reward } => unbond(deps, env, info, amount, withdraw_pending_reward ),
        ExecuteMsg::Claim {} => try_claim(deps, env, info),
    }
}



#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State { timestamp } => to_binary(&query_state(deps, _env, timestamp)?),
        QueryMsg::StakerInfo { staker, timestamp } => to_binary(&query_staker_info(deps,_env, staker, timestamp)?),
        QueryMsg::Timestamp { } => to_binary(&query_timestamp( _env)?),
    }
}


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Err(StdError::generic_err("unimplemented"))
}
//----------------------------------------------------------------------------------------
// Handle Functions
//----------------------------------------------------------------------------------------

/// Only MARS-UST LP Token can be sent to this contract via the Cw20ReceiveMsg Hook
/// @dev Increases user's staked LP Token balance via the Bond Function 
pub fn receive_cw20(deps: DepsMut, env: Env, info: MessageInfo, cw20_msg: Cw20ReceiveMsg) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;

    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::Bond {}) => {
            // only staking token contract can execute this message
            if config.staking_token.to_string() != info.sender.as_str() {
                return Err(StdError::generic_err("unauthorized"));
            }
            let cw20_sender = deps.api.addr_validate(&cw20_msg.sender)?;
            bond(deps, env, cw20_sender, cw20_msg.amount.into())
        }
        Err(_) => Err(StdError::generic_err("data should be given")),
    }
}

/// @dev Called by receive_cw20(). Increases user's staked LP Token balance
/// @params sender_addr : User Address who sent the LP Tokens
/// @params amount : Number of LP Tokens transferred to the contract
pub fn bond(deps: DepsMut, env: Env, sender_addr: Addr, amount: Uint256) -> StdResult<Response> {

    let config: Config = CONFIG.load(deps.storage)?;
    let mut state: State = STATE.load(deps.storage)?;
    let mut staker_info = STAKER_INFO.may_load(deps.storage, &sender_addr)?.unwrap_or_default();

    compute_reward( &config, &mut state, env.block.time.seconds() );                    // Compute global reward
    compute_staker_reward(&state, &mut staker_info)?;                                   // Compute staker reward
    increase_bond_amount(&mut state, &mut staker_info, amount);                         // Increase bond_amount

    // Store updated state with staker's staker_info
    STAKER_INFO.save( deps.storage, &sender_addr, &staker_info)?;
    STATE.save( deps.storage, &state )?;

    Ok(Response::new().add_attributes(vec![
        ("action", "ExecuteMsg::Bond"),
        ("user", sender_addr.as_str()),
        ("amount", amount.to_string().as_str()),
    ]))
}

/// @dev Only owner can call this function. Updates the config
/// @dev init_timestamp : can only be updated before it gets elapsed
/// @dev till_timestamp : can only be updated before it gets elapsed
/// @params new_config : New config object
pub fn update_config( deps: DepsMut, env: Env, info: MessageInfo, new_config: UpdateConfigMsg ) -> StdResult<Response> { 

    let mut config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // ONLY OWNER CAN UPDATE CONFIG
    if info.sender != config.owner {
        return Err(StdError::generic_err("Only owner can update configuration"));
    }

    // ACCURE CURRENT REWARDS IN-CASE `reward_increase` / `current_cycle_rewards` ARE UPDATED
    compute_reward(&config, &mut state, env.block.time.seconds());      // Compute global reward & staker reward

    // UPDATE :: ADDRESSES IF PROVIDED
    config.address_provider = option_string_to_addr(deps.api, new_config.address_provider, config.address_provider)?;
    config.staking_token = option_string_to_addr(deps.api, new_config.staking_token, config.staking_token)?;
    config.owner = option_string_to_addr(deps.api, new_config.owner, config.owner)?;

    // UPDATE :: VALUES IF PROVIDED
    match new_config.reward_increase {
        Some(new_increase_ratio) => {
            if new_increase_ratio < Decimal256::one() {
                config.reward_increase = new_increase_ratio; 
            } else {
                return Err(StdError::generic_err("Invalid reward increase ratio"));
            }
        }
        None => {}
    }
    state.current_cycle_rewards = new_config.cycle_rewards.unwrap_or(state.current_cycle_rewards);

    // UPDATE INIT TIMESTAMP AND STATE :: DOABLE ONLY IF IT HASN'T ALREADY PASSED YET
    match new_config.init_timestamp  {
        Some(new_init_timestamp) => {
            // Update if rewards distribution has not started yet and new init_timestamp hasn't passed
            if config.init_timestamp > env.block.time.seconds() && new_init_timestamp > env.block.time.seconds() && new_init_timestamp < config.till_timestamp {
                config.init_timestamp = new_init_timestamp;
            }  else {
                return Err(StdError::generic_err("Invalid init timestamp"));
            }
        }
        None => {}
    }

    // UPDATE TILL TIMESTAMP :: DOABLE ONLY IF IT HASN'T ALREADY PASSED YET
    match new_config.till_timestamp  {
        Some(new_till_timestamp) => {
            // Update if the current till_timestamp and new till_timestamp haven't passed
            if config.till_timestamp > env.block.time.seconds() && new_till_timestamp > env.block.time.seconds() && new_till_timestamp > config.init_timestamp {
                config.till_timestamp = new_till_timestamp;
            } else {
                return Err(StdError::generic_err("Invalid till timestamp"));
            }
        }
        None => {}
    }    


    CONFIG.save(deps.storage, &config)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new().add_attribute("action", "staking::ExecuteMsg::UpdateConfig"))
}


/// @dev Reduces user's staked position. MARS Rewards are transferred along-with unstaked LP Tokens
/// @params amount :  Number of LP Tokens transferred to be unstaked
pub fn unbond(deps: DepsMut, env: Env, info: MessageInfo, amount: Uint256, withdraw_pending_reward: Option<bool>) -> StdResult<Response> {

    let sender_addr = info.sender.clone();
    let config: Config = CONFIG.load(deps.storage)?;
    let mut state: State = STATE.load(deps.storage)?;
    let mut staker_info: StakerInfo = STAKER_INFO.may_load(deps.storage, &sender_addr)?.unwrap_or_default();

    if staker_info.bond_amount < amount {
        return Err(StdError::generic_err("Cannot unbond more than bond amount"));
    }
    
    compute_reward(&config, &mut state, env.block.time.seconds());      // Compute global reward & staker reward
    compute_staker_reward(&state, &mut staker_info)?;                               // Compute staker reward
    decrease_bond_amount(&mut state, &mut staker_info, amount);                    // Decrease bond_amount
    
    let mut messages = vec![];
    let mut claimed_rewards = Uint256::zero();

    match withdraw_pending_reward { 
        Some(withdraw_pending_reward) => {
            if withdraw_pending_reward {
                claimed_rewards = staker_info.pending_reward;
                if claimed_rewards > Uint256::zero() {
                    staker_info.pending_reward = Uint256::zero();
                    let mars_token = query_address( &deps.querier,config.address_provider.clone(), MarsContract::MarsToken )?;
                    messages.push( build_send_cw20_token_msg(sender_addr.clone(), mars_token, claimed_rewards.into())? );        
                }
            }
        }
        None => {}
    }


    // Store Staker info, depends on the left bond amount
    STAKER_INFO.save( deps.storage, &sender_addr, &staker_info)?;
    STATE.save( deps.storage, &state )?;                    

    messages.push( build_send_cw20_token_msg(sender_addr.clone(), config.staking_token, amount.into())? ) ;

    // UNBOND STAKED TOKEN , TRANSFER $MARS
    Ok(Response::new()    
        .add_messages( messages)
        .add_attributes(vec![
            ("action", "ExecuteMsg::Unbond"),
            ("user", sender_addr.as_str()),
            ("amount", amount.to_string().as_str()),
            ("claimed_rewards", claimed_rewards.to_string().as_str())
        ])
    )
}



/// @dev Function to claim accrued MARS Rewards 
pub fn try_claim(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {

    let sender_addr = info.sender;
    let config: Config = CONFIG.load(deps.storage)?;
    let mut state: State = STATE.load(deps.storage)?;
    let mut staker_info = STAKER_INFO.may_load(deps.storage, &sender_addr)?.unwrap_or_default();

    // Compute global reward & staker reward
    compute_reward(&config, &mut state, env.block.time.seconds());
    compute_staker_reward(&state, &mut staker_info)?;

    let accrued_rewards = staker_info.pending_reward;
    staker_info.pending_reward = Uint256::zero();

    STAKER_INFO.save( deps.storage, &sender_addr, &staker_info)?;    // Update Staker Info
    STATE.save( deps.storage, &state )?;                               // Store updated state

    let mut messages = vec![];

    if accrued_rewards == Uint256::zero() {
        return Err(StdError::generic_err("No rewards to claim"));
    } else {
        let mars_token = query_address( &deps.querier,config.address_provider.clone(), MarsContract::MarsToken )?;
        messages.push( build_send_cw20_token_msg(sender_addr.clone(), mars_token, accrued_rewards.into())? );
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(vec![
            ("action", "ExecuteMsg::Claim"),
            ("user", sender_addr.as_str()),
            ("claimed_rewards", accrued_rewards.to_string().as_str()),
        ])
    )
}


//----------------------------------------------------------------------------------------
// Query Functions
//----------------------------------------------------------------------------------------

/// @dev Returns the contract's configuration
pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    let mars_token = query_address( &deps.querier,config.address_provider.clone(), MarsContract::MarsToken )?;

    Ok (ConfigResponse {
        owner: config.owner.to_string(),
        address_provider: config.address_provider.to_string(),
        mars_token: mars_token.to_string(),
        staking_token: config.staking_token.to_string(),
        init_timestamp: config.init_timestamp,
        till_timestamp: config.till_timestamp,
        cycle_duration: config.cycle_duration,
        reward_increase: config.reward_increase,
    })
}

/// @dev Returns the contract's simulated state at a certain timestamp
/// /// @param timestamp : Option parameter. Contract's Simulated state is retrieved if the timestamp is provided   
pub fn query_state(deps: Deps, env:Env, timestamp: Option<u64>) -> StdResult<StateResponse> {
    let mut state: State = STATE.load(deps.storage)?;
    let config = CONFIG.load(deps.storage)?;

    match timestamp {
        Some(timestamp) => {
            compute_reward(&config, &mut state, std::cmp::max(timestamp, env.block.time.seconds()) );
        }
        None => {
            compute_reward(&config, &mut state, env.block.time.seconds());
        }
    }

    Ok(StateResponse {
        current_cycle: state.current_cycle,
        current_cycle_rewards: state.current_cycle_rewards,
        last_distributed: state.last_distributed,
        total_bond_amount: state.total_bond_amount,
        global_reward_index: state.global_reward_index,
    })
}

/// @dev Returns the User's simulated state at a certain timestamp
/// @param staker : User address whose state is to be retrieved
/// @param timestamp : Option parameter. User's Simulated state is retrieved if the timestamp is provided   
pub fn query_staker_info( deps: Deps, env:Env, staker: String, timestamp: Option<u64>) -> StdResult<StakerInfoResponse> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let mut staker_info = STAKER_INFO.may_load(deps.storage, &deps.api.addr_validate(&staker)?)?.unwrap_or_default();

    match timestamp {
        Some(timestamp) => {
            compute_reward(&config, &mut state, std::cmp::max(timestamp, env.block.time.seconds()) );
        }
        None => {
            compute_reward(&config, &mut state, env.block.time.seconds());
        }
    }

    compute_staker_reward(&state, &mut staker_info)?;    

    Ok(StakerInfoResponse {
        staker,
        reward_index: staker_info.reward_index,
        bond_amount: staker_info.bond_amount,
        pending_reward: staker_info.pending_reward,
    })
}


/// @dev Returns the current timestamp
pub fn query_timestamp( env: Env) -> StdResult<TimeResponse> {
    Ok(TimeResponse { timestamp: env.block.time.seconds() })
}



//----------------------------------------------------------------------------------------
// Helper Functions
//----------------------------------------------------------------------------------------

/// @dev Increases total LP shares and user's staked LP shares by `amount`
fn increase_bond_amount(state: &mut State, staker_info: &mut StakerInfo, amount: Uint256) {
    state.total_bond_amount += amount;
    staker_info.bond_amount += amount;
}

/// @dev Decreases total LP shares and user's staked LP shares by `amount`
fn decrease_bond_amount(state: &mut State,staker_info: &mut StakerInfo,amount: Uint256) {
    state.total_bond_amount = state.total_bond_amount - amount;
    staker_info.bond_amount = staker_info.bond_amount - amount;
}

/// @dev Computes total accrued rewards 
fn compute_reward(config: &Config, state: &mut State, cur_timestamp: u64) {

    let mut last_distribution_cycle = state.current_cycle.clone();
    state.current_cycle = calculate_cycles_elapsed(cur_timestamp, config.init_timestamp, config.cycle_duration );
    let mut rewards_to_distribute = Decimal256::zero();
    let mut last_distribution_next_timestamp : u64; // 0 as u64;

    while state.current_cycle >= last_distribution_cycle {
        last_distribution_next_timestamp = calculate_init_timestamp_for_cycle(config.init_timestamp,last_distribution_cycle + 1, config.cycle_duration );
        rewards_to_distribute += rewards_distributed_for_cycle( Decimal256::from_ratio(state.current_cycle_rewards, config.cycle_duration), state.last_distributed, std::cmp::min(cur_timestamp, last_distribution_next_timestamp)  );
        state.current_cycle_rewards = calculate_cycle_rewards(state.current_cycle_rewards.clone(), config.reward_increase.clone(), state.current_cycle == last_distribution_cycle );  
        state.last_distributed = std::cmp::min(cur_timestamp, last_distribution_next_timestamp);
        last_distribution_cycle +=1;
    }

    if state.total_bond_amount == Uint256::zero() || config.init_timestamp > cur_timestamp {
        return;
    }
    
    state.global_reward_index = state.global_reward_index + (rewards_to_distribute / Decimal256::from_uint256(state.total_bond_amount));
 }


fn calculate_cycles_elapsed(current_timestamp:u64, config_init_timestamp:u64, cycle_duration:u64 ) -> u64 {
    if config_init_timestamp >= current_timestamp {
        return 0 as u64
    }
    let time_elapsed = current_timestamp - config_init_timestamp;
    time_elapsed / cycle_duration
}

fn calculate_init_timestamp_for_cycle(config_init_timestamp:u64, current_cycle:u64, cycle_duration:u64 ) -> u64 {
    config_init_timestamp + (current_cycle*cycle_duration)
}


fn rewards_distributed_for_cycle(rewards_per_sec:Decimal256, from_timestamp: u64, till_timestamp: u64 ) -> Decimal256 {
    rewards_per_sec * Decimal256::from_uint256(till_timestamp - from_timestamp)
}

fn calculate_cycle_rewards(current_cycle_rewards:Uint256, reward_increase_percent: Decimal256, is_same_cycle: bool ) -> Uint256 {
    if is_same_cycle {
        return current_cycle_rewards
    }
    current_cycle_rewards + Uint256::from(current_cycle_rewards * reward_increase_percent)
}


/// @dev Computes user's accrued rewards 
fn compute_staker_reward(state: &State, staker_info: &mut StakerInfo) -> StdResult<()> {
    let pending_reward = (staker_info.bond_amount * state.global_reward_index) - (staker_info.bond_amount * staker_info.reward_index);
    staker_info.reward_index = state.global_reward_index;
    staker_info.pending_reward += pending_reward;
    Ok(())
}


/// @dev Helper function to build `CosmosMsg` to send cw20 tokens to a recepient address 
fn build_send_cw20_token_msg(recipient: Addr, token_contract_address: Addr, amount: Uint256) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: token_contract_address.into(),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: recipient.into(),
            amount: amount.into(),
        })?,
        funds: vec![],
    }))
}


//----------------------------------------------------------------------------------------
// TESTS 
//----------------------------------------------------------------------------------------



#[cfg(test)]
mod tests { 
    use super::*;
    use cosmwasm_std::testing::{MockApi, MockStorage, MOCK_CONTRACT_ADDR};
    use cosmwasm_std::{Timestamp,BlockInfo, ContractInfo, attr, Coin, from_binary, OwnedDeps, SubMsg};
    use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
    use crate::msg::{ConfigResponse, StateResponse, StakerInfoResponse, TimeResponse, InstantiateMsg, QueryMsg  } ;
    use crate::msg::ExecuteMsg::{Receive, UpdateConfig , Unbond, Claim};
    use mars::testing::{
        assert_generic_error_message, mock_dependencies, mock_env, mock_env_at_block_time,
        mock_info, MarsMockQuerier, MockEnvParams,
    };


    #[test]
    fn test_proper_initialization() {
        let mut deps = mock_dependencies(&[]);

        let init_timestamp = 1_000_000_001;
        let till_timestamp = 1_000_000_00000;
        let reward_increase = Decimal256::from_ratio( 2u64, 100u64 );
        
        // *** Test : "Invalid cycle duration" because cycle duration = 0

        // Config with valid base params 
        let mut base_config = InstantiateMsg {
            owner: Some("owner".to_string()),
            address_provider : Some("address_provider".to_string()),
            staking_token : Some("staking_token".to_string()),
            init_timestamp: init_timestamp,
            till_timestamp: till_timestamp,
            cycle_rewards: Some( Uint256::from(100000000u64) ),
            cycle_duration: 0u64,
            reward_increase: Some(reward_increase)
        };

        let info = mock_info("owner");
        let env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(init_timestamp),
            ..Default::default()
        });

        let mut res_f = instantiate(deps.as_mut(), env.clone(), info.clone(), base_config.clone());
        assert_generic_error_message(res_f,"Invalid cycle duration");

        // *** Test : "Invalid timestamps" because (msg.init_timestamp < env.block.time.seconds())

        base_config.init_timestamp = 1_000_000_000;
        base_config.cycle_duration = 10u64;
        res_f = instantiate(deps.as_mut(), env.clone(), info.clone(), base_config.clone());
        assert_generic_error_message(res_f,"Invalid timestamps");

        // *** Test : "Invalid timestamps" because (msg.till_timestamp < msg.init_timestamp)

        base_config.init_timestamp = 1_000_000_001;
        base_config.till_timestamp = 1_000_000_000;
        res_f = instantiate(deps.as_mut(), env.clone(), info.clone(), base_config.clone());
        assert_generic_error_message(res_f,"Invalid timestamps");

        // *** Test : Should instantiate successfully

        base_config.init_timestamp = 1_000_000_001;
        base_config.till_timestamp = till_timestamp;
        // we can just call .unwrap() to assert this was a success
        let res_s = instantiate(deps.as_mut(), env.clone(), info.clone(), base_config.clone()).unwrap();
        assert_eq!(0, res_s.messages.len());
        
        // let's verify the config
        let config_ = CONFIG.load(&deps.storage).unwrap();
        assert_eq!("owner".to_string(), config_.owner);
        assert_eq!("address_provider".to_string(), config_.address_provider);
        assert_eq!("staking_token".to_string(), config_.staking_token);
        assert_eq!(init_timestamp.clone(), config_.init_timestamp);
        assert_eq!(till_timestamp.clone(), config_.till_timestamp);
        assert_eq!(10u64, config_.cycle_duration);
        assert_eq!(reward_increase.clone(), config_.reward_increase);

        // let's verify the state
        let state_ = STATE.load(&deps.storage).unwrap();
        assert_eq!(0u64, state_.current_cycle);
        assert_eq!(Uint256::from(100000000u64), state_.current_cycle_rewards);
        assert_eq!(init_timestamp, state_.last_distributed);
        assert_eq!(Uint256::zero(), state_.total_bond_amount);
        assert_eq!(Decimal256::zero(), state_.global_reward_index);
    }


    #[test]
    fn test_update_config() { 
        let mut deps = mock_dependencies(&[]);
        let mut info = mock_info("owner");
        let mut env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_000_000_00),
            ..Default::default()
        });
        let reward_increase = Decimal256::from_ratio( 2u64, 100u64 );

        // Config with valid base params 
        let base_config = InstantiateMsg {
            owner: Some("owner".to_string()),
            address_provider : Some("address_provider".to_string()),
            staking_token : Some("staking_token".to_string()),
            init_timestamp: 1_000_000_10,
            till_timestamp: 1_001_000_00,
            cycle_rewards: Some( Uint256::from(100000000u64) ),
            cycle_duration: 1000u64,
            reward_increase: Some(reward_increase)
        };
        
        // Instantiate staking contract
        let mut res_s = instantiate(deps.as_mut(), env.clone(), info.clone(), base_config.clone()).unwrap();
        assert_eq!(0, res_s.messages.len());

        // *** Test : Error "Only owner can update configuration" ****
        info = mock_info("not_owner");

        let mut new_config_msg = UpdateConfigMsg {
            owner : None,
            address_provider : Some("new_address_provider".to_string()),
            staking_token : Some("new_staking_token".to_string()),
            init_timestamp : None,
            till_timestamp : None,
            cycle_rewards : None,
            reward_increase : None,
        };

        let mut update_config_msg = UpdateConfig {
            new_config : new_config_msg.clone()
        };
        
        let mut res_f = execute(deps.as_mut(), env.clone(), info.clone(), update_config_msg.clone() );
        assert_generic_error_message(res_f,"Only owner can update configuration");

        // *** Test : Should update addresses correctly ****
        info = mock_info("owner");
        res_s = execute(deps.as_mut(), env.clone(), info.clone(), update_config_msg.clone() ).unwrap();
        assert_eq!( res_s.attributes, vec![attr("action", "staking::ExecuteMsg::UpdateConfig")] );
        let mut config_ = CONFIG.load(&deps.storage).unwrap();
        assert_eq!("new_address_provider".to_string(), config_.address_provider);
        assert_eq!("new_staking_token".to_string(), config_.staking_token);

        // *** Test : "Invalid reward increase ratio" :: Reason : new reward increase ratio = 100% (should be less than 100%) ****
        new_config_msg.reward_increase = Some(Decimal256::one());
        new_config_msg.cycle_rewards = Some(Uint256::from(1000u64));
        update_config_msg = UpdateConfig {  new_config : new_config_msg.clone() };
        res_f = execute(deps.as_mut(), env.clone(), info.clone(), update_config_msg.clone() );
        assert_generic_error_message(res_f,"Invalid reward increase ratio");

        // *** Test : Should update reward_increase, current_cycle_rewards params  correctly ****
        new_config_msg.reward_increase = Some( Decimal256::from_ratio( 7u64, 100u64 ) );
        new_config_msg.cycle_rewards = Some(Uint256::from(654u64));
        update_config_msg = UpdateConfig {  new_config : new_config_msg.clone() };
        res_s = execute(deps.as_mut(), env.clone(), info.clone(), update_config_msg.clone() ).unwrap();
        assert_eq!( res_s.attributes, vec![attr("action", "staking::ExecuteMsg::UpdateConfig")] );
        config_ = CONFIG.load(&deps.storage).unwrap();
        let mut state_ = STATE.load(&deps.storage).unwrap();
        assert_eq!("new_address_provider".to_string(), config_.address_provider);
        assert_eq!("new_staking_token".to_string(), config_.staking_token);
        assert_eq!(Decimal256::from_ratio( 7u64, 100u64 ), config_.reward_increase);
        assert_eq!(Uint256::from(654u64), state_.current_cycle_rewards);

        // *** Test : Error (Updating init_timestamp) :: Reason : Rewards already being distributed ****
        env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_000_000_11),
            ..Default::default()
        });
        new_config_msg.init_timestamp = Some(1_000_000_50) ;
        new_config_msg.reward_increase = None;
        new_config_msg.cycle_rewards = None;
        new_config_msg.staking_token = None;
        new_config_msg.address_provider = None;
        update_config_msg = UpdateConfig {  new_config : new_config_msg.clone() };
        res_f = execute(deps.as_mut(), env.clone(), info.clone(), update_config_msg.clone() );
        assert_generic_error_message(res_f,"Invalid init timestamp");

        // *** Test : Error (Updating init_timestamp) :: Reason : New init_timestamp has already passed ****
        env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_000_000_05),
            ..Default::default()
        });
        new_config_msg.init_timestamp = Some(1_000_000_04) ;
        update_config_msg = UpdateConfig {  new_config : new_config_msg.clone() };
        res_f = execute(deps.as_mut(), env.clone(), info.clone(), update_config_msg.clone() );
        assert_generic_error_message(res_f,"Invalid init timestamp");

        // *** Test : Error (Updating init_timestamp) :: Reason : New init_timestamp > config.till_timestamp ****
        new_config_msg.init_timestamp = Some(1_001_000_01) ;
        update_config_msg = UpdateConfig {  new_config : new_config_msg.clone() };
        res_f = execute(deps.as_mut(), env.clone(), info.clone(), update_config_msg.clone() );
        assert_generic_error_message(res_f,"Invalid init timestamp");

        // *** Test : Should update init_timestamp  correctly ****
        new_config_msg.init_timestamp = Some(1_000_000_15) ;
        update_config_msg = UpdateConfig {  new_config : new_config_msg.clone() };
        res_s = execute(deps.as_mut(), env.clone(), info.clone(), update_config_msg.clone() ).unwrap();
        assert_eq!( res_s.attributes, vec![attr("action", "staking::ExecuteMsg::UpdateConfig")] );
        config_ = CONFIG.load(&deps.storage).unwrap();
        assert_eq!(1_000_000_15, config_.init_timestamp);

        // *** Test : Error (Updating till_timestamp) :: Reason : Rewards distribution over ****
        env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_001_000_01),
            ..Default::default()
        });
        new_config_msg.till_timestamp = Some(1_001_000_11) ;
        new_config_msg.init_timestamp = None;
        update_config_msg = UpdateConfig {  new_config : new_config_msg.clone() };
        res_f = execute(deps.as_mut(), env.clone(), info.clone(), update_config_msg.clone() );
        assert_generic_error_message(res_f,"Invalid till timestamp");

        // *** Test : Error (Updating till_timestamp) :: Reason : New till_timestamp < config.init_timestamp ****
        env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_000_000_11),
            ..Default::default()
        });
        new_config_msg.till_timestamp = Some(1_000_000_14) ;
        update_config_msg = UpdateConfig {  new_config : new_config_msg.clone() };
        res_f = execute(deps.as_mut(), env.clone(), info.clone(), update_config_msg.clone() );
        assert_generic_error_message(res_f,"Invalid till timestamp"); 
        
        // *** Test : Error (Updating till_timestamp) :: Reason : New till_timestamp has already passed ****
        env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_000_000_19),
            ..Default::default()
        });
        new_config_msg.till_timestamp = Some(1_000_000_17) ;
        update_config_msg = UpdateConfig {  new_config : new_config_msg.clone() };
        res_f = execute(deps.as_mut(), env.clone(), info.clone(), update_config_msg.clone() );
        assert_generic_error_message(res_f,"Invalid till timestamp"); 
        
        // *** Test : Should update till_timestamp  correctly ****
        new_config_msg.till_timestamp = Some(1_000_001_00) ;
        update_config_msg = UpdateConfig {  new_config : new_config_msg.clone() };
        res_s = execute(deps.as_mut(), env.clone(), info.clone(), update_config_msg.clone() ).unwrap();
        assert_eq!( res_s.attributes, vec![attr("action", "staking::ExecuteMsg::UpdateConfig")] );
        config_ = CONFIG.load(&deps.storage).unwrap();
        assert_eq!(1_000_001_00, config_.till_timestamp);
    }


    #[test]
    fn test_bond_tokens() { 

    }







    // pub struct MockEnvParams {
    //     pub block_time: Timestamp,
    //     pub block_height: u64,
    // }
    
    // impl Default for MockEnvParams {
    //     fn default() -> Self {
    //         MockEnvParams {
    //             block_time: Timestamp::from_nanos(1_571_797_419_879_305_533),
    //             block_height: 1,
    //         }
    //     }
    // }


    // fn th_setup(contract_balances: &[Coin]) -> OwnedDeps<MockStorage, MockApi, MarsMockQuerier> {
    //     let mut deps = mock_dependencies(contract_balances);
    //     let env = mock_env(MockEnvParams::default());
    //     let info = mock_info("owner");
    //     let config = CreateOrUpdateConfig {
    //         owner: Some("owner".to_string()),
    //         address_provider_address: Some("address_provider".to_string()),
    //         insurance_fund_fee_share: Some(Decimal::from_ratio(5u128, 10u128)),
    //         treasury_fee_share: Some(Decimal::from_ratio(3u128, 10u128)),
    //         ma_token_code_id: Some(1u64),
    //         close_factor: Some(Decimal::from_ratio(1u128, 2u128)),
    //     };
    //     let msg = InstantiateMsg { config };
    //     instantiate(deps.as_mut(), env, info, msg).unwrap();
    //     deps
    // }

    
    // mock_env replacement for cosmwasm_std::testing::mock_env
    // pub fn mock_env(mock_env_params: MockEnvParams) -> Env {
    //     Env {
    //         block: BlockInfo {
    //             height: mock_env_params.block_height,
    //             time: mock_env_params.block_time,
    //             chain_id: "cosmos-testnet-14002".to_string(),
    //         },
    //         contract: ContractInfo {
    //             address: Addr::unchecked(MOCK_CONTRACT_ADDR),
    //         },
    //     }
    // }

    // quick mock info with just the sender
    // TODO: Maybe this one does not make sense given there's a very smilar helper in cosmwasm_std
    // pub fn mock_info(sender: &str) -> MessageInfo {
    //     MessageInfo {
    //         sender: Addr::unchecked(sender),
    //         funds: vec![],
    //     }
    // }

    // mock_dependencies replacement for cosmwasm_std::testing::mock_dependencies
    // pub fn mock_dependencies(
    //     contract_balance: &[Coin],
    // ) -> OwnedDeps<MockStorage, MockApi, MockQuerier> {
    //     let contract_addr = Addr::unchecked(MOCK_CONTRACT_ADDR);
    //     let custom_querier: MockQuerier = MockQuerier::new(&[(
    //         &contract_addr.to_string(),
    //         contract_balance,
    //     )]);

    //     OwnedDeps {
    //         storage: MockStorage::default(),
    //         api: MockApi::default(),
    //         querier: custom_querier,
    //     }
    // }

    // Assert StdError::GenericErr message with expected_msg
    // pub fn assert_generic_error_message<T>(response: StdResult<T>, expected_msg: &str) {
    //     match response {
    //         Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, expected_msg),
    //         Err(other_err) => panic!("Unexpected error: {:?}", other_err),
    //         Ok(_) => panic!("SHOULD NOT ENTER HERE!"),
    //     }
    // }



}

