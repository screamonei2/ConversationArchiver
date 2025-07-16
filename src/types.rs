use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;

pub type TokenMint = String;
pub type DexName = String;

#[derive(Debug, Clone, PartialEq)]
pub enum ArbitrageType {
    Direct,      // A -> B -> A
    Triangular,  // A -> B -> C -> A
    CrossDex,    // A -> B (DEX1), B -> A (DEX2)
}

#[derive(Debug, Clone)]
pub enum TradeDirection {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
pub struct Price {
    pub token_mint: TokenMint,
    pub price_usd: f64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub dex: DexName,
    pub liquidity_usd: f64,
}

#[derive(Debug, Clone)]
pub struct TokenBalance {
    pub mint: TokenMint,
    pub amount: u64,
    pub decimals: u8,
}

#[derive(Debug, Clone)]
pub struct WalletInfo {
    pub address: Pubkey,
    pub balances: HashMap<TokenMint, TokenBalance>,
    pub sol_balance: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResult {
    pub signature: String,
    pub success: bool,
    pub error: Option<String>,
    pub compute_units_consumed: Option<u64>,
    pub fee_lamports: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct MarketData {
    pub prices: HashMap<TokenMint, Vec<Price>>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}
