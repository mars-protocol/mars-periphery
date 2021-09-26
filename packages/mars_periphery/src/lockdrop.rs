use cosmwasm_std::{to_binary, Addr, CosmosMsg, StdResult, WasmMsg};

use cosmwasm_bignumber::{Decimal256, Uint256};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// Account who can update config
    pub owner: String,
    /// Contract used to query addresses related to red-bank (MARS Token)
    pub address_provider: Option<String>,
    ///  maUST token address - Minted upon UST deposits into red bank
    pub ma_ust_token: Option<String>,
    /// Timestamp till when deposits can be made
    pub init_timestamp: u64,
    /// Number of seconds for which lockup deposits will be accepted
    pub deposit_window: u64,
    /// Number of seconds for which lockup withdrawals will be allowed
    pub withdrawal_window: u64,
    /// Min. no. of days allowed for lockup
    pub min_duration: u64,
    /// Max. no. of days allowed for lockup
    pub max_duration: u64,
    /// "uusd" - Native token accepted by the contract for deposits
    pub denom: Option<String>,
    /// Lockdrop Reward multiplier
    pub weekly_multiplier: Option<Decimal256>,
    /// Total MARS lockdrop incentives to be distributed among the users
    pub lockdrop_incentives: Option<Uint256>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UpdateConfigMsg {
    /// Account who can update config
    pub owner: Option<String>,
    /// Contract used to query addresses related to red-bank (MARS Token)
    pub address_provider: Option<String>,
    ///  maUST token address - Minted upon UST deposits into red bank
    pub ma_ust_token: Option<String>,
    /// Timestamp till when deposits can be made
    pub init_timestamp: Option<u64>,
    /// Number of seconds for which lockup deposits will be accepted
    pub deposit_window: Option<u64>,
    /// Number of seconds for which lockup withdrawals will be allowed
    pub withdrawal_window: Option<u64>,
    /// Min. no. of days allowed for lockup
    pub min_duration: Option<u64>,
    /// Max. no. of days allowed for lockup
    pub max_duration: Option<u64>,
    /// Lockdrop Reward multiplier
    pub weekly_multiplier: Option<Decimal256>,
    /// Total MARS lockdrop incentives to be distributed among the users
    pub lockdrop_incentives: Option<Uint256>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    DepositUst {
        duration: u64,
    },
    WithdrawUst {
        duration: u64,
        amount: Uint256,
    },
    Unlock {
        duration: u64,
    },
    ClaimRewards {},
    UpdateConfig {
        new_config: UpdateConfigMsg,
    },
    DepositUstInRedBank {},
    /// Callbacks; only callable by the contract itself.
    Callback(CallbackMsg),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CallbackMsg {
    UpdateStateOnRedBankDeposit {
        prev_ma_ust_balance: Uint256,
    },
    UpdateStateOnClaim {
        user: Addr,
        prev_xmars_balance: Uint256,
    },
    DissolvePosition {
        user: Addr,
        duration: u64,
    },
}

// Modified from
// https://github.com/CosmWasm/cosmwasm-plus/blob/v0.2.3/packages/cw20/src/receiver.rs#L15
impl CallbackMsg {
    pub fn to_cosmos_msg(&self, contract_addr: &Addr) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: String::from(contract_addr),
            msg: to_binary(&ExecuteMsg::Callback(self.clone()))?,
            funds: vec![],
        }))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    State {},
    UserInfo { address: String },
    LockUpInfo { address: String, duration: u64 },
    LockUpInfoWithId { lockup_id: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    /// Account who can update config
    pub owner: String,
    /// Contract used to query addresses related to red-bank (MARS Token)
    pub address_provider: String,
    ///  maUST token address - Minted upon UST deposits into red bank
    pub ma_ust_token: String,
    /// Timestamp till when deposits can be made
    pub init_timestamp: u64,
    /// Number of seconds for which lockup deposits will be accepted
    pub deposit_window: u64,
    /// Number of seconds for which lockup withdrawals will be allowed
    pub withdrawal_window: u64,
    /// Min. no. of weeks allowed for lockup
    pub min_duration: u64,
    /// Max. no. of weeks allowed for lockup
    pub max_duration: u64,
    /// Lockdrop Reward multiplier
    pub multiplier: Decimal256,
    /// Total MARS lockdrop incentives to be distributed among the users
    pub lockdrop_incentives: Uint256,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GlobalStateResponse {
    /// Total UST deposited at the end of Lockdrop window. This value remains unchanged post the lockdrop window
    pub final_ust_locked: Uint256,
    /// maUST minted at the end of Lockdrop window upon UST deposit in red bank. This value remains unchanged post the lockdrop window
    pub final_maust_locked: Uint256,
    /// UST deposited in the contract. This value is updated real-time upon each UST deposit / unlock
    pub total_ust_locked: Uint256,
    /// maUST held by the contract. This value is updated real-time upon each maUST withdrawal from red bank
    pub total_maust_locked: Uint256,
    /// Total weighted deposits
    pub total_deposits_weight: Uint256,
    /// Ratio of MARS rewards accured to total_maust_locked. Used to calculate MARS incentives accured by each user
    pub global_reward_index: Decimal256,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfoResponse {
    pub total_ust_locked: Uint256,
    pub total_maust_locked: Uint256,
    pub lockup_position_ids: Vec<String>,
    pub is_lockdrop_claimed: bool,
    pub reward_index: Decimal256,
    pub pending_xmars: Uint256,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LockUpInfoResponse {
    /// Lockup Duration
    pub duration: u64,
    /// UST locked as part of this lockup position
    pub ust_locked: Uint256,
    /// MA-UST share
    pub maust_balance: Uint256,
    /// Lockdrop incentive distributed to this position
    pub lockdrop_reward: Uint256,
    /// Timestamp beyond which this position can be unlocked
    pub unlock_timestamp: u64,
}