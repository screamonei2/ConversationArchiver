use crate::{
    dex::DexClient,
    models::{Pool, TokenInfo},
    utils::rpc::RpcClient,
    // config::Config, // Unused
    console::ConsoleManager,
};
use anyhow::Result;
use async_trait::async_trait;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use std::collections::HashMap;
// use serde::{Deserialize, Serialize}; // Unused
// use tracing::{info, error, warn}; // Unused
use chrono;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use std::str::FromStr;

pub const SERUM_PROGRAM_ID: &str = "9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin";

// Serum market discriminator


#[derive(Debug)]
pub struct SerumMarket {
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub base_vault: Pubkey,
    pub quote_vault: Pubkey,
    pub bids: Pubkey,
    pub asks: Pubkey,
    pub event_queue: Pubkey,
    pub base_lot_size: u64,
    pub quote_lot_size: u64,
    pub fee_rate_bps: u64,
}

#[derive(Debug)]
pub struct OrderBookLevel {
    pub price: f64,
    pub size: f64,
}

pub struct SerumDex {
    pub client: RpcClient,
    pub program_id: Pubkey,
    pub known_markets: HashMap<String, Pubkey>,
    console_manager: Option<Arc<ConsoleManager>>,
}

impl SerumDex {
    pub fn new(rpc_client: Arc<crate::utils::rpc::RpcClient>, console_manager: Arc<ConsoleManager>) -> Result<Self> {
        let program_id = Pubkey::from_str(SERUM_PROGRAM_ID)?;
        
        // Initialize with some well-known Serum markets
        let mut known_markets = HashMap::new();
        known_markets.insert(
            "SOL/USDC".to_string(),
            Pubkey::from_str("9wFFyRfZBsuAha4YcuxcXLKwMxJR43S7fPfQLusDBzvT").unwrap(),
        );
        known_markets.insert(
            "SOL/USDT".to_string(),
            Pubkey::from_str("HWHvQhFmJB3NUcu1aihKmrKegfVxBEHzwVX6yZCKEsi1").unwrap(),
        );
        
        Ok(Self {
            client: (*rpc_client).clone(),
            program_id,
            known_markets,
            console_manager: Some(console_manager),
        })
    }

    pub async fn fetch_pools(&self) -> Result<Vec<Pool>> {
        let mut pools = Vec::new();
        
        // Fetch from known markets first
        for (market_name, market_pubkey) in &self.known_markets {
            if let Ok(market_data) = self.fetch_market_data(market_pubkey).await {
                let pool = self.market_to_pool(market_name, market_pubkey, &market_data).await?;
                pools.push(pool);
            }
        }
        
        // Also try to discover markets from program accounts
        let discovered_pools = self.discover_markets().await?;
        pools.extend(discovered_pools);
        
        Ok(pools)
    }

    async fn fetch_market_data(&self, market_pubkey: &Pubkey) -> Result<SerumMarket> {
        let account = match self.client.try_get_account(market_pubkey).await? {
            Some(account) => account,
            None => return Err(anyhow::anyhow!("Market account not found")),
        };
        self.parse_serum_market_data(&account.data)
    }

    async fn discover_markets(&self) -> Result<Vec<Pool>> {
        let accounts = self.client.get_program_accounts(&self.program_id).await?;
        let mut pools = Vec::new();
        
        for (pubkey, account) in accounts {
            if account.data.len() >= 8 && self.is_serum_market_account(&account.data) {
                if let Ok(market_data) = self.parse_serum_market_data(&account.data) {
                    let market_name = format!(
                        "{}/{}",
                        self.get_token_symbol(&market_data.base_mint),
                        self.get_token_symbol(&market_data.quote_mint)
                    );
                    
                    let pool = self.market_to_pool(&market_name, &pubkey, &market_data).await?;
                    pools.push(pool);
                    
                    if pools.len() >= 10 { // Limit discovery
                        break;
                    }
                }
            }
        }
        
        Ok(pools)
    }

    async fn market_to_pool(
        &self,
        _market_name: &str,
        market_pubkey: &Pubkey,
        market_data: &SerumMarket,
    ) -> Result<Pool> {
        let base_balance = self.get_token_account_balance(&market_data.base_vault).await.unwrap_or(0.0);
        let quote_balance = self.get_token_account_balance(&market_data.quote_vault).await.unwrap_or(0.0);
        
        let token_a_info = TokenInfo {
            mint: market_data.base_mint,
            symbol: self.get_token_symbol(&market_data.base_mint),
            decimals: 6, // Default, should be fetched from mint
            price_usd: None,
        };
        
        let token_b_info = TokenInfo {
            mint: market_data.quote_mint,
            symbol: self.get_token_symbol(&market_data.quote_mint),
            decimals: 6, // Default, should be fetched from mint
            price_usd: None,
        };
        
        let fee_rate = market_data.fee_rate_bps as f64 / 10000.0;
        
        Ok(Pool {
            address: *market_pubkey,
            dex: "Serum".to_string(),
            token_a: token_a_info,
            token_b: token_b_info,
            reserve_a: base_balance as u64,
            reserve_b: quote_balance as u64,
            fee_percent: Decimal::from_f64(fee_rate).unwrap_or_default(),
            liquidity_usd: Decimal::from((base_balance + quote_balance) as u64),
            last_updated: chrono::Utc::now(),
        })
    }

    fn is_serum_market_account(&self, data: &[u8]) -> bool {
        if data.len() < 8 {
            return false;
        }
        
        // Check for market account size (Serum markets are typically around 388 bytes)
        data.len() >= 300 && data.len() <= 500
    }

    fn parse_serum_market_data(&self, data: &[u8]) -> Result<SerumMarket> {
        if data.len() < 300 {
            return Err(anyhow::anyhow!("Invalid Serum market data size"));
        }
        
        // Parse Serum market structure
        // Note: This is a simplified parsing - actual Serum markets have a more complex structure
        let base_mint = Pubkey::try_from(&data[53..85])?;
        let quote_mint = Pubkey::try_from(&data[85..117])?;
        let base_vault = Pubkey::try_from(&data[117..149])?;
        let quote_vault = Pubkey::try_from(&data[149..181])?;
        let bids = Pubkey::try_from(&data[181..213])?;
        let asks = Pubkey::try_from(&data[213..245])?;
        let event_queue = Pubkey::try_from(&data[245..277])?;
        
        let base_lot_size = u64::from_le_bytes([
            data[277], data[278], data[279], data[280],
            data[281], data[282], data[283], data[284],
        ]);
        
        let quote_lot_size = u64::from_le_bytes([
            data[285], data[286], data[287], data[288],
            data[289], data[290], data[291], data[292],
        ]);
        
        let fee_rate_bps = u64::from_le_bytes([
            data[293], data[294], data[295], data[296],
            data[297], data[298], data[299], data[300],
        ]);
        
        Ok(SerumMarket {
            base_mint,
            quote_mint,
            base_vault,
            quote_vault,
            bids,
            asks,
            event_queue,
            base_lot_size,
            quote_lot_size,
            fee_rate_bps,
        })
    }

    async fn get_token_account_balance(&self, vault_pubkey: &Pubkey) -> Result<f64> {
        match self.client.try_get_token_account_balance(vault_pubkey).await {
            Ok(Some(balance)) => {
                let decimals = 6; // Default decimals, should be fetched from mint
                Ok(balance as f64 / 10_f64.powi(decimals as i32))
            }
            Ok(None) => Ok(0.0), // Account not found or invalid
            Err(_) => Ok(0.0), // Other errors
        }
    }

    fn get_token_symbol(&self, mint: &Pubkey) -> String {
        // Map known token mints to symbols
        match mint.to_string().as_str() {
            "So11111111111111111111111111111111111111112" => "SOL".to_string(),
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" => "USDC".to_string(),
            "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB" => "USDT".to_string(),
            _ => "UNKNOWN".to_string(),
        }
    }

    pub async fn get_order_book(&self, market_pubkey: &Pubkey) -> Result<(Vec<OrderBookLevel>, Vec<OrderBookLevel>)> {
        let market_data = self.fetch_market_data(market_pubkey).await?;
        
        // Fetch bids and asks
        let bids = self.parse_order_book(&market_data.bids, true).await?;
        let asks = self.parse_order_book(&market_data.asks, false).await?;
        
        Ok((bids, asks))
    }

    async fn parse_order_book(&self, _order_book_pubkey: &Pubkey, is_bids: bool) -> Result<Vec<OrderBookLevel>> {
        // Simplified order book parsing
        // In a real implementation, this would parse the Serum order book structure
        let mut levels = Vec::new();
        
        // Mock data for now
        if is_bids {
            levels.push(OrderBookLevel { price: 50.0, size: 100.0 });
            levels.push(OrderBookLevel { price: 49.5, size: 200.0 });
        } else {
            levels.push(OrderBookLevel { price: 50.5, size: 150.0 });
            levels.push(OrderBookLevel { price: 51.0, size: 250.0 });
        }
        
        Ok(levels)
    }

    pub async fn is_healthy(&self) -> bool {
        self.client.get_latest_blockhash().await.is_ok()
    }
}

#[async_trait]
impl DexClient for SerumDex {
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
        // For Serum, we would need to fetch the latest vault balances
        // This is a simplified implementation
        if let Some(updated_pool) = self.get_pool_by_tokens(&pool.token_a.mint.to_string(), &pool.token_b.mint.to_string()).await? {
            pool.reserve_a = updated_pool.reserve_a;
            pool.reserve_b = updated_pool.reserve_b;
            pool.last_updated = chrono::Utc::now();
        }
        Ok(())
    }

    fn get_dex_name(&self) -> &'static str {
        "Serum"
    }

    fn set_console_manager(&mut self, console_manager: Arc<ConsoleManager>) {
        self.console_manager = Some(console_manager);
    }
}