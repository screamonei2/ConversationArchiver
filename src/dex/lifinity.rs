use crate::models::{Pool, TokenInfo};
use crate::dex::DexClient;
use crate::console::ConsoleManager;

use crate::utils::rpc::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
// use std::collections::HashMap; // Unused
// use serde::{Deserialize, Serialize}; // Unused
// use tracing::{info, error, warn}; // Unused
use chrono;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;

pub const LIFINITY_PROGRAM_ID: &str = "EewxydAPCCVuNEyrVN68PuSYdQ7wKn27V9Gjeoi8dy3S";

// Lifinity pool discriminator


#[derive(Debug)]
pub struct LifinityPool {
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub token_a_vault: Pubkey,
    pub token_b_vault: Pubkey,
    pub pool_mint: Pubkey,
    pub oracle_a: Pubkey,
    pub oracle_b: Pubkey,
    pub fee_rate: u64,
    pub oracle_priority: u8,
    pub rebalance_threshold: u64,
    pub last_rebalance_time: i64,
    pub concentrated_liquidity_params: ConcentratedLiquidityParams,
}

#[derive(Debug)]
pub struct ConcentratedLiquidityParams {
    pub lower_tick: i32,
    pub upper_tick: i32,
    pub liquidity: u128,
    pub fee_growth_inside_a: u128,
    pub fee_growth_inside_b: u128,
}

pub struct LifinityDex {
    pub client: Arc<RpcClient>,
    pub program_id: Pubkey,
    pub console_manager: Option<Arc<ConsoleManager>>,
}

impl LifinityDex {
    pub fn new(rpc_client: Arc<crate::utils::rpc::RpcClient>, console_manager: Arc<ConsoleManager>) -> Result<Self, anyhow::Error> {
        let program_id = Pubkey::from_str(LIFINITY_PROGRAM_ID)?;
        
        Ok(Self {
            client: rpc_client,
            program_id,
            console_manager: Some(console_manager),
        })
    }

    pub async fn fetch_pools(&self) -> Result<Vec<Pool>, anyhow::Error> {
        let accounts = self.client.get_program_accounts(&self.program_id).await?;
        let mut pools = Vec::new();
        
        for (pubkey, account) in accounts {
            if account.data.len() >= 8 && self.is_lifinity_pool_account(&account.data) {
                if let Ok(pool_data) = self.parse_lifinity_pool_data(&account.data) {
                    // Get vault balances
                    let reserve_a = self.get_token_account_balance(&pool_data.token_a_vault).await.unwrap_or(0.0);
                    let reserve_b = self.get_token_account_balance(&pool_data.token_b_vault).await.unwrap_or(0.0);
                    
                    // Get oracle prices for better pricing
                    let _oracle_price_a = self.get_oracle_price(&pool_data.oracle_a).await.unwrap_or(1.0);
                    let _oracle_price_b = self.get_oracle_price(&pool_data.oracle_b).await.unwrap_or(1.0);
                    
                    let fee_rate = pool_data.fee_rate as f64 / 10000.0;
                    
                    let pool = Pool {
                        address: pubkey,
                        dex: "Lifinity".to_string(),
                        token_a: TokenInfo {
                            mint: pool_data.token_a_mint,
                            symbol: self.get_token_symbol(&pool_data.token_a_mint),
                            decimals: 6,
                            price_usd: None,
                        },
                        token_b: TokenInfo {
                            mint: pool_data.token_b_mint,
                            symbol: self.get_token_symbol(&pool_data.token_b_mint),
                            decimals: 6,
                            price_usd: None,
                        },
                        reserve_a: reserve_a as u64,
                        reserve_b: reserve_b as u64,
                        fee_percent: Decimal::from_f64(fee_rate).unwrap_or_default(),
                        liquidity_usd: Decimal::from((reserve_a + reserve_b) as u64),
                        last_updated: chrono::Utc::now(),
                    };
                    
                    pools.push(pool);
                }
            }
        }
        
        Ok(pools)
    }

    fn is_lifinity_pool_account(&self, data: &[u8]) -> bool {
        if data.len() < 8 {
            return false;
        }
        
        // Check for reasonable pool account size
        data.len() >= 400 && data.len() <= 800
    }

    fn parse_lifinity_pool_data(&self, data: &[u8]) -> Result<LifinityPool, anyhow::Error> {
        if data.len() < 400 {
            return Err(anyhow::anyhow!("Invalid Lifinity pool data size"));
        }
        
        // Parse Lifinity pool structure
        let token_a_mint = Pubkey::try_from(&data[8..40])?;
        let token_b_mint = Pubkey::try_from(&data[40..72])?;
        let token_a_vault = Pubkey::try_from(&data[72..104])?;
        let token_b_vault = Pubkey::try_from(&data[104..136])?;
        let pool_mint = Pubkey::try_from(&data[136..168])?;
        let oracle_a = Pubkey::try_from(&data[168..200])?;
        let oracle_b = Pubkey::try_from(&data[200..232])?;
        
        let fee_rate = u64::from_le_bytes([
            data[232], data[233], data[234], data[235],
            data[236], data[237], data[238], data[239],
        ]);
        
        let oracle_priority = data[240];
        
        let rebalance_threshold = u64::from_le_bytes([
            data[241], data[242], data[243], data[244],
            data[245], data[246], data[247], data[248],
        ]);
        
        let last_rebalance_time = i64::from_le_bytes([
            data[249], data[250], data[251], data[252],
            data[253], data[254], data[255], data[256],
        ]);
        
        // Parse concentrated liquidity parameters
        let lower_tick = i32::from_le_bytes([data[257], data[258], data[259], data[260]]);
        let upper_tick = i32::from_le_bytes([data[261], data[262], data[263], data[264]]);
        
        let liquidity = u128::from_le_bytes([
            data[265], data[266], data[267], data[268],
            data[269], data[270], data[271], data[272],
            data[273], data[274], data[275], data[276],
            data[277], data[278], data[279], data[280],
        ]);
        
        let fee_growth_inside_a = u128::from_le_bytes([
            data[281], data[282], data[283], data[284],
            data[285], data[286], data[287], data[288],
            data[289], data[290], data[291], data[292],
            data[293], data[294], data[295], data[296],
        ]);
        
        let fee_growth_inside_b = u128::from_le_bytes([
            data[297], data[298], data[299], data[300],
            data[301], data[302], data[303], data[304],
            data[305], data[306], data[307], data[308],
            data[309], data[310], data[311], data[312],
        ]);
        
        let concentrated_liquidity_params = ConcentratedLiquidityParams {
            lower_tick,
            upper_tick,
            liquidity,
            fee_growth_inside_a,
            fee_growth_inside_b,
        };
        
        Ok(LifinityPool {
            token_a_mint,
            token_b_mint,
            token_a_vault,
            token_b_vault,
            pool_mint,
            oracle_a,
            oracle_b,
            fee_rate,
            oracle_priority,
            rebalance_threshold,
            last_rebalance_time,
            concentrated_liquidity_params,
        })
    }

    async fn get_oracle_price(&self, oracle_pubkey: &Pubkey) -> Result<f64, anyhow::Error> {
        // Simplified oracle price fetching
        // In practice, this would parse Pyth, Switchboard, or other oracle data
        match self.client.try_get_account(oracle_pubkey).await {
            Ok(Some(account)) => {
                if account.data.len() >= 8 {
                    // Mock oracle price parsing
                    let price_bytes = &account.data[8..16];
                    let price = f64::from_le_bytes([
                        price_bytes[0], price_bytes[1], price_bytes[2], price_bytes[3],
                        price_bytes[4], price_bytes[5], price_bytes[6], price_bytes[7],
                    ]);
                    Ok(price.abs()) // Ensure positive price
                } else {
                    Ok(1.0) // Default price
                }
            }
            Ok(None) | Err(_) => Ok(1.0), // Default price if oracle not accessible or account not found
        }
    }

    async fn get_token_account_balance(&self, vault_pubkey: &Pubkey) -> Result<f64, anyhow::Error> {
        match self.client.try_get_token_account_balance(vault_pubkey).await {
            Ok(Some(balance)) => {
                let amount = balance as f64 / 1e6; // Convert from raw amount to UI amount
                Ok(amount)
            }
            Ok(None) => Ok(0.0), // Account not found or invalid
            Err(_) => Ok(0.0), // Other errors
        }
    }

    fn get_token_symbol(&self, mint: &Pubkey) -> String {
        match mint.to_string().as_str() {
            "So11111111111111111111111111111111111111112" => "SOL".to_string(),
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" => "USDC".to_string(),
            "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB" => "USDT".to_string(),
            _ => "UNKNOWN".to_string(),
        }
    }

    // Lifinity-specific proactive market making calculation
    pub fn calculate_proactive_price(
        &self,
        oracle_price: f64,
        current_price: f64,
        _time_since_last_rebalance: i64,
        rebalance_threshold: f64,
    ) -> f64 {
        // Lifinity's proactive market making adjusts prices based on oracle data
        let price_deviation = (current_price - oracle_price) / oracle_price;
        
        if price_deviation.abs() > rebalance_threshold {
            // Move price towards oracle price
            let adjustment_factor = 0.1; // 10% adjustment per rebalance
            current_price + (oracle_price - current_price) * adjustment_factor
        } else {
            current_price
        }
    }

    pub async fn is_healthy(&self) -> bool {
        self.client.get_latest_blockhash().await.is_ok()
    }
}

#[async_trait]
impl DexClient for LifinityDex {
    async fn fetch_pools(&self) -> Result<Vec<Pool>> {
        self.fetch_pools().await
    }

    async fn get_pool_by_tokens(&self, token_a: &str, token_b: &str) -> Result<Option<Pool>> {
        let pools = self.fetch_pools().await?;
        
        for pool in pools {
            if (pool.token_a.mint.to_string() == token_a && pool.token_b.mint.to_string() == token_b) ||
               (pool.token_a.mint.to_string() == token_b && pool.token_b.mint.to_string() == token_a) {
                return Ok(Some(pool));
            }
        }
        
        Ok(None)
    }

    async fn update_pool_reserves(&self, pool: &mut Pool) -> Result<()> {
        // Fetch updated pool data
        if let Ok(Some(account)) = self.client.try_get_account(&pool.address).await {
            if let Ok(pool_data) = self.parse_lifinity_pool_data(&account.data) {
                let reserve_a = self.get_token_account_balance(&pool_data.token_a_vault).await.unwrap_or(0.0);
                let reserve_b = self.get_token_account_balance(&pool_data.token_b_vault).await.unwrap_or(0.0);
                
                pool.reserve_a = reserve_a as u64;
                pool.reserve_b = reserve_b as u64;
                pool.last_updated = chrono::Utc::now();
            }
        }
        
        Ok(())
    }

    fn get_dex_name(&self) -> &'static str {
        "Lifinity"
    }

    fn set_console_manager(&mut self, console: Arc<ConsoleManager>) {
        self.console_manager = Some(console);
    }
}