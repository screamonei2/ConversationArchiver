use crate::models::{Pool, TokenInfo};
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
// use anchor_lang::prelude::*; // Unused
use async_trait::async_trait;
use std::sync::Arc;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use crate::dex::DexClient;
use crate::console::ConsoleManager;
use anyhow::Result;

pub const METEORA_DLMM_PROGRAM_ID: &str = "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo";
pub const METEORA_DAMM_PROGRAM_ID: &str = "Eo7WjKq67rjJQSZxS6z3YkapzY3eMj6Xy8X5EQVn5UaB";

// Meteora DLMM (Dynamic Liquidity Market Maker) account discriminator
const DLMM_POOL_DISCRIMINATOR: [u8; 8] = [247, 237, 227, 245, 215, 195, 222, 70];

#[derive(Debug)]
pub struct MeteoraPool {
    pub token_x_mint: Pubkey,
    pub token_y_mint: Pubkey,
    pub bin_step: u16,
    pub base_fee_percentage: u16,
    pub protocol_fee_percentage: u16,
    pub liquidity: u128,
    pub reward_infos: Vec<RewardInfo>,
}

#[derive(Debug)]
pub struct RewardInfo {
    pub mint: Pubkey,
    pub vault: Pubkey,
    pub funder: Pubkey,
    pub reward_duration: u64,
    pub reward_duration_end: u64,
    pub reward_rate: u128,
    pub last_update_time: u64,
}

pub struct MeteoraDex {
    pub client: RpcClient,
    pub dlmm_program_id: Pubkey,
    pub damm_program_id: Pubkey,
    console_manager: Option<Arc<ConsoleManager>>,
}

impl MeteoraDex {
    pub fn new(rpc_client: Arc<crate::utils::rpc::RpcClient>, console: Arc<ConsoleManager>) -> Result<Self> {
        let dlmm_program_id = Pubkey::from_str(METEORA_DLMM_PROGRAM_ID)?;
        let damm_program_id = Pubkey::from_str(METEORA_DAMM_PROGRAM_ID)?;
        
        Ok(Self {
            client: RpcClient::new(rpc_client.get_url().to_string()),
            dlmm_program_id,
            damm_program_id,
            console_manager: Some(console),
        })
    }

    pub async fn fetch_pools(&self) -> Result<Vec<Pool>> {
        let mut pools = Vec::new();
        
        // Fetch DLMM pools
        let dlmm_pools = self.fetch_dlmm_pools().await?;
        pools.extend(dlmm_pools);
        
        // Fetch DAMM pools
        let damm_pools = self.fetch_damm_pools().await?;
        pools.extend(damm_pools);
        
        Ok(pools)
    }

    async fn fetch_dlmm_pools(&self) -> Result<Vec<Pool>> {
        let accounts = self.client.get_program_accounts(&self.dlmm_program_id)?;
        let mut pools = Vec::new();
        
        for (pubkey, account) in accounts {
            if account.data.len() >= 8 && self.is_dlmm_pool_account(&account.data) {
                if let Ok(pool_data) = self.parse_dlmm_pool_data(&account.data) {

                    
                    let pool = Pool {
                        address: pubkey,
                        dex: "Meteora DLMM".to_string(),
                        token_a: TokenInfo {
                            mint: pool_data.token_x_mint,
                            symbol: "UNKNOWN".to_string(),
                            decimals: 6,
                            price_usd: None,
                        },
                        token_b: TokenInfo {
                            mint: pool_data.token_y_mint,
                            symbol: "UNKNOWN".to_string(),
                            decimals: 6,
                            price_usd: None,
                        },
                        reserve_a: ((pool_data.liquidity / 2) / 1_000_000) as u64,
                        reserve_b: ((pool_data.liquidity / 2) / 1_000_000) as u64,
                        fee_percent: Decimal::from_f64(pool_data.base_fee_percentage as f64 / 100.0).unwrap_or_default(),
                        liquidity_usd: Decimal::from((pool_data.liquidity / 1_000_000) as u64),
                        last_updated: chrono::Utc::now(),
                    };
                    
                    pools.push(pool);
                }
            }
        }
        
        Ok(pools)
    }

    async fn fetch_damm_pools(&self) -> Result<Vec<Pool>> {
        let accounts = self.client.get_program_accounts(&self.damm_program_id)?;
        let mut pools = Vec::new();
        
        for (pubkey, account) in accounts {
            if account.data.len() >= 8 {
                // Basic pool parsing for DAMM - would need proper discriminator
                if account.data.len() > 100 { // Basic size check
                    let pool = Pool {
                        address: pubkey,
                        dex: "Meteora DAMM".to_string(),
                        token_a: TokenInfo {
                            mint: Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
                            symbol: "SOL".to_string(),
                            decimals: 9,
                            price_usd: None,
                        },
                        token_b: TokenInfo {
                            mint: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
                            symbol: "USDC".to_string(),
                            decimals: 6,
                            price_usd: None,
                        },
                        reserve_a: 100000000000,
                        reserve_b: 50000000,
                        fee_percent: Decimal::from_f64(0.3).unwrap_or_default(),
                        liquidity_usd: Decimal::from(5000000),
                        last_updated: chrono::Utc::now(),
                    };
                    
                    pools.push(pool);
                    
                    if pools.len() >= 5 { // Limit for now
                        break;
                    }
                }
            }
        }
        
        Ok(pools)
    }

    fn is_dlmm_pool_account(&self, data: &[u8]) -> bool {
        if data.len() < 8 {
            return false;
        }
        
        &data[0..8] == DLMM_POOL_DISCRIMINATOR
    }

    fn parse_dlmm_pool_data(&self, data: &[u8]) -> Result<MeteoraPool> {
        if data.len() < 200 { // Minimum expected size
            return Err(anyhow::anyhow!("Invalid pool data size"));
        }
        
        // Parse the pool data structure
        let token_x_mint = Pubkey::try_from(&data[8..40])?;
        let token_y_mint = Pubkey::try_from(&data[40..72])?;
        
        let bin_step = u16::from_le_bytes([data[72], data[73]]);
        let base_fee_percentage = u16::from_le_bytes([data[74], data[75]]);
        let protocol_fee_percentage = u16::from_le_bytes([data[76], data[77]]);
        
        // Simplified liquidity calculation
        let liquidity = u128::from_le_bytes([
            data[80], data[81], data[82], data[83],
            data[84], data[85], data[86], data[87],
            data[88], data[89], data[90], data[91],
            data[92], data[93], data[94], data[95],
        ]);
        
        Ok(MeteoraPool {
            token_x_mint,
            token_y_mint,
            bin_step,
            base_fee_percentage,
            protocol_fee_percentage,
            liquidity,
            reward_infos: Vec::new(), // Simplified for now
        })
    }

    pub fn is_healthy(&self) -> bool {
        // Check if we can connect to the RPC
        self.client.get_latest_blockhash().is_ok()
    }
}

#[async_trait]
impl DexClient for MeteoraDex {
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
        // For Meteora, we would need to fetch the latest pool state
        // This is a simplified implementation
        if let Some(updated_pool) = self.get_pool_by_tokens(&pool.token_a.mint.to_string(), &pool.token_b.mint.to_string()).await? {
            pool.reserve_a = updated_pool.reserve_a;
            pool.reserve_b = updated_pool.reserve_b;
            pool.last_updated = chrono::Utc::now();
        }
        Ok(())
    }

    fn get_dex_name(&self) -> &'static str {
        "Meteora"
    }

    fn set_console_manager(&mut self, console_manager: Arc<ConsoleManager>) {
        self.console_manager = Some(console_manager);
    }
}