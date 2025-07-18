//! Mock data module for DEX testing when APIs are unavailable
//! This provides realistic test data for Orca, Raydium, and Phoenix DEXs

use crate::models::{Pool, TokenInfo};
use rust_decimal::Decimal;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use tracing::info;

/// Common Solana token mints for testing
pub struct CommonTokens;

impl CommonTokens {
    pub const SOL: &'static str = "So11111111111111111111111111111111111111112";
    pub const USDC: &'static str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
    pub const USDT: &'static str = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";
    pub const RAY: &'static str = "4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R";
    pub const ORCA: &'static str = "orcaEKTdK7LKz57vaAYr9QeNsVEPfiu6QeMU1kektZE";
    pub const MSOL: &'static str = "mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So";
    pub const BONK: &'static str = "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263";
}

/// Generate mock Orca pools with realistic data
pub fn generate_mock_orca_pools() -> Vec<Pool> {
    info!("Generating mock Orca pools for testing");
    
    vec![
        // SOL/USDC pool
        Pool {
            address: Pubkey::from_str("HJPjoWUrhoZzkNfRpHuieeFk9WcZWjwy6PBjZ81ngndJ").unwrap(),
            dex: "orca".to_string(),
            token_a: TokenInfo {
                mint: Pubkey::from_str(CommonTokens::SOL).unwrap(),
                symbol: "SOL".to_string(),
                decimals: 9,
                price_usd: Some(Decimal::from_f64_retain(95.50).unwrap()),
            },
            token_b: TokenInfo {
                mint: Pubkey::from_str(CommonTokens::USDC).unwrap(),
                symbol: "USDC".to_string(),
                decimals: 6,
                price_usd: Some(Decimal::from_f64_retain(1.00).unwrap()),
            },
            reserve_a: 125_000_000_000, // 125 SOL
            reserve_b: 12_000_000_000,   // 12,000 USDC
            fee_percent: Decimal::from_f64_retain(0.003).unwrap(), // 0.3%
            liquidity_usd: Decimal::from_f64_retain(24_000.0).unwrap(),
            last_updated: chrono::Utc::now(),
        },
        // RAY/USDC pool
        Pool {
            address: Pubkey::from_str("6UmmUiYoBjSrhakAobJw8BvkmJtDVxaeBtbt7rxWo1mg").unwrap(),
            dex: "orca".to_string(),
            token_a: TokenInfo {
                mint: Pubkey::from_str(CommonTokens::RAY).unwrap(),
                symbol: "RAY".to_string(),
                decimals: 6,
                price_usd: Some(Decimal::from_f64_retain(0.85).unwrap()),
            },
            token_b: TokenInfo {
                mint: Pubkey::from_str(CommonTokens::USDC).unwrap(),
                symbol: "USDC".to_string(),
                decimals: 6,
                price_usd: Some(Decimal::from_f64_retain(1.00).unwrap()),
            },
            reserve_a: 50_000_000_000,  // 50,000 RAY
            reserve_b: 42_500_000_000,  // 42,500 USDC
            fee_percent: Decimal::from_f64_retain(0.003).unwrap(),
            liquidity_usd: Decimal::from_f64_retain(85_000.0).unwrap(),
            last_updated: chrono::Utc::now(),
        },
        // ORCA/USDC pool
        Pool {
            address: Pubkey::from_str("2p7nYbtPBgtmY69NsE8DAW6szpRJn7tQvDnqvoEWQvjY").unwrap(),
            dex: "orca".to_string(),
            token_a: TokenInfo {
                mint: Pubkey::from_str(CommonTokens::ORCA).unwrap(),
                symbol: "ORCA".to_string(),
                decimals: 6,
                price_usd: Some(Decimal::from_f64_retain(3.20).unwrap()),
            },
            token_b: TokenInfo {
                mint: Pubkey::from_str(CommonTokens::USDC).unwrap(),
                symbol: "USDC".to_string(),
                decimals: 6,
                price_usd: Some(Decimal::from_f64_retain(1.00).unwrap()),
            },
            reserve_a: 15_000_000_000,  // 15,000 ORCA
            reserve_b: 48_000_000_000,  // 48,000 USDC
            fee_percent: Decimal::from_f64_retain(0.003).unwrap(),
            liquidity_usd: Decimal::from_f64_retain(96_000.0).unwrap(),
            last_updated: chrono::Utc::now(),
        },
    ]
}

/// Generate mock Raydium pools with realistic data
pub fn generate_mock_raydium_pools() -> Vec<Pool> {
    info!("Generating mock Raydium pools for testing");
    
    vec![
        // SOL/USDC pool (slightly different reserves for arbitrage opportunities)
        Pool {
            address: Pubkey::from_str("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2").unwrap(),
            dex: "raydium".to_string(),
            token_a: TokenInfo {
                mint: Pubkey::from_str(CommonTokens::SOL).unwrap(),
                symbol: "SOL".to_string(),
                decimals: 9,
                price_usd: Some(Decimal::from_f64_retain(95.75).unwrap()), // Slightly higher price
            },
            token_b: TokenInfo {
                mint: Pubkey::from_str(CommonTokens::USDC).unwrap(),
                symbol: "USDC".to_string(),
                decimals: 6,
                price_usd: Some(Decimal::from_f64_retain(1.00).unwrap()),
            },
            reserve_a: 200_000_000_000, // 200 SOL
            reserve_b: 19_000_000_000,  // 19,000 USDC (creates price difference)
            fee_percent: Decimal::from_f64_retain(0.0025).unwrap(), // 0.25%
            liquidity_usd: Decimal::from_f64_retain(38_000.0).unwrap(),
            last_updated: chrono::Utc::now(),
        },
        // RAY/USDC pool
        Pool {
            address: Pubkey::from_str("6UmmUiYoBjSrhakAobJw8BvkmJtDVxaeBtbt7rxWo1mg").unwrap(),
            dex: "raydium".to_string(),
            token_a: TokenInfo {
                mint: Pubkey::from_str(CommonTokens::RAY).unwrap(),
                symbol: "RAY".to_string(),
                decimals: 6,
                price_usd: Some(Decimal::from_f64_retain(0.87).unwrap()), // Slightly higher
            },
            token_b: TokenInfo {
                mint: Pubkey::from_str(CommonTokens::USDC).unwrap(),
                symbol: "USDC".to_string(),
                decimals: 6,
                price_usd: Some(Decimal::from_f64_retain(1.00).unwrap()),
            },
            reserve_a: 75_000_000_000,  // 75,000 RAY
            reserve_b: 64_000_000_000,  // 64,000 USDC (creates arbitrage opportunity)
            fee_percent: Decimal::from_f64_retain(0.0025).unwrap(),
            liquidity_usd: Decimal::from_f64_retain(128_000.0).unwrap(),
            last_updated: chrono::Utc::now(),
        },
        // USDT/USDC pool
        Pool {
            address: Pubkey::from_str("7XawhbbxtsRcQA8KTkHT9f9nc6d69UwqCDh6U5EEbEmX").unwrap(),
            dex: "raydium".to_string(),
            token_a: TokenInfo {
                mint: Pubkey::from_str(CommonTokens::USDT).unwrap(),
                symbol: "USDT".to_string(),
                decimals: 6,
                price_usd: Some(Decimal::from_f64_retain(1.001).unwrap()),
            },
            token_b: TokenInfo {
                mint: Pubkey::from_str(CommonTokens::USDC).unwrap(),
                symbol: "USDC".to_string(),
                decimals: 6,
                price_usd: Some(Decimal::from_f64_retain(0.999).unwrap()),
            },
            reserve_a: 500_000_000_000, // 500,000 USDT
            reserve_b: 498_000_000_000, // 498,000 USDC (small arbitrage opportunity)
            fee_percent: Decimal::from_f64_retain(0.0025).unwrap(),
            liquidity_usd: Decimal::from_f64_retain(998_000.0).unwrap(),
            last_updated: chrono::Utc::now(),
        },
    ]
}

/// Generate mock Phoenix markets with realistic data
pub fn generate_mock_phoenix_pools() -> Vec<Pool> {
    info!("Generating mock Phoenix markets for testing");
    
    vec![
        // SOL/USDC market
        Pool {
            address: Pubkey::from_str("4DoNfFBfF7UokCC2FQzriy7yHK6DY6NVdYpuekQ5pRgg").unwrap(),
            dex: "phoenix".to_string(),
            token_a: TokenInfo {
                mint: Pubkey::from_str(CommonTokens::SOL).unwrap(),
                symbol: "SOL".to_string(),
                decimals: 9,
                price_usd: Some(Decimal::from_f64_retain(95.25).unwrap()), // Lower price for arbitrage
            },
            token_b: TokenInfo {
                mint: Pubkey::from_str(CommonTokens::USDC).unwrap(),
                symbol: "USDC".to_string(),
                decimals: 6,
                price_usd: Some(Decimal::from_f64_retain(1.00).unwrap()),
            },
            reserve_a: 80_000_000_000,  // 80 SOL in orderbook
            reserve_b: 7_500_000_000,   // 7,500 USDC in orderbook
            fee_percent: Decimal::from_f64_retain(0.0001).unwrap(), // 0.01% (lower fees)
            liquidity_usd: Decimal::from_f64_retain(15_000.0).unwrap(),
            last_updated: chrono::Utc::now(),
        },
        // BONK/SOL market
        Pool {
            address: Pubkey::from_str("8BnEgHoWFysVcuFFX7QztDmzuH8r5ZFvyP3sYwn1XTh6").unwrap(),
            dex: "phoenix".to_string(),
            token_a: TokenInfo {
                mint: Pubkey::from_str(CommonTokens::BONK).unwrap(),
                symbol: "BONK".to_string(),
                decimals: 5,
                price_usd: Some(Decimal::from_f64_retain(0.000015).unwrap()),
            },
            token_b: TokenInfo {
                mint: Pubkey::from_str(CommonTokens::SOL).unwrap(),
                symbol: "SOL".to_string(),
                decimals: 9,
                price_usd: Some(Decimal::from_f64_retain(95.50).unwrap()),
            },
            reserve_a: 1_000_000_000_000_000, // 1B BONK
            reserve_b: 150_000_000_000,       // 150 SOL
            fee_percent: Decimal::from_f64_retain(0.0001).unwrap(),
            liquidity_usd: Decimal::from_f64_retain(30_000.0).unwrap(),
            last_updated: chrono::Utc::now(),
        },
    ]
}

/// Get all mock pools from all DEXs
pub fn get_all_mock_pools() -> Vec<Pool> {
    let mut all_pools = Vec::new();
    all_pools.extend(generate_mock_orca_pools());
    all_pools.extend(generate_mock_raydium_pools());
    all_pools.extend(generate_mock_phoenix_pools());
    all_pools
}

/// Check if mock data should be used based on environment variable
pub fn should_use_mock_data() -> bool {
    std::env::var("USE_MOCK_DATA")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase() == "true"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_data_generation() {
        let orca_pools = generate_mock_orca_pools();
        assert!(!orca_pools.is_empty());
        assert_eq!(orca_pools[0].dex, "orca");

        let raydium_pools = generate_mock_raydium_pools();
        assert!(!raydium_pools.is_empty());
        assert_eq!(raydium_pools[0].dex, "raydium");

        let phoenix_pools = generate_mock_phoenix_pools();
        assert!(!phoenix_pools.is_empty());
        assert_eq!(phoenix_pools[0].dex, "phoenix");
    }

    #[test]
    fn test_arbitrage_opportunities() {
        let all_pools = get_all_mock_pools();
        
        // Find SOL/USDC pools across different DEXs
        let sol_usdc_pools: Vec<_> = all_pools.iter()
            .filter(|pool| {
                (pool.token_a.symbol == "SOL" && pool.token_b.symbol == "USDC") ||
                (pool.token_a.symbol == "USDC" && pool.token_b.symbol == "SOL")
            })
            .collect();
        
        assert!(sol_usdc_pools.len() >= 2, "Should have SOL/USDC pools on multiple DEXs for arbitrage");
    }
}