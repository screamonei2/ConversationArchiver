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

use crate::console::ConsoleManager;

#[derive(Debug, Clone, Deserialize)]
struct OrcaPool {
    pub address: String,
    pub token_a: OrcaToken,
    pub token_b: OrcaToken,
    pub liquidity: f64,
    pub fee_rate: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct OrcaToken {
    pub mint: String,
    pub symbol: String,
    pub decimals: u8,
}

pub struct OrcaClient {
    rpc_client: Arc<RpcClient>,
    pools_cache: tokio::sync::RwLock<HashMap<String, Pool>>,
    console: Arc<ConsoleManager>,
}

impl OrcaClient {
    pub fn new(rpc_client: Arc<RpcClient>, console: Arc<ConsoleManager>) -> Result<Self> {
        Ok(Self {
            rpc_client,
            pools_cache: tokio::sync::RwLock::new(HashMap::new()),
            console,
        })
    }

    async fn fetch_orca_pools_from_api(&self) -> Result<Vec<OrcaPool>> {
        let client = reqwest::Client::new();
        
        // Orca's public API endpoint for pool data
        let response = client
            .get("https://api.orca.so/v1/whirlpool/list")
            .send()
            .await
            .context("Failed to fetch Orca pools")?;

        if !response.status().is_success() {
            anyhow::bail!("Orca API returned error status: {}", response.status());
        }

        let pools: Vec<OrcaPool> = response
            .json()
            .await
            .context("Failed to parse Orca pools response")?;

        debug!("Fetched {} pools from Orca API", pools.len());
        Ok(pools)
    }

    async fn convert_orca_pool(&self, orca_pool: &OrcaPool) -> Result<Pool> {
        let pool_address = Pubkey::from_str(&orca_pool.address)
            .context("Invalid pool address")?;

        let token_a_mint = Pubkey::from_str(&orca_pool.token_a.mint)
            .context("Invalid token A mint")?;
        
        let token_b_mint = Pubkey::from_str(&orca_pool.token_b.mint)
            .context("Invalid token B mint")?;

        // Get current reserves from the blockchain
        let (reserve_a, reserve_b) = self.fetch_pool_reserves(&pool_address).await?;

        let pool = Pool {
            address: pool_address,
            dex: "orca".to_string(),
            token_a: TokenInfo {
                mint: token_a_mint,
                symbol: orca_pool.token_a.symbol.clone(),
                decimals: orca_pool.token_a.decimals,
                price_usd: None, // Will be fetched separately if needed
            },
            token_b: TokenInfo {
                mint: token_b_mint,
                symbol: orca_pool.token_b.symbol.clone(),
                decimals: orca_pool.token_b.decimals,
                price_usd: None,
            },
            reserve_a,
            reserve_b,
            fee_percent: Decimal::from_f64_retain(orca_pool.fee_rate)
                .unwrap_or(Decimal::from_f64_retain(0.003).unwrap()), // Default 0.3%
            liquidity_usd: Decimal::from_f64_retain(orca_pool.liquidity)
                .unwrap_or(Decimal::ZERO),
            last_updated: chrono::Utc::now(),
        };

        Ok(pool)
    }

    async fn fetch_pool_reserves(&self, pool_address: &Pubkey) -> Result<(u64, u64)> {
        // This would typically involve fetching the pool's account data
        // and parsing the reserves. For now, we'll use a placeholder implementation
        // that would need to be replaced with actual Orca whirlpool account parsing
        
        match self.rpc_client.get_account_data(pool_address).await {
            Ok(account_data) => {
                // Parse Orca whirlpool account data to extract reserves
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
                Ok((0, 0)) // Return zero reserves on error
            }
        }
    }
}

#[async_trait]
impl DexClient for OrcaClient {
    async fn fetch_pools(&self) -> Result<Vec<Pool>> {
        info!("Fetching Orca pools...");
        self.console.update_status(self.get_dex_name(), "Fetching pools");
        
        let orca_pools = self.fetch_orca_pools_from_api().await?;
        let mut pools = Vec::new();

        for orca_pool in orca_pools.iter() {
            match self.convert_orca_pool(orca_pool).await {
                Ok(pool) => {
                    pools.push(pool);
                }
                Err(e) => {
                    error!("Failed to convert Orca pool {}: {}", orca_pool.address, e);
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

        info!("Successfully fetched {} Orca pools", pools.len());
        self.console.update_status(self.get_dex_name(), &format!("Fetched {} pools", pools.len()));
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
        "orca"
    }

    fn set_console_manager(&mut self, console: Arc<ConsoleManager>) {
        self.console = console;
    }
}
