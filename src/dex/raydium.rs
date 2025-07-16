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
use tracing::{debug, error, info};

#[derive(Debug, Clone, Deserialize)]
struct RaydiumPool {
    pub id: String,
    pub base_mint: String,
    pub quote_mint: String,
    pub base_reserve: u64,
    pub quote_reserve: u64,
    pub lp_mint: String,
    pub open_orders: String,
    pub target_orders: String,
    pub base_decimals: u8,
    pub quote_decimals: u8,
    pub state: u64,
    pub reset_flag: u64,
    pub min_size: u64,
    pub vol_max_cut_ratio: u64,
    pub amount_wave_ratio: u64,
    pub base_lot_size: u64,
    pub quote_lot_size: u64,
    pub min_price_multiplier: u64,
    pub max_price_multiplier: u64,
    pub system_decimal_value: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct RaydiumPoolsResponse {
    pub official: Vec<RaydiumPool>,
    pub unOfficial: Vec<RaydiumPool>,
}

pub struct RaydiumClient {
    rpc_client: Arc<RpcClient>,
    pools_cache: tokio::sync::RwLock<HashMap<String, Pool>>,
}

impl RaydiumClient {
    pub fn new(rpc_client: Arc<RpcClient>) -> Result<Self> {
        Ok(Self {
            rpc_client,
            pools_cache: tokio::sync::RwLock::new(HashMap::new()),
        })
    }

    async fn fetch_raydium_pools_from_api(&self) -> Result<Vec<RaydiumPool>> {
        let client = reqwest::Client::new();
        
        // Raydium's public API endpoint for pool data
        let response = client
            .get("https://api.raydium.io/v2/sdk/liquidity/mainnet.json")
            .send()
            .await
            .context("Failed to fetch Raydium pools")?;

        if !response.status().is_success() {
            anyhow::bail!("Raydium API returned error status: {}", response.status());
        }

        let pools_response: RaydiumPoolsResponse = response
            .json()
            .await
            .context("Failed to parse Raydium pools response")?;

        // Combine official and unofficial pools
        let mut all_pools = pools_response.official;
        all_pools.extend(pools_response.unOfficial);

        debug!("Fetched {} pools from Raydium API", all_pools.len());
        Ok(all_pools)
    }

    async fn convert_raydium_pool(&self, raydium_pool: &RaydiumPool) -> Result<Pool> {
        let pool_address = Pubkey::from_str(&raydium_pool.id)
            .context("Invalid pool address")?;

        let base_mint = Pubkey::from_str(&raydium_pool.base_mint)
            .context("Invalid base mint")?;
        
        let quote_mint = Pubkey::from_str(&raydium_pool.quote_mint)
            .context("Invalid quote mint")?;

        // Get current reserves (Raydium provides them in the API response)
        let (reserve_a, reserve_b) = (raydium_pool.base_reserve, raydium_pool.quote_reserve);

        // Calculate liquidity in USD (simplified)
        let liquidity_usd = self.estimate_liquidity_usd(reserve_a, reserve_b, raydium_pool.base_decimals, raydium_pool.quote_decimals).await;

        let pool = Pool {
            address: pool_address,
            dex: "raydium".to_string(),
            token_a: TokenInfo {
                mint: base_mint,
                symbol: "UNK".to_string(), // Raydium API doesn't always provide symbols
                decimals: raydium_pool.base_decimals,
                price_usd: None,
            },
            token_b: TokenInfo {
                mint: quote_mint,
                symbol: "UNK".to_string(),
                decimals: raydium_pool.quote_decimals,
                price_usd: None,
            },
            reserve_a,
            reserve_b,
            fee_percent: Decimal::from_f64_retain(0.0025).unwrap(), // Raydium typically uses 0.25%
            liquidity_usd,
            last_updated: chrono::Utc::now(),
        };

        Ok(pool)
    }

    async fn estimate_liquidity_usd(&self, reserve_a: u64, reserve_b: u64, decimals_a: u8, decimals_b: u8) -> Decimal {
        // Simplified liquidity estimation
        // In a real implementation, you'd fetch token prices from a price feed
        let reserve_a_normalized = reserve_a as f64 / 10_f64.powi(decimals_a as i32);
        let reserve_b_normalized = reserve_b as f64 / 10_f64.powi(decimals_b as i32);
        
        // Assume the quote token (token B) might be USDC/USDT with ~$1 value
        // This is a rough approximation and should be replaced with real price data
        let estimated_liquidity = reserve_b_normalized * 2.0; // Double the quote token value
        
        Decimal::from_f64_retain(estimated_liquidity).unwrap_or(Decimal::ZERO)
    }

    async fn fetch_pool_reserves(&self, pool_address: &Pubkey) -> Result<(u64, u64)> {
        match self.rpc_client.get_account_data(pool_address).await {
            Ok(account_data) => {
                // Parse Raydium AMM account data to extract reserves
                // This is a simplified implementation - real parsing would be more complex
                if account_data.len() >= 16 {
                    let reserve_a = u64::from_le_bytes(
                        account_data[0..8].try_into().unwrap_or([0; 8])
                    );
                    let reserve_b = u64::from_le_bytes(
                        account_data[8..16].try_into().unwrap_or([0; 8])
                    );
                    Ok((reserve_a, reserve_b))
                } else {
                    Ok((0, 0))
                }
            }
            Err(e) => {
                error!("Failed to fetch pool reserves for {}: {}", pool_address, e);
                Ok((0, 0))
            }
        }
    }
}

#[async_trait]
impl DexClient for RaydiumClient {
    async fn fetch_pools(&self) -> Result<Vec<Pool>> {
        info!("Fetching Raydium pools...");
        
        let raydium_pools = self.fetch_raydium_pools_from_api().await?;
        let mut pools = Vec::new();

        for raydium_pool in raydium_pools.iter() {
            match self.convert_raydium_pool(raydium_pool).await {
                Ok(pool) => {
                    pools.push(pool);
                }
                Err(e) => {
                    error!("Failed to convert Raydium pool {}: {}", raydium_pool.id, e);
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

        info!("Successfully fetched {} Raydium pools", pools.len());
        Ok(pools)
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

    async fn update_pool_reserves(&self, pool: &mut Pool) -> Result<()> {
        let (reserve_a, reserve_b) = self.fetch_pool_reserves(&pool.address).await?;
        pool.reserve_a = reserve_a;
        pool.reserve_b = reserve_b;
        pool.last_updated = chrono::Utc::now();
        Ok(())
    }

    fn get_dex_name(&self) -> &'static str {
        "raydium"
    }
}
