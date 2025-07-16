use anyhow::Result;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

/// Calculate output amount for a constant product AMM swap
/// Uses the formula: output = (input * output_reserve) / (input_reserve + input)
/// Accounts for fees by reducing input amount
pub fn calculate_output_amount(
    input_amount: u64,
    input_reserve: u64,
    output_reserve: u64,
    fee_percent: Decimal,
) -> Result<u64> {
    if input_reserve == 0 || output_reserve == 0 {
        return Ok(0);
    }

    // Apply fee (reduce input by fee percentage)
    let fee_multiplier = Decimal::ONE - fee_percent;
    let input_after_fee = Decimal::from(input_amount) * fee_multiplier;
    
    // Constant product formula: x * y = k
    let input_reserve_decimal = Decimal::from(input_reserve);
    let output_reserve_decimal = Decimal::from(output_reserve);
    
    let numerator = input_after_fee * output_reserve_decimal;
    let denominator = input_reserve_decimal + input_after_fee;
    
    if denominator.is_zero() {
        return Ok(0);
    }
    
    let output = numerator / denominator;
    
    Ok(output.to_u64().unwrap_or(0))
}

/// Calculate price impact for a swap
/// Price impact = (old_price - new_price) / old_price
pub fn calculate_price_impact(
    input_amount: u64,
    input_reserve: u64,
    output_reserve: u64,
) -> Result<Decimal> {
    if input_reserve == 0 || output_reserve == 0 {
        return Ok(Decimal::ZERO);
    }

    let old_price = Decimal::from(output_reserve) / Decimal::from(input_reserve);
    
    let new_input_reserve = input_reserve + input_amount;
    let new_output_reserve = output_reserve - calculate_output_amount(
        input_amount, 
        input_reserve, 
        output_reserve, 
        Decimal::ZERO // No fee for price impact calculation
    )?;
    
    if new_input_reserve == 0 {
        return Ok(Decimal::ZERO);
    }
    
    let new_price = Decimal::from(new_output_reserve) / Decimal::from(new_input_reserve);
    
    if old_price.is_zero() {
        return Ok(Decimal::ZERO);
    }
    
    let price_impact = (old_price - new_price) / old_price;
    
    Ok(price_impact.abs())
}

/// Calculate slippage for a trade
/// Slippage is the difference between expected and actual execution price
pub fn calculate_slippage(
    expected_output: u64,
    actual_reserve: u64,
    max_slippage_percent: f64,
) -> Result<Decimal> {
    let max_slippage = Decimal::from_f64_retain(max_slippage_percent / 100.0)
        .unwrap_or(Decimal::from_f64_retain(0.01).unwrap()); // Default 1%
    
    // This is a simplified slippage calculation
    // In practice, you'd compare expected vs actual prices
    let reserve_impact = Decimal::from(expected_output) / Decimal::from(actual_reserve);
    
    // Higher reserve impact = higher slippage
    let calculated_slippage = reserve_impact * Decimal::from_f64_retain(0.1).unwrap();
    
    Ok(calculated_slippage.min(max_slippage))
}

/// Calculate the optimal trade size based on price impact tolerance
pub fn calculate_optimal_trade_size(
    input_reserve: u64,
    output_reserve: u64,
    max_price_impact: Decimal,
) -> Result<u64> {
    if input_reserve == 0 || output_reserve == 0 {
        return Ok(0);
    }

    // Binary search for optimal trade size
    let mut low = 1u64;
    let mut high = input_reserve / 10; // Start with 10% of reserve as max
    let mut optimal_size = 0u64;
    
    while low <= high {
        let mid = (low + high) / 2;
        let price_impact = calculate_price_impact(mid, input_reserve, output_reserve)?;
        
        if price_impact <= max_price_impact {
            optimal_size = mid;
            low = mid + 1;
        } else {
            high = mid - 1;
        }
    }
    
    Ok(optimal_size)
}

/// Calculate profit after fees and slippage
pub fn calculate_net_profit(
    input_amount: u64,
    output_amount: u64,
    transaction_fee: u64,
    gas_fee: u64,
) -> i64 {
    let gross_profit = output_amount as i64 - input_amount as i64;
    gross_profit - transaction_fee as i64 - gas_fee as i64
}

/// Calculate the break-even price for an arbitrage opportunity
pub fn calculate_break_even_price(
    input_amount: u64,
    total_fees: Decimal,
) -> Result<Decimal> {
    let input_decimal = Decimal::from(input_amount);
    let fee_amount = input_decimal * total_fees;
    
    // Break-even price is input + fees
    Ok(input_decimal + fee_amount)
}

/// Calculate compound annual growth rate (CAGR) for profit tracking
pub fn calculate_cagr(
    initial_value: f64,
    final_value: f64,
    time_periods: f64,
) -> f64 {
    if initial_value <= 0.0 || time_periods <= 0.0 {
        return 0.0;
    }
    
    (final_value / initial_value).powf(1.0 / time_periods) - 1.0
}

/// Calculate Sharpe ratio for risk-adjusted returns
pub fn calculate_sharpe_ratio(
    returns: &[f64],
    risk_free_rate: f64,
) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }
    
    let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
    let excess_return = mean_return - risk_free_rate;
    
    if returns.len() < 2 {
        return 0.0;
    }
    
    let variance = returns.iter()
        .map(|r| (r - mean_return).powi(2))
        .sum::<f64>() / (returns.len() - 1) as f64;
    
    let std_dev = variance.sqrt();
    
    if std_dev == 0.0 {
        return 0.0;
    }
    
    excess_return / std_dev
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_output_amount() {
        let input_amount = 1000;
        let input_reserve = 100000;
        let output_reserve = 200000;
        let fee_percent = Decimal::from_f64_retain(0.003).unwrap(); // 0.3%
        
        let output = calculate_output_amount(input_amount, input_reserve, output_reserve, fee_percent).unwrap();
        assert!(output > 0);
        assert!(output < 2000); // Should be less than 1:1 ratio due to slippage and fees
    }

    #[test]
    fn test_calculate_price_impact() {
        let input_amount = 1000;
        let input_reserve = 100000;
        let output_reserve = 200000;
        
        let price_impact = calculate_price_impact(input_amount, input_reserve, output_reserve).unwrap();
        assert!(price_impact >= Decimal::ZERO);
        assert!(price_impact < Decimal::ONE); // Should be less than 100%
    }

    #[test]
    fn test_calculate_net_profit() {
        let input_amount = 1000;
        let output_amount = 1100;
        let transaction_fee = 20;
        let gas_fee = 5;
        
        let net_profit = calculate_net_profit(input_amount, output_amount, transaction_fee, gas_fee);
        assert_eq!(net_profit, 75); // 1100 - 1000 - 20 - 5 = 75
    }
}
