use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Addr, Uint128 };
use cw_storage_plus::{Item, Map};

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATE: Item<State> = Item::new("state");
pub const USERS: Map< &Addr, UserInfo> = Map::new("users");
pub const CLAIMEES: Map<&[u8], IsClaimed> = Map::new("claimed");

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
    pub terra_merkle_roots: Vec<String>,
    /// Merkle roots used to verify is an evm user is eligible for the airdrop 
    pub evm_merkle_roots: Vec<String>,
    /// Timestamp since which MARS airdrops can be delegated to boostrap auction contract
    pub from_timestamp: u64, 
    /// Timestamp till which MARS airdrops can be claimed 
    pub till_timestamp: u64, 
    /// Boostrap auction contract address
    pub boostrap_auction_address: Addr,
    /// Boolean value indicating if the users can withdraw their MARS airdrop tokens or not
    /// This value is updated in the same Tx in which Liquidity is added to the LP Pool
    pub are_claims_allowed: bool
}



#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct State {
    /// Total MARS issuance used as airdrop incentives
    pub total_airdrop_size: Uint128, 
    /// Total MARS tokens that have been delegated to the boostrap auction pool
    pub tokens_used_for_auction: Uint128, 
    /// Total MARS tokens that are yet to be claimed by the users
    pub unclaimed_tokens: Uint128 
}


impl Default for State {
    fn default() -> Self {
        State {
            total_airdrop_size: Uint128::zero(),
            tokens_used_for_auction: Uint128::zero(),
            unclaimed_tokens: Uint128::zero()
        }
    }
}




#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfo {
    /// Total MARS airdrop tokens claimable by the user
    pub airdrop_amount: Uint128,
    /// MARS tokens delegated to the bootstrap auction contract to add to the user's position
    pub tokens_used_for_auction: Uint128,
    /// Boolean value indicating is the user has claimed the remaning MARS tokens or not
    pub tokens_claimed: Uint128
}

impl Default for UserInfo {
    fn default() -> Self {
        UserInfo {
            airdrop_amount: Uint128::zero(),
            tokens_used_for_auction: Uint128::zero(),
            tokens_claimed: Uint128::zero()
        }
    }
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct IsClaimed {
    pub is_claimed: bool,
}

impl Default for IsClaimed {
    fn default() -> Self {
        IsClaimed { is_claimed: false }
    }
}