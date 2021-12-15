use cosmwasm_std::{Addr, Uint128};
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
    /// Merkle roots used to verify is a terra user is eligible for the airdrop
    pub merkle_roots: Vec<String>,
    /// Timestamp since which MARS airdrops can be delegated to bootstrap auction contract
    pub from_timestamp: u64,
    /// Timestamp to which MARS airdrops can be claimed
    pub to_timestamp: u64,
    /// Bootstrap auction contract address
    pub auction_contract_address: Option<Addr>,
    /// Boolean value indicating if the users can withdraw their MARS airdrop tokens or not
    /// This value is updated in the same Tx in which Liquidity is added to the LP Pool
    pub are_claims_enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct State {
    /// Total MARS issuance used as airdrop incentives
    pub total_airdrop_size: Uint128,
    /// Total MARS tokens that have been delegated to the bootstrap auction pool
    pub total_delegated_amount: Uint128,
    /// Total MARS tokens that are yet to be claimed by the users
    pub unclaimed_tokens: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfo {
    /// Total MARS airdrop tokens claimable by the user
    pub claimed_amount: Uint128,
    /// MARS tokens delegated to the bootstrap auction contract to add to the user's position
    pub delegated_amount: Uint128,
    /// Boolean value indicating if the user has withdrawn the remaining MARS tokens
    pub tokens_withdrawn: bool,
}

impl Default for UserInfo {
    fn default() -> Self {
        UserInfo {
            claimed_amount: Uint128::zero(),
            delegated_amount: Uint128::zero(),
            tokens_withdrawn: false,
        }
    }
}
