use crate::types::{ArbitrageType, DexName, TokenMint, TradeDirection};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

#[derive(Debug, Clone)]
pub struct Pool {
    pub address: Pubkey,
    pub dex: DexName,
    pub token_a: TokenInfo,
    pub token_b: TokenInfo,
    pub reserve_a: u64,
    pub reserve_b: u64,
    pub fee_percent: Decimal,
    pub liquidity_usd: Decimal,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct TokenInfo {
    pub mint: Pubkey,
    pub symbol: String,
    pub decimals: u8,
    pub price_usd: Option<Decimal>,
}

#[derive(Debug, Clone)]
pub struct ArbitrageRoute {
    pub route_type: ArbitrageType,
    pub from_token: TokenMint,
    pub to_token: TokenMint,
    pub intermediate_token: Option<TokenMint>,
    pub steps: Vec<TradeStep>,
    pub total_fee_percent: Decimal,
}

#[derive(Debug, Clone)]
pub struct TradeStep {
    pub pool: Pool,
    pub direction: TradeDirection,
    pub input_amount: u64,
    pub expected_output: u64,
    pub price_impact: Decimal,
    pub slippage: Decimal,
}

#[derive(Debug, Clone)]
pub struct ArbitrageOpportunity {
    pub id: String,
    pub route: ArbitrageRoute,
    pub input_amount: u64,
    pub expected_output: u64,
    pub expected_profit: u64,
    pub expected_profit_percent: f64,
    pub confidence_score: f64,
    pub risk_score: f64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub expiry: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct ProfitabilityAnalysis {
    pub gross_profit: Decimal,
    pub net_profit: Decimal,
    pub profit_percentage: f64,
    pub total_fees: Decimal,
    pub gas_estimate: u64,
    pub break_even_amount: Decimal,
    pub risk_adjusted_return: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolTransaction {
    pub signature: String,
    pub from_address: String,
    pub to_address: Option<String>,
    pub amount_sol: f64,
    pub token_mint: Option<String>,
    pub program_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct WhaleActivity {
    pub wallet_address: Pubkey,
    pub transaction_signature: String,
    pub token_mint: TokenMint,
    pub amount: u64,
    pub direction: TradeDirection,
    pub dex: DexName,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub price_impact: Option<Decimal>,
}

#[derive(Debug, Clone)]
pub struct TradeExecution {
    pub opportunity_id: String,
    pub signature: String,
    pub executed_at: chrono::DateTime<chrono::Utc>,
    pub input_amount: u64,
    pub actual_output: u64,
    pub actual_profit: i64,
    pub gas_fee: u64,
    pub slippage: Decimal,
    pub success: bool,
    pub error_message: Option<String>,
}


