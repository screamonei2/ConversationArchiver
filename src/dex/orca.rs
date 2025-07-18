use crate::{
    dex::DexClient,
    models::{Pool, TokenInfo},
    utils::rpc::RpcClient,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use rust_decimal::Decimal;
use solana_sdk::pubkey::Pubkey;
use std::{collections::HashMap, str::FromStr, sync::Arc};
use tracing::{debug, error, info, warn};

use crate::console::ConsoleManager;

// Removed old API structs - now fetching directly from blockchain

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

    async fn fetch_orca_pools_from_blockchain(&self) -> Result<Vec<Pool>> {
        let whirlpool_program_id = Pubkey::from_str("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc")
            .context("Invalid Whirlpool program ID")?;

        // Get all Whirlpool accounts
        let accounts = self.rpc_client
            .get_program_accounts(&whirlpool_program_id)
            .await
            .context("Failed to fetch Whirlpool accounts")?;

        let mut pools = Vec::new();
        
        for (pubkey, account) in accounts {
            // Filter for Whirlpool accounts by checking discriminator and data length
            if account.data.len() >= 653 && self.is_whirlpool_account(&account.data) {
                match self.parse_whirlpool_data(&pubkey, &account.data).await {
                    Ok(pool) => {
                        pools.push(pool);
                    }
                    Err(e) => {
                        debug!("Failed to parse Whirlpool account {}: {}", pubkey, e);
                        continue;
                    }
                }
            }
        }

        info!("Fetched {} Orca pools from blockchain", pools.len());
        Ok(pools)
    }

    fn is_whirlpool_account(&self, account_data: &[u8]) -> bool {
        // Check if this is a Whirlpool account by examining the discriminator
        // Whirlpool accounts have a specific 8-byte discriminator at the beginning
        if account_data.len() < 8 {
            return false;
        }
        
        // Whirlpool discriminator (first 8 bytes)
        // This is the hash of "account:Whirlpool"
        let whirlpool_discriminator = [0x63, 0xd9, 0x96, 0xf2, 0x8c, 0x26, 0x8b, 0x8a];
        
        &account_data[0..8] == whirlpool_discriminator
    }

    async fn parse_whirlpool_data(&self, pool_address: &Pubkey, account_data: &[u8]) -> Result<Pool> {
        // Parse Whirlpool account data structure
        if account_data.len() < 653 {
            anyhow::bail!("Whirlpool account data too short");
        }

        // Extract token mints from the account data
        // Token A mint: bytes 101-133 (32 bytes)
        // Token B mint: bytes 133-165 (32 bytes)
        let token_a_mint_bytes = &account_data[101..133];
        let token_b_mint_bytes = &account_data[133..165];
        
        let token_a_mint = Pubkey::try_from(token_a_mint_bytes)
            .context("Invalid token A mint")?;
        let token_b_mint = Pubkey::try_from(token_b_mint_bytes)
            .context("Invalid token B mint")?;

        // Extract token vaults
        // Token A vault: bytes 165-197 (32 bytes)
        // Token B vault: bytes 197-229 (32 bytes)
        let token_a_vault_bytes = &account_data[165..197];
        let token_b_vault_bytes = &account_data[197..229];
        
        let token_a_vault = Pubkey::try_from(token_a_vault_bytes)
            .context("Invalid token A vault")?;
        let token_b_vault = Pubkey::try_from(token_b_vault_bytes)
            .context("Invalid token B vault")?;

        // Get vault balances
        let reserve_a = self.get_token_account_balance(&token_a_vault).await.unwrap_or(0);
        let reserve_b = self.get_token_account_balance(&token_b_vault).await.unwrap_or(0);

        // Extract fee rate (bytes 229-231, 2 bytes as u16)
        let fee_rate_bytes = &account_data[229..231];
        let fee_rate_raw = u16::from_le_bytes([fee_rate_bytes[0], fee_rate_bytes[1]]);
        let fee_rate = fee_rate_raw as f64 / 1_000_000.0; // Convert from basis points

        let pool = Pool {
            address: *pool_address,
            dex: "orca".to_string(),
            token_a: TokenInfo {
                mint: token_a_mint,
                symbol: "UNK".to_string(), // Will be resolved later
                decimals: 6, // Default, will be resolved later
                price_usd: None,
            },
            token_b: TokenInfo {
                mint: token_b_mint,
                symbol: "UNK".to_string(), // Will be resolved later
                decimals: 6, // Default, will be resolved later
                price_usd: None,
            },
            reserve_a,
            reserve_b,
            fee_percent: Decimal::from_f64_retain(fee_rate)
                .unwrap_or(Decimal::from_f64_retain(0.003).unwrap()),
            liquidity_usd: Decimal::ZERO, // Will be calculated later
            last_updated: chrono::Utc::now(),
        };

        Ok(pool)
    }

    // Removed old fetch_pool_reserves and parse_whirlpool_account methods
    // Now using parse_whirlpool_data which handles everything in one place

    async fn get_token_account_balance(&self, token_account: &Pubkey) -> Result<u64> {
        match self.rpc_client.try_get_account(token_account).await {
            Ok(Some(account)) => {
                // SPL Token account layout: amount is at bytes 64-72
                if account.data.len() >= 72 {
                    let amount_bytes = &account.data[64..72];
                    let amount = u64::from_le_bytes(
                        amount_bytes.try_into().unwrap_or([0; 8])
                    );
                    Ok(amount)
                } else {
                    Ok(0)
                }
            }
            Ok(None) => {
                debug!("Token account not found for {}", token_account);
                Ok(0)
            }
            Err(e) => {
                debug!("Failed to fetch token account balance for {}: {:?}", token_account, e);
                Ok(0)
            }
        }
    }
}

#[async_trait]
impl DexClient for OrcaClient {
    async fn fetch_pools(&self) -> Result<Vec<Pool>> {
        info!("Fetching Orca pools...");
        self.console.update_status(self.get_dex_name(), "Connecting to API");
        
        // Removed mock data - fetching real pools only
        
        match self.fetch_orca_pools_from_blockchain().await {
            Ok(pools) => {
                self.console.update_status_with_info(
                    self.get_dex_name(), 
                    "Processing pools", 
                    &format!("{} pools from blockchain", pools.len())
                );
                
                // Update cache
                let mut cache = self.pools_cache.write().await;
                cache.clear();
                for pool in &pools {
                    cache.insert(pool.address.to_string(), pool.clone());
                }

                info!("Successfully fetched {} Orca pools", pools.len());
                self.console.update_status_with_info(
                    self.get_dex_name(), 
                    "Connected", 
                    &format!("{} pools cached", pools.len())
                );
                Ok(pools)
            }
            Err(e) => {
                error!("Failed to fetch Orca pools from blockchain: {}", e);
                self.console.update_status_with_info(
                    self.get_dex_name(), 
                    "Error - Using fallback", 
                    "0 pools"
                );
                // Return empty pools instead of mock data when real data is requested
                Ok(vec![])
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

    async fn update_pool_reserves(&self, pool: &mut Pool) -> Result<()> {
        // Get fresh account data and parse it
        match self.rpc_client.try_get_account(&pool.address).await {
            Ok(Some(account)) => {
                match self.parse_whirlpool_data(&pool.address, &account.data).await {
                    Ok(updated_pool) => {
                        pool.reserve_a = updated_pool.reserve_a;
                        pool.reserve_b = updated_pool.reserve_b;
                        pool.last_updated = chrono::Utc::now();
                        Ok(())
                    }
                    Err(e) => {
                        warn!("Failed to parse updated pool data for {}: {}", pool.address, e);
                        Err(e)
                    }
                }
            }
            Ok(None) => {
                warn!("Pool account not found for {}", pool.address);
                Err(anyhow::anyhow!("Pool account not found"))
            }
            Err(e) => {
                warn!("Failed to fetch updated account data for {}: {:?}", pool.address, e);
                Err(anyhow::anyhow!("Failed to fetch account data"))
            }
        }
    }

    fn get_dex_name(&self) -> &'static str {
        "orca"
    }

    fn set_console_manager(&mut self, console: Arc<ConsoleManager>) {
        self.console = console;
    }
}
