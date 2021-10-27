use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATE: Item<State> = Item::new("state");
pub const USERS: Map<&Addr, UserInfo> = Map::new("users");

//----------------------------------------------------------------------------------------
// Storage types
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Config {
    /// Account who can update config
    pub owner: Addr,
    ///  MARS token address
    pub mars_token_address: Addr,
    /// Airdrop Contract address
    pub airdrop_contract_address: Addr,
    /// Lockdrop Contract address
    pub lockdrop_contract_address: Addr,
    ///  MARS-UST LP Pool address
    pub astroport_lp_pool: Addr,
    ///  MARS-UST LP Token address
    pub lp_token_address: Addr,
    ///  Astroport Generator contract with which MARS-UST LP Tokens are staked
    pub generator_contract: Addr,
    /// Total MARS token rewards to be used to incentivize boostrap auction participants
    pub mars_rewards: Uint256,
    /// Number of seconds over which MARS incentives are vested
    pub mars_vesting_duration: u64,
    ///  Number of seconds over which LP Tokens are vested
    pub lp_tokens_vesting_duration: u64,
    /// Timestamp since which MARS / UST deposits will be allowrd
    pub init_timestamp: u64,
    /// Number of seconds post init_timestamp during which deposits / withdrawals will be allowed
    pub deposit_window: u64,
    /// Number of seconds post deposit_window completion during which only withdrawals are allowed
    pub withdrawal_window: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct State {
    /// Total MARS tokens delegated to the contract by lockdrop participants / airdrop recepients
    pub total_mars_deposited: Uint256,
    /// Total UST deposited in the contract
    pub total_ust_deposited: Uint256,
    /// Total LP shares minted post liquidity addition to the MARS-UST Pool
    pub lp_shares_minted: Uint256,
    /// Number of LP shares that have been withdrawn as they unvest
    pub lp_shares_withdrawn: Uint256,
    /// MARS--UST LP Shares currently staked with the Staking contract
    pub are_staked: bool,
    /// Timestamp at which liquidity was added to the MARS-UST LP Pool
    pub pool_init_timestamp: u64,
    /// index used to keep track of LP staking rewards and distribute them proportionally among the auction participants
    pub global_reward_index: Decimal256,
}

impl Default for State {
    fn default() -> Self {
        State {
            total_mars_deposited: Uint256::zero(),
            total_ust_deposited: Uint256::zero(),
            lp_shares_minted: Uint256::zero(),
            lp_shares_withdrawn: Uint256::zero(),
            pool_init_timestamp: 0u64,
            are_staked: false,
            global_reward_index: Decimal256::zero(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfo {
    // Total MARS Tokens delegated by the user
    pub mars_deposited: Uint256,
    // Total UST deposited by the user
    pub ust_deposited: Uint256,
    // Withdrawal counter to capture if the user already withdrew UST during the "only withdrawals" window
    pub withdrawl_counter: bool,
    // User's LP share balance
    pub lp_shares: Uint256,
    // LP shares withdrawn by the user
    pub withdrawn_lp_shares: Uint256,
    // User's MARS rewards for participating in the auction
    pub total_auction_incentives: Uint256,
    // MARS rewards withdrawn by the user
    pub withdrawn_auction_incentives: Uint256,
    // ASTRO staking incentives (LP token staking) withdrawn by the user
    pub withdrawn_staking_incentives: Uint256,
    // Index used to calculate user's staking rewards
    pub user_reward_index: Decimal256,
}

impl Default for UserInfo {
    fn default() -> Self {
        UserInfo {
            mars_deposited: Uint256::zero(),
            ust_deposited: Uint256::zero(),
            withdrawl_counter: false,
            lp_shares: Uint256::zero(),
            withdrawn_lp_shares: Uint256::zero(),
            total_auction_incentives: Uint256::zero(),
            withdrawn_auction_incentives: Uint256::zero(),
            withdrawn_staking_incentives: Uint256::zero(),
            user_reward_index: Decimal256::zero(),
        }
    }
}
