use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Addr};
use cw_storage_plus::{Item, Map};

pub const CONFIG: Item<Config> = Item::new("config");
pub const CLAIMEES: Map< &[u8], bool> = Map::new("claimed");

//----------------------------------------------------------------------------------------
// Storage types
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Config {
    pub mars_address: Addr,
    pub admin: Addr,
    pub terra_merkle_roots: Vec<String>,
    pub evm_merkle_roots: Vec<String>,
    pub till_timestamp: u64, 
}
