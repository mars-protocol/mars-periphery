interface CouncilInitMsg {
  config: {
    address_provider_address?: string
    proposal_voting_period: number
    proposal_effective_delay: number
    proposal_expiration_period: number
    proposal_required_deposit: string
    proposal_required_quorum: string
    proposal_required_threshold: string
  }
}

interface StakingInitMsg {
  config: {
    owner?: string
    address_provider_address?: string
    terraswap_factory_address?: string
    terraswap_max_spread: string
    cooldown_duration: number
    unstake_window: number
  }
}

interface InsuranceFundInitMsg {
  owner?: string
  terraswap_factory_address?: string
  terraswap_max_spread: string
}

interface RedBankInitMsg {
  config: {
    owner?: string
    address_provider_address?: string
    insurance_fund_fee_share: string
    treasury_fee_share: string
    ma_token_code_id?: number
    close_factor: string
  }
}

interface Asset {
  denom?: string
  symbol?: string
  contract_addr?: string
  initial_borrow_rate: string
  min_borrow_rate: string
  max_borrow_rate: string
  max_loan_to_value: string
  reserve_factor: string
  maintenance_margin: string
  liquidation_bonus: string
  kp_1: string
  optimal_utilization_rate: string
  kp_augmentation_threshold: string
  kp_2: string
}

interface Config {
  councilInitMsg: CouncilInitMsg
  stakingInitMsg: StakingInitMsg
  insuranceFundInitMsg: InsuranceFundInitMsg
  redBankInitMsg: RedBankInitMsg
  initialAssets: Asset[]
  mirFarmingStratContractAddress: string | undefined
  ancFarmingStratContractAddress: string | undefined
  cw20_code_id: number | undefined
}
