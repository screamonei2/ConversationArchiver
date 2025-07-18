use crate::models::{Pool, TokenInfo};
use crate::dex::DexClient;
use crate::console::ConsoleManager;
use anyhow::Result;
use async_trait::async_trait;
use crate::utils::rpc::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use serde_json::Value;
use reqwest;
use std::sync::Arc;
// use std::collections::HashMap; // Unused
// use serde::{Deserialize, Serialize}; // Unused
// use tracing::{info, error, warn}; // Unused
use chrono;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;

pub const PUMPFUN_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
pub const PUMPFUN_API_BASE: &str = "https://frontend-api.pump.fun";

// Pump.fun bonding curve discriminator
const PUMPFUN_CURVE_DISCRIMINATOR: [u8; 8] = [67, 117, 114, 118, 101, 0, 0, 0]; // "Curve\0\0\0"

#[derive(Debug)]
pub struct PumpFunCurve {
    pub mint: Pubkey,
    pub bonding_curve: Pubkey,
    pub associated_bonding_curve: Pubkey,
    pub creator: Pubkey,
    pub virtual_token_reserves: u64,
    pub virtual_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub real_sol_reserves: u64,
    pub token_total_supply: u64,
    pub complete: bool,
}

pub struct PumpFunDex {
    pub client: Arc<RpcClient>,
    pub program_id: Pubkey,
    pub console_manager: Option<Arc<ConsoleManager>>,
}

impl PumpFunDex {
    pub fn new(rpc_client: Arc<crate::utils::rpc::RpcClient>, console_manager: Arc<ConsoleManager>) -> Result<Self> {
        let program_id = Pubkey::from_str(PUMPFUN_PROGRAM_ID)?;
        
        Ok(Self {
            client: rpc_client,
            program_id,
            console_manager: Some(console_manager),
        })
    }

    pub async fn fetch_pools(&self) -> Result<Vec<Pool>> {
        let mut pools = Vec::new();
        
        // Fetch from API first for active tokens
        let api_pools = self.fetch_pools_from_api().await?;
        pools.extend(api_pools);
        
        // Also fetch from blockchain for additional discovery
        let blockchain_pools = self.fetch_pools_from_blockchain().await?;
        pools.extend(blockchain_pools);
        
        Ok(pools)
    }

    async fn fetch_pools_from_api(&self) -> Result<Vec<Pool>> {
        let url = format!("{}/coins", PUMPFUN_API_BASE);
        
        match reqwest::get(&url).await {
            Ok(response) => {
                if let Ok(coins) = response.json::<Value>().await {
                    let mut pools = Vec::new();
                    
                    if let Some(coins_array) = coins.as_array() {
                        for coin in coins_array.iter().take(20) { // Limit to top 20
                            if let Some(pool) = self.api_coin_to_pool(coin) {
                                pools.push(pool);
                            }
                        }
                    }
                    
                    Ok(pools)
                } else {
                    Ok(Vec::new())
                }
            }
            Err(_) => {
                // Fallback to blockchain data if API fails
                Ok(Vec::new())
            }
        }
    }

    async fn fetch_pools_from_blockchain(&self) -> Result<Vec<Pool>> {
        let accounts = self.client.get_program_accounts(&self.program_id).await?;
        let mut pools = Vec::new();
        
        for (pubkey, account) in accounts {
            if account.data.len() >= 8 && self.is_pumpfun_curve_account(&account.data) {
                if let Ok(curve_data) = self.parse_pumpfun_curve_data(&account.data) {
                    // Only include active (incomplete) curves
                    if !curve_data.complete && curve_data.real_sol_reserves > 0 {
                        let pool = self.curve_to_pool(&pubkey, &curve_data)?;
                        pools.push(pool);
                        
                        if pools.len() >= 10 { // Limit blockchain discovery
                            break;
                        }
                    }
                }
            }
        }
        
        Ok(pools)
    }

    fn api_coin_to_pool(&self, coin: &Value) -> Option<Pool> {
        let mint = coin["mint"].as_str()?;
        let _name = coin["name"].as_str().unwrap_or("Unknown");
        let symbol = coin["symbol"].as_str().unwrap_or("UNKNOWN");
        let market_cap = coin["market_cap"].as_f64().unwrap_or(0.0);
        
        // Calculate virtual reserves based on market cap
        let virtual_sol_reserves = market_cap / 50.0; // Rough estimate
        let virtual_token_reserves = 1000000000.0; // 1B tokens typical
        
        let token_info = TokenInfo {
            mint: Pubkey::from_str(mint).unwrap_or_else(|_| Pubkey::new_unique()),
            symbol: symbol.to_string(),
            decimals: 6,
            price_usd: None,
        };
        
        let sol_info = TokenInfo {
            mint: Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
            symbol: "SOL".to_string(),
            decimals: 9,
            price_usd: None,
        };
        
        Some(Pool {
             address: Pubkey::from_str(mint).unwrap_or_else(|_| Pubkey::new_unique()),
            dex: "Pump.fun".to_string(),
            token_a: token_info,
            token_b: sol_info,
            reserve_a: virtual_token_reserves as u64,
            reserve_b: virtual_sol_reserves as u64,
            fee_percent: Decimal::from_f64(0.01).unwrap(), // 1% fee typical for pump.fun
            liquidity_usd: Decimal::from(market_cap as u64),
            last_updated: chrono::Utc::now(),
        })
    }

    fn curve_to_pool(&self, curve_pubkey: &Pubkey, curve_data: &PumpFunCurve) -> Result<Pool> {
        let token_info = TokenInfo {
            mint: curve_data.mint,
            symbol: "MEME".to_string(),
            decimals: 6,
            price_usd: None,
        };
        
        let sol_info = TokenInfo {
            mint: Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
            symbol: "SOL".to_string(),
            decimals: 9,
            price_usd: None,
        };
        
        let _token_reserves = curve_data.virtual_token_reserves as f64 / 1e6;
        let _sol_reserves = curve_data.virtual_sol_reserves as f64 / 1e9;
        
        Ok(Pool {
             address: *curve_pubkey,
            dex: "Pump.fun".to_string(),
            token_a: token_info,
            token_b: sol_info,
            reserve_a: curve_data.virtual_token_reserves,
            reserve_b: curve_data.virtual_sol_reserves,
            fee_percent: Decimal::from_f64(0.01).unwrap(), // 1% fee
            liquidity_usd: Decimal::from((curve_data.virtual_token_reserves + curve_data.virtual_sol_reserves) as u64),
            last_updated: chrono::Utc::now(),
        })
    }

    fn is_pumpfun_curve_account(&self, data: &[u8]) -> bool {
        if data.len() < 8 {
            return false;
        }
        
        // Check for reasonable curve account size
        data.len() >= 200 && data.len() <= 400
    }

    fn parse_pumpfun_curve_data(&self, data: &[u8]) -> Result<PumpFunCurve> {
        if data.len() < 200 {
            return Err(anyhow::anyhow!("Invalid Pump.fun curve data size"));
        }
        
        // Parse Pump.fun bonding curve structure
        let mint = Pubkey::try_from(&data[8..40])?;
        let bonding_curve = Pubkey::try_from(&data[40..72])?;
        let associated_bonding_curve = Pubkey::try_from(&data[72..104])?;
        let creator = Pubkey::try_from(&data[104..136])?;
        
        let virtual_token_reserves = u64::from_le_bytes([
            data[136], data[137], data[138], data[139],
            data[140], data[141], data[142], data[143],
        ]);
        
        let virtual_sol_reserves = u64::from_le_bytes([
            data[144], data[145], data[146], data[147],
            data[148], data[149], data[150], data[151],
        ]);
        
        let real_token_reserves = u64::from_le_bytes([
            data[152], data[153], data[154], data[155],
            data[156], data[157], data[158], data[159],
        ]);
        
        let real_sol_reserves = u64::from_le_bytes([
            data[160], data[161], data[162], data[163],
            data[164], data[165], data[166], data[167],
        ]);
        
        let token_total_supply = u64::from_le_bytes([
            data[168], data[169], data[170], data[171],
            data[172], data[173], data[174], data[175],
        ]);
        
        let complete = data[176] != 0;
        
        Ok(PumpFunCurve {
            mint,
            bonding_curve,
            associated_bonding_curve,
            creator,
            virtual_token_reserves,
            virtual_sol_reserves,
            real_token_reserves,
            real_sol_reserves,
            token_total_supply,
            complete,
        })
    }

    // Pump.fun bonding curve price calculation
    pub fn calculate_bonding_curve_price(
        &self,
        virtual_sol_reserves: u64,
        virtual_token_reserves: u64,
        token_amount: u64,
    ) -> f64 {
        // Bonding curve formula: price = sol_reserves / token_reserves
        let current_price = virtual_sol_reserves as f64 / virtual_token_reserves as f64;
        
        // Calculate price impact for the trade
        let new_token_reserves = virtual_token_reserves - token_amount;
        let new_price = virtual_sol_reserves as f64 / new_token_reserves as f64;
        
        (current_price + new_price) / 2.0 // Average price
    }

    pub async fn get_token_info(&self, mint: &str) -> Result<Option<Value>> {
        let url = format!("{}/coins/{}", PUMPFUN_API_BASE, mint);
        
        match reqwest::get(&url).await {
            Ok(response) => {
                if response.status().is_success() {
                    let token_info: Value = response.json().await?;
                    Ok(Some(token_info))
                } else {
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
        }
    }

    pub async fn is_healthy(&self) -> bool {
        self.client.get_health().await.is_ok()
    }
}

#[async_trait]
impl DexClient for PumpFunDex {
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
        // For Pump.fun, try to get updated data from API first
        if let Ok(Some(token_info)) = self.get_token_info(&pool.token_a.mint.to_string()).await {
            if let Some(market_cap) = token_info["market_cap"].as_f64() {
                let virtual_sol_reserves = market_cap / 50.0;
                let virtual_token_reserves = 1000000000.0;
                
                pool.reserve_a = virtual_token_reserves as u64;
                pool.reserve_b = virtual_sol_reserves as u64;
                pool.last_updated = chrono::Utc::now();
            }
        } else {
            // Fallback to blockchain data
            if let Ok(Some(account)) = self.client.try_get_account(&pool.address).await {
                if let Ok(curve_data) = self.parse_pumpfun_curve_data(&account.data) {
                    pool.reserve_a = curve_data.virtual_token_reserves;
                    pool.reserve_b = curve_data.virtual_sol_reserves;
                    pool.last_updated = chrono::Utc::now();
                }
            }
        }
        
        Ok(())
    }

    fn get_dex_name(&self) -> &'static str {
        "PumpFun"
    }

    fn set_console_manager(&mut self, console: Arc<ConsoleManager>) {
        self.console_manager = Some(console);
    }
}