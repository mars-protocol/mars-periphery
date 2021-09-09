use crate::state::{Market, CONFIG};
use cosmwasm_std::{Decimal, DepsMut, Env, StdError, StdResult, Uint128};
use mars::asset::AssetType;
use mars::helpers::cw20_get_balance;
use mars::interest_rate_models::InterestRateModel;
use mars::math::{decimal_multiplication, reverse_decimal};
use std::str;

/// Scaling factor used to keep more precision during division / multiplication by index.
pub const SCALING_FACTOR: u128 = 1_000_000;

const SECONDS_PER_YEAR: u64 = 31536000u64;

/// Updates market indices and protocol_income by applying current interest rates on the time between
/// last interest update and current block.
/// Note it does not save the market to the store (that is left to the caller)
pub fn apply_accumulated_interests(env: &Env, market: &mut Market) {
    let current_timestamp = env.block.time.seconds();
    // Since interest is updated on every change on scale debt, multiplying the scaled debt for each
    // of the indices and subtracting them returns the accrued borrow interest for the period since
    // when the indices were last updated and the current point in time.
    let previous_borrow_index = market.borrow_index;

    if market.interests_last_updated < current_timestamp {
        let time_elapsed = current_timestamp - market.interests_last_updated;

        if market.borrow_rate > Decimal::zero() {
            market.borrow_index = calculate_applied_linear_interest_rate(
                market.borrow_index,
                market.borrow_rate,
                time_elapsed,
            );
        }
        if market.liquidity_rate > Decimal::zero() {
            market.liquidity_index = calculate_applied_linear_interest_rate(
                market.liquidity_index,
                market.liquidity_rate,
                time_elapsed,
            );
        }
        market.interests_last_updated = current_timestamp;
    }

    let previous_debt_total = get_descaled_amount(market.debt_total_scaled, previous_borrow_index);
    let new_debt_total = get_descaled_amount(market.debt_total_scaled, market.borrow_index);

    let interest_accrued = if new_debt_total > previous_debt_total {
        new_debt_total - previous_debt_total
    } else {
        Uint128::zero()
    };

    let new_protocol_income_to_distribute = interest_accrued * market.reserve_factor;
    market.protocol_income_to_distribute += new_protocol_income_to_distribute;
}

pub fn calculate_applied_linear_interest_rate(
    index: Decimal,
    rate: Decimal,
    time_elapsed: u64,
) -> Decimal {
    let rate_factor = decimal_multiplication(
        rate,
        Decimal::from_ratio(Uint128::from(time_elapsed), Uint128::from(SECONDS_PER_YEAR)),
    );
    decimal_multiplication(index, Decimal::one() + rate_factor)
}

/// Scales the amount dividing by an index in order to compute interest rates. Before dividing,
/// the value is multiplied by SCALED_FACTOR for greater precision.
/// Example:
/// Current index is 10. We deposit 6.123456 UST (6123456 uusd). Scaled amount will be
/// 6123456 / 10 = 612345 so we loose some precision. In order to avoid this situation
/// we scale the amount by SCALING_FACTOR.
pub fn get_scaled_amount(amount: Uint128, index: Decimal) -> Uint128 {
    // Scale by SCALING_FACTOR to have better precision
    let scaled_amount = Uint128::from(amount.u128() * SCALING_FACTOR);
    // Different form for: scaled_amount / index
    scaled_amount * reverse_decimal(index)
}

/// Descales the amount introduced by `get_scaled_amount`. As interest rate is accumulated
/// the index used to descale the amount should be bigger than the one used to scale it.
pub fn get_descaled_amount(amount: Uint128, index: Decimal) -> Uint128 {
    // Multiply scaled amount by decimal (index)
    let result = amount * index;
    // Descale by SCALING_FACTOR which is introduced by `get_scaled_amount`
    result.checked_div(Uint128::from(SCALING_FACTOR)).unwrap()
}

/// Return applied interest rate for borrow index according to passed blocks
/// NOTE: Calling this function when interests for the market are up to date with the current block
/// and index is not, will use the wrong interest rate to update the index.
pub fn get_updated_borrow_index(market: &Market, block_time: u64) -> Decimal {
    if market.interests_last_updated < block_time {
        let time_elapsed = block_time - market.interests_last_updated;

        if market.borrow_rate > Decimal::zero() {
            let applied_interest_rate = calculate_applied_linear_interest_rate(
                market.borrow_index,
                market.borrow_rate,
                time_elapsed,
            );
            return applied_interest_rate;
        }
    }

    market.borrow_index
}

/// Return applied interest rate for liquidity index according to passed blocks
/// NOTE: Calling this function when interests for the market are up to date with the current block
/// and index is not, will use the wrong interest rate to update the index.
pub fn get_updated_liquidity_index(market: &Market, block_time: u64) -> Decimal {
    if market.interests_last_updated < block_time {
        let time_elapsed = block_time - market.interests_last_updated;

        if market.liquidity_rate > Decimal::zero() {
            let applied_interest_rate = calculate_applied_linear_interest_rate(
                market.liquidity_index,
                market.liquidity_rate,
                time_elapsed,
            );
            return applied_interest_rate;
        }
    }

    market.liquidity_index
}

/// Update interest rates for current liquidity and debt levels
/// Note it does not save the market to the store (that is left to the caller)
pub fn update_interest_rates(
    deps: &DepsMut,
    env: &Env,
    reference: &[u8],
    market: &mut Market,
    liquidity_taken: Uint128,
) -> StdResult<()> {
    let contract_current_balance = match market.asset_type {
        AssetType::Native => {
            let denom = str::from_utf8(reference);
            let denom = match denom {
                Ok(denom) => denom,
                Err(_) => return Err(StdError::generic_err("failed to encode denom into string")),
            };
            deps.querier
                .query_balance(env.contract.address.clone(), denom)?
                .amount
        }
        AssetType::Cw20 => {
            let cw20_addr = str::from_utf8(reference);
            let cw20_addr = match cw20_addr {
                Ok(cw20_addr) => cw20_addr,
                Err(_) => {
                    return Err(StdError::generic_err(
                        "failed to encode Cw20 address into string",
                    ))
                }
            };
            let cw20_addr = deps.api.addr_validate(cw20_addr)?;
            cw20_get_balance(&deps.querier, cw20_addr, env.contract.address.clone())?
        }
    };

    // Get protocol income to be deducted from liquidity (doesn't belong to the money market
    // anymore)
    let config = CONFIG.load(deps.storage)?;
    // NOTE: No check for underflow because this is done on config validations
    let protocol_income_minus_treasury_amount =
        (Decimal::one() - config.treasury_fee_share) * market.protocol_income_to_distribute;
    let liquidity_to_deduct_from_current_balance =
        liquidity_taken + protocol_income_minus_treasury_amount;

    let available_liquidity = if contract_current_balance < liquidity_to_deduct_from_current_balance
    {
        // liquidity_taken for deposit / repay is zero so we don't want to block these operations
        if !liquidity_taken.is_zero() {
            return Err(StdError::generic_err(
                "Protocol income to be distributed and liquidity taken cannot be greater than available liquidity",
            ));
        }
        Uint128::zero()
    } else {
        contract_current_balance - liquidity_to_deduct_from_current_balance
    };

    let total_debt = get_descaled_amount(
        market.debt_total_scaled,
        get_updated_borrow_index(market, env.block.time.seconds()),
    );
    let current_utilization_rate = if total_debt > Uint128::zero() {
        Decimal::from_ratio(total_debt, available_liquidity + total_debt)
    } else {
        Decimal::zero()
    };

    let (new_borrow_rate, new_liquidity_rate) =
        market.interest_rate_strategy.get_updated_interest_rates(
            current_utilization_rate,
            market.borrow_rate,
            market.reserve_factor,
        );
    market.borrow_rate = new_borrow_rate;
    market.liquidity_rate = new_liquidity_rate;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::interest_rates::calculate_applied_linear_interest_rate;
    use cosmwasm_std::Decimal;

    #[test]
    fn test_accumulated_index_calculation() {
        let index = Decimal::from_ratio(1u128, 10u128);
        let rate = Decimal::from_ratio(2u128, 10u128);
        let time_elapsed = 15768000; // half a year
        let accumulated = calculate_applied_linear_interest_rate(index, rate, time_elapsed);

        assert_eq!(accumulated, Decimal::from_ratio(11u128, 100u128));
    }
}
