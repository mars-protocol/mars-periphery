use cosmwasm_std::{Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: Option<String>,
    pub mars_token_address: Option<String>,
    pub terra_merkle_roots: Option<Vec<String>>,
    pub evm_merkle_roots: Option<Vec<String>>,    
    pub from_timestamp: Option<u64>,
    pub till_timestamp: Option<u64>,
    pub boostrap_auction_address: Option<String>,
    pub total_airdrop_size: Uint128
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Admin function to update the configuration parameteres
    UpdateConfig {
        new_config: InstantiateMsg,
    },
    /// Allows Terra users to claim their MARS Airdrop 
    ClaimByTerraUser {
        claim_amount: Uint128,
        merkle_proof: Vec<String>,
        root_index: u32
    },
    /// Allows EVM users to claim their MARS Airdrop 
    ClaimByEvmUser {
        eth_address: String,
        claim_amount: Uint128,
        merkle_proof: Vec<String>,
        root_index: u32,
        signature: String,
        signed_msg_hash: String
        
    },
    /// Allows users to delegate their MARS tokens to the LP Bootstrap auction contract 
    DelegateMarsAstroToBootstrapAuction {
        amount_to_delegate: Uint128
    },
    /// Allows users to withdraw their MARS tokens 
    WithdrawAirdropReward { },
    /// Admin function to facilitate transfer of the unclaimed MARS Tokens
    TransferUnclaimedTokens {
        recepient: String,
        amount: Uint128,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    State {},
    UserInfo {
        address: String,
     },
     HasUserClaimed { address: String },
     IsValidSignature {
        evm_address: String,
        evm_signature: String,
        signed_msg_hash: String,                
     },
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub mars_token_address: String,
    pub terra_merkle_roots: Vec<String>,
    pub evm_merkle_roots: Vec<String>,    
    pub from_timestamp: u64,
    pub till_timestamp: u64,
    pub boostrap_auction_address: String,
    pub are_claims_allowed: bool
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StateResponse {
    pub total_airdrop_size: Uint128,
    pub tokens_used_for_auction: Uint128,
    pub unclaimed_tokens: Uint128,
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfoResponse {
    pub airdrop_amount: Uint128,
    pub tokens_used_for_auction: Uint128,
    pub tokens_claimed: Uint128
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ClaimResponse {
    pub is_claimed: bool,
}



#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct SignatureResponse {
    pub is_valid: bool,
    pub public_key: String,
    pub recovered_address: String
}

