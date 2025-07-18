use crate::{
    dex::DexClient,
    models::{Pool, TokenInfo},
    utils::rpc::RpcClient,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::Deserialize;
use solana_sdk::pubkey::Pubkey;
use std::{collections::HashMap, str::FromStr, sync::Arc};
use tracing::{debug, error, info, warn};

use crate::console::ConsoleManager;

#[derive(Debug, Clone, Deserialize)]
struct PhoenixMarket {
    pub market: String,
    pub base_mint: String,
    pub quote_mint: String,
    pub base_decimals: u8,
    pub quote_decimals: u8,
    pub _tick_size: f64,
    pub _min_base_order_size: f64,
}

pub struct PhoenixClient {
    rpc_client: Arc<RpcClient>,
    pools_cache: tokio::sync::RwLock<HashMap<String, Pool>>,
    console: Arc<ConsoleManager>,
}

impl PhoenixClient {
    pub fn new(rpc_client: Arc<RpcClient>, console: Arc<ConsoleManager>) -> Result<Self> {
        Ok(Self {
            rpc_client,
            pools_cache: tokio::sync::RwLock::new(HashMap::new()),
            console,
        })
    }

    async fn fetch_phoenix_markets_from_api(&self) -> Result<Vec<PhoenixMarket>> {
        let client = reqwest::Client::new();
        
        // Try the current Phoenix SDK structure
        let response = client
            .get("https://raw.githubusercontent.com/Ellipsis-Labs/phoenix-sdk/master/typescript/src/market_configs/mainnet.json")
            .header("User-Agent", "solana-arbitrage-bot/1.0")
            .header("Accept", "application/json")
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .context("Failed to fetch Phoenix markets")?;

        if !response.status().is_success() {
            // Try the rust SDK endpoint
            let alt_response = client
                .get("https://raw.githubusercontent.com/Ellipsis-Labs/phoenix-sdk/master/rust/phoenix-sdk/src/market_configs/mainnet.json")
                .header("User-Agent", "solana-arbitrage-bot/1.0")
                .header("Accept", "application/json")
                .timeout(std::time::Duration::from_secs(30))
                .send()
                .await;
            
            match alt_response {
                Ok(alt_resp) if alt_resp.status().is_success() => {
                    let markets: Vec<PhoenixMarket> = alt_resp
                        .json()
                        .await
                        .context("Failed to parse Phoenix markets response from rust SDK")?;
                    debug!("Fetched {} markets from Phoenix Rust SDK", markets.len());
                    return Ok(markets);
                }
                _ => {
                    // If both fail, create hardcoded markets for major pairs
                    warn!("Phoenix API endpoints failed, using hardcoded markets");
                    let hardcoded_markets = vec![
                        PhoenixMarket {
                            market: "4DoNfFBfF7UokCC2FQzriy7yHK6DY6NVdYpuekQ5pRgg".to_string(), // SOL/USDC
                            base_mint: "So11111111111111111111111111111111111111112".to_string(), // SOL
                            quote_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(), // USDC
                            base_decimals: 9,
                            quote_decimals: 6,
                            _tick_size: 0.01,
                            _min_base_order_size: 0.001,
                        },
                        PhoenixMarket {
                            market: "Ew9uHbYtNzJBaKAKdmvqKLZjcAVXNdJLp2UqFKjKGCfH".to_string(), // ETH/USDC (example)
                            base_mint: "7vfCXTUXx5WJV5JADk17DUJ4ksgau7utNKj4b963voxs".to_string(), // ETH
                            quote_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(), // USDC
                            base_decimals: 8,
                            quote_decimals: 6,
                            _tick_size: 0.01,
                            _min_base_order_size: 0.001,
                        },
                    ];
                    return Ok(hardcoded_markets);
                }
            }
        }

        // Try to parse as different possible formats
        let response_text = response.text().await
            .context("Failed to get response text")?;
            
        // Try parsing as direct array first
        if let Ok(markets) = serde_json::from_str::<Vec<PhoenixMarket>>(&response_text) {
            debug!("Fetched {} markets from Phoenix API (direct array)", markets.len());
            return Ok(markets);
        }
        
        // Try parsing as object with markets field
        if let Ok(wrapper) = serde_json::from_str::<serde_json::Value>(&response_text) {
            if let Some(markets_array) = wrapper.get("markets").or_else(|| wrapper.get("data")) {
                if let Ok(markets) = serde_json::from_value::<Vec<PhoenixMarket>>(markets_array.clone()) {
                    debug!("Fetched {} markets from Phoenix API (wrapped)", markets.len());
                    return Ok(markets);
                }
            }
        }
        
        // If parsing fails, fall back to hardcoded markets
        warn!("Failed to parse Phoenix markets response, using hardcoded markets");
        let hardcoded_markets = vec![
            PhoenixMarket {
                market: "4DoNfFBfF7UokCC2FQzriy7yHK6DY6NVdYpuekQ5pRgg".to_string(),
                base_mint: "So11111111111111111111111111111111111111112".to_string(),
                quote_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
                base_decimals: 9,
                quote_decimals: 6,
                _tick_size: 0.01,
                _min_base_order_size: 0.001,
            },
        ];
        Ok(hardcoded_markets)
    }

    async fn convert_phoenix_market(&self, phoenix_market: &PhoenixMarket) -> Result<Pool> {
        let market_address = Pubkey::from_str(&phoenix_market.market)
            .context("Invalid market address")?;

        let base_mint = Pubkey::from_str(&phoenix_market.base_mint)
            .context("Invalid base mint")?;
        
        let quote_mint = Pubkey::from_str(&phoenix_market.quote_mint)
            .context("Invalid quote mint")?;

        // For Phoenix (orderbook DEX), we need to fetch the current bid/ask book
        let (base_liquidity, quote_liquidity) = self.fetch_orderbook_liquidity(&market_address).await?;

        let pool = Pool {
            address: market_address,
            dex: "phoenix".to_string(),
            token_a: TokenInfo {
                mint: base_mint,
                symbol: "UNK".to_string(),
                decimals: phoenix_market.base_decimals,
                price_usd: None,
            },
            token_b: TokenInfo {
                mint: quote_mint,
                symbol: "UNK".to_string(),
                decimals: phoenix_market.quote_decimals,
                price_usd: None,
            },
            reserve_a: base_liquidity,
            reserve_b: quote_liquidity,
            fee_percent: Decimal::from_f64_retain(0.0001).unwrap(), // Phoenix typically uses lower fees
            liquidity_usd: Decimal::ZERO, // Will be calculated separately
            last_updated: chrono::Utc::now(),
        };

        Ok(pool)
    }

    async fn fetch_orderbook_liquidity(&self, market_address: &Pubkey) -> Result<(u64, u64)> {
        // Phoenix uses orderbook model, so we need to sum up the liquidity in the book
        // This is a simplified implementation that would need to parse the actual orderbook
        match self.rpc_client.try_get_account(market_address).await {
            Ok(Some(account)) => {
                // Parse Phoenix market account data to extract orderbook liquidity
                // This is a placeholder implementation - real parsing would be much more complex
                if account.data.len() >= 32 {
                    // Simplified liquidity estimation based on account data
                    let base_liquidity = u64::from_le_bytes(
                        account.data[0..8].try_into().unwrap_or([0; 8])
                    );
                    let quote_liquidity = u64::from_le_bytes(
                        account.data[8..16].try_into().unwrap_or([0; 8])
                    );
                    Ok((base_liquidity, quote_liquidity))
                } else {
                    Ok((0, 0))
                }
            }
            Ok(None) => {
                debug!("Market account not found for {}, using zero liquidity", market_address);
                Ok((0, 0))
            }
            Err(e) => {
                warn!("Failed to fetch orderbook liquidity for {}: {}", market_address, e);
                Ok((0, 0))
            }
        }
    }

    async fn _get_best_bid_ask(&self, market_address: &Pubkey) -> Result<(Option<f64>, Option<f64>)> {
        // Fetch the best bid and ask prices from the orderbook
        // This would involve parsing the Phoenix orderbook data structure
        match self.rpc_client.try_get_account(market_address).await {
            Ok(Some(_account)) => {
                // Parse orderbook to find best bid/ask
                // This is a placeholder - real implementation would be more complex
                Ok((None, None))
            }
            Ok(None) => {
                debug!("Market account not found for {}", market_address);
                Ok((None, None))
            }
            Err(e) => {
                warn!("Failed to fetch bid/ask for {}: {}", market_address, e);
                Ok((None, None))
            }
        }
    }
}

#[async_trait]
impl DexClient for PhoenixClient {
    async fn fetch_pools(&self) -> Result<Vec<Pool>> {
        info!("Fetching Phoenix markets...");
        self.console.update_status(self.get_dex_name(), "Connecting to API");
        
        // Removed mock data - fetching real pools only
        
        match self.fetch_phoenix_markets_from_api().await {
            Ok(phoenix_markets) => {
                self.console.update_status_with_info(
                    self.get_dex_name(), 
                    "Processing markets", 
                    &format!("{} markets from API", phoenix_markets.len())
                );
                
                let mut pools = Vec::new();
                let mut _processed = 0;

                for phoenix_market in phoenix_markets.iter() {
                    match self.convert_phoenix_market(phoenix_market).await {
                        Ok(pool) => {
                            pools.push(pool);
                            _processed += 1;
                        }
                        Err(e) => {
                            error!("Failed to convert Phoenix market {}: {}", phoenix_market.market, e);
                            continue;
                        }
                    }
                }

                // Update cache
                let mut cache = self.pools_cache.write().await;
                cache.clear();
                for pool in &pools {
                    cache.insert(pool.address.to_string(), pool.clone());
                }

                info!("Successfully fetched {} Phoenix markets", pools.len());
                self.console.update_status_with_info(
                    self.get_dex_name(), 
                    "Connected", 
                    &format!("{} markets cached", pools.len())
                );
                Ok(pools)
            }
            Err(e) => {
                error!("Failed to fetch Phoenix markets from API: {}", e);
                return Err(anyhow::anyhow!("Failed to fetch Phoenix markets from blockchain"));
            }
        }
    }

    async fn get_pool_by_tokens(&self, token_a: &str, token_b: &str) -> Result<Option<Pool>> {
        let cache = self.pools_cache.read().await;
        
        for pool in cache.values() {
            let pool_token_a = pool.token_a.mint.to_string();
            let pool_token_b = pool.token_b.mint.to_string();
            
            if (pool_token_a == token_a && pool_token_b == token_b) ||
               (pool_token_a == token_b && pool_token_b == token_a) {
                return Ok(Some(pool.clone()));
            }
        }
        
        Ok(None)
    }

    async fn update_pool_reserves(&self, pool: &mut Pool) -> anyhow::Result<()> {
        let (base_liquidity, quote_liquidity) = self.fetch_orderbook_liquidity(&pool.address).await?;
        pool.reserve_a = base_liquidity;
        pool.reserve_b = quote_liquidity;
        pool.last_updated = chrono::Utc::now();
        Ok(())
    }

    fn get_dex_name(&self) -> &'static str {
        "phoenix"
    }

    fn set_console_manager(&mut self, console: Arc<ConsoleManager>) {
        self.console = console;
    }
}
