use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::asset::{Asset, AssetInfo};
use crate::hook::InitHook;

use crate::factory::PairType;
use cosmwasm_std::{Addr, Decimal, Uint128};
use cw20::Cw20ReceiveMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// Asset infos
    pub asset_infos: [AssetInfo; 2],
    /// Token contract code id for initialization
    pub token_code_id: u64,
    /// Hook for post initialization
    pub init_hook: Option<InitHook>,
    /// Factory contract address
    pub factory_addr: Addr,
    /// Pair type
    pub pair_type: PairType,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    /// Post initize step to allow user to set controlled contract address after creating it
    PostInitialize {},
    /// ProvideLiquidity a user provides pool liquidity
    ProvideLiquidity {
        assets: [Asset; 2],
        slippage_tolerance: Option<Decimal>,
    },
    /// Swap an offer asset to the other
    Swap {
        offer_asset: Asset,
        belief_price: Option<Decimal>,
        max_spread: Option<Decimal>,
        to: Option<String>,
    },
    UpdateConfig {
        amp: Option<u64>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    /// Sell a given amount of asset
    Swap {
        belief_price: Option<Decimal>,
        max_spread: Option<Decimal>,
        to: Option<String>,
    },
    WithdrawLiquidity {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Pair {},
    Pool {},
    Share { amount: Uint128 },
    Simulation { offer_asset: Asset },
    ReverseSimulation { ask_asset: Asset },
    CumulativePrices {},
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolResponse {
    pub assets: [Asset; 2],
    pub total_share: Uint128,
}

/// SimulationResponse returns swap simulation response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct SimulationResponse {
    pub return_amount: Uint128,
    pub spread_amount: Uint128,
    pub commission_amount: Uint128,
}

/// ReverseSimulationResponse returns reverse swap simulation response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ReverseSimulationResponse {
    pub offer_amount: Uint128,
    pub spread_amount: Uint128,
    pub commission_amount: Uint128,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CumulativePricesResponse {
    pub assets: [Asset; 2],
    pub total_share: Uint128,
    pub price0_cumulative_last: Uint128,
    pub price1_cumulative_last: Uint128,
}

/// We currently take no arguments for migrations
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}
