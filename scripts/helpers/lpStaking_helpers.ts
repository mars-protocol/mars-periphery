import {executeContract,  queryContract, toEncodedBinary} from "./helpers.js"
  import { LCDClient, LocalTerra, Wallet, MnemonicKey, Int } from "@terra-money/terra.js"

//-----------------------------------------------------
// ------ ExecuteContract :: Function signatures ------
// - stake_LP_Tokens(terra, wallet, stakingContractAddress, lpTokenAddress, amount) --> STAKE LP TOKENS
// - unstake_LP_Tokens(terra, wallet, stakingContractAddress, marsTokenAddress, amount) --> UN-STAKE LP TOKENS
// - claim_LPstaking_rewards(terra, wallet, stakingContractAddress, marsTokenAddress) --> CLAIM $MARS REWARDS
// - update_LP_Staking_config(terra, wallet, stakingContractAddress, owner, address_provider, 
//                          staking_token, init_timestamp, till_timestamp, cycle_rewards, cycle_duration, reward_increase) --> UPDATE CONFIG
//------------------------------------------------------
//------------------------------------------------------
// ----------- Queries :: Function signatures ----------
// - query_LPStaking_config(terra, stakingContractAddress) --> Returns configuration
// - query_LPStaking_state(terra, stakingContractAddress, timestamp) --> Returns contract's global state
// - query_LPStaking_stakerInfo(terra, stakingContractAddress, stakerAddress, timestamp) --> Returns user's position info
// - query_LPStaking_timestamp(terra, stakingContractAddress) --> Returns timestamp
//------------------------------------------------------


// LP STAKING :: STAKE LP TOKENS
export async function stake_LP_Tokens(terra: LCDClient, wallet:Wallet, stakingContractAddress:string ,lpTokenAddress: string, amount: number) {
    let staking_msg = {
                        "send" : {
                            "contract": stakingContractAddress,
                            "amount": amount.toString(),
                            "msg": toEncodedBinary({"bond":{}}),
                        }
                      };
    let resp = await executeContract(terra, wallet, lpTokenAddress, staking_msg );
    console.log( (amount / 1e6).toString() + " LP Tokens staked successfully by " + wallet.key.accAddress);
}  

// LP STAKING :: UN-STAKE LP TOKENS
export async function unstake_LP_Tokens(terra: LCDClient, wallet:Wallet, stakingContractAddress:string, marsTokenAddress:string, amount:number) {
    let mars_balance = await queryContract(terra, marsTokenAddress, {"balance": {"address": wallet.key.accAddress}} );
    let unstake_msg = { "unbond":{"amount":amount.toString()} };
    let resp = await executeContract(terra, wallet, stakingContractAddress, unstake_msg );
    let new_mars_balance = await queryContract(terra, marsTokenAddress, {"balance": {"address": wallet.key.accAddress}} );
    console.log(" LP Tokens unstaked. " + (new_mars_balance - mars_balance).toString() + " $MARS (scale = 1e6) claimed as rewards" );
}  


// LP STAKING :: CLAIM $MARS REWARDS
export async function claim_LPstaking_rewards(terra: LCDClient, wallet:Wallet, stakingContractAddress:string, marsTokenAddress:string) {
    let mars_balance = await queryContract(terra, marsTokenAddress, {"balance": {"address": wallet.key.accAddress}} );
    let claim_msg = { "claim":{} };
    let resp = await executeContract(terra, wallet, stakingContractAddress, claim_msg );
    let new_mars_balance = await queryContract(terra, marsTokenAddress, {"balance": {"address": wallet.key.accAddress}} );
    console.log((new_mars_balance - mars_balance).toString() + " $MARS (scale = 1e6) claimed as LP Staking rewards" );
}  


// UPDATE CONFIGURATION
export async function update_LP_Staking_config(
        terra: LCDClient, 
        wallet:Wallet, 
        stakingContractAddress:string ,
        owner: null, 
        address_provider: null, 
        staking_token: null, 
        init_timestamp: null, 
        till_timestamp: null, 
        cycle_rewards: null, 
        cycle_duration: null, 
        reward_increase: null
    ) {
    let config_msg = { "update_config" : {  "owner" : owner,
                                            "address_provider" : address_provider,
                                            "staking_token" : staking_token,
                                            "init_timestamp" : init_timestamp,
                                            "till_timestamp" : till_timestamp,
                                            "cycle_rewards" : cycle_rewards,
                                            "cycle_duration" : cycle_duration,
                                            "reward_increase" : reward_increase
                                        }
                    };
    let resp = await executeContract(terra, wallet, stakingContractAddress, config_msg );
    console.log(" LP STAKING CONTRACT : Configuration successfully updated");
}  

// Returns configuration
export async function query_LPStaking_config(terra: LCDClient, stakingContractAddress:string) {
    return await queryContract(terra, stakingContractAddress, {"config":{}});
}

// Returns contract's global state
export async function query_LPStaking_state(terra: LCDClient, stakingContractAddress:string, timestamp: null) {
    return await queryContract(terra, stakingContractAddress, {"state":{"timestamp":timestamp}});
}

// Returns user's position info
export async function query_LPStaking_stakerInfo(terra: LCDClient, stakingContractAddress:string, stakerAddress: string, timestamp: null) {
    return await queryContract(terra, stakingContractAddress, {"staker_info": {"staker":stakerAddress, "timestamp":timestamp} } );
}

// Returns timestamp
export async function query_LPStaking_timestamp(terra: LCDClient, stakingContractAddress:string) {
    return await queryContract(terra, stakingContractAddress, {"timestamp":{}});
}


