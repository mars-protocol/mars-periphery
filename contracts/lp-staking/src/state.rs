use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{Addr,StdError, StdResult};
use cw_storage_plus::{Item, Map};

//----------------------------------------------------------------------------------------
// Struct's :: Contract State
//----------------------------------------------------------------------------------------

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATE: Item<State> = Item::new("state");
pub const STAKER_INFO: Map<&Addr, StakerInfo> = Map::new("staker");


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// Account who can update config
    pub owner: Addr,
    /// Contract used to query addresses related to red-bank (MARS Token)
    pub address_provider: Addr,
    ///  MARS-UST LP token address - accepted by the contract via Cw20ReceiveMsg function 
    pub staking_token: Addr,
    /// Timestamp from which MARS Rewards will start getting accrued against the staked LP tokens
    pub init_timestamp: u64,               
    /// Timestamp till which MARS Rewards will be accrued. No staking rewards are accrued beyond this timestamp
    pub till_timestamp: u64,    
    // Cycle duration in timestamps
    pub cycle_duration: u64,   
    /// Percent increase in Rewards per cycle        
    pub reward_increase: Decimal256,     
}

impl Config {
    pub fn validate(&self) -> StdResult<()> { 
        if (&self.init_timestamp < &self.till_timestamp) && (&self.reward_increase < &Decimal256::one()) {
            return Ok(());
        }
        return Err(StdError::generic_err("Invalid configuration"));
    }
}



#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    /// Timestamp at which the current reward cycle begin
    pub cycle_init_timestamp: u64,      
    /// MARS rewards to be distributed in the current cycle      
    pub current_cycle_rewards: Uint256,        
    /// Timestamp at which the global_reward_index was last updated
    pub last_distributed: u64,
    /// Total number of MARS-UST LP tokens deposited in the contract
    pub total_bond_amount: Uint256,
    ///  total MARS rewards / total_bond_amount ratio. Used to calculate MARS rewards accured over time elapsed
    pub global_reward_index: Decimal256,
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakerInfo {
    /// MARS-UST LP tokens deposited by the user
    pub bond_amount: Uint256,
    /// MARS rewards / bond_amount ratio.  Used to calculate MARS rewards accured over time elapsed
    pub reward_index: Decimal256,
    /// Pending MARS rewards which are yet to be claimed
    pub pending_reward: Uint256,
}

impl Default for StakerInfo {
    fn default() -> Self {
        StakerInfo {
            reward_index: Decimal256::one(),
            bond_amount: Uint256::zero(),
            pending_reward: Uint256::zero(),
        }
    }
}
