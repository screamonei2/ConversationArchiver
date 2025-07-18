use crate::models::{Pool, TokenInfo};
use anyhow::Result;

use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use chrono;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use crate::dex::DexClient;
use crate::console::ConsoleManager;
use async_trait::async_trait;
use std::sync::Arc;
use crate::utils::rpc::RpcClient as CustomRpcClient;

pub const SABER_PROGRAM_ID: &str = "SSwpkEEcbUqx4vtoEByFjSkhKdCT862DNVb52nZg1UZ";

// Saber Stable Swap pool discriminator


#[derive(Debug)]
pub struct SaberPool {
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub token_a_vault: Pubkey,
    pub token_b_vault: Pubkey,
    pub pool_mint: Pubkey,
    pub fee_numerator: u64,
    pub fee_denominator: u64,
    pub admin_fee_numerator: u64,
    pub admin_fee_denominator: u64,
    pub amp_factor: u64,
}

pub struct SaberDex {
    pub client: Arc<CustomRpcClient>,
    pub program_id: Pubkey,
    console_manager: Option<Arc<ConsoleManager>>,
}

impl SaberDex {
    pub fn new(rpc_client: Arc<CustomRpcClient>, console_manager: Arc<ConsoleManager>) -> Result<Self> {
        let program_id = Pubkey::from_str(SABER_PROGRAM_ID)?;
        
        Ok(Self {
            client: rpc_client,
            program_id,
            console_manager: Some(console_manager),
        })
    }

    pub async fn fetch_pools(&self) -> Result<Vec<Pool>> {
        let accounts = self.client.get_program_accounts(&self.program_id).await?;
        let mut pools = Vec::new();
        
        for (pubkey, account) in accounts {
            if account.data.len() >= 8 && self.is_saber_pool_account(&account.data) {
                if let Ok(pool_data) = self.parse_saber_pool_data(&account.data) {
                    // Get vault balances
                    let reserve_a = self.get_token_account_balance(&pool_data.token_a_vault).await.unwrap_or(0.0);
                    let reserve_b = self.get_token_account_balance(&pool_data.token_b_vault).await.unwrap_or(0.0);
                    
                    let fee_rate = pool_data.fee_numerator as f64 / pool_data.fee_denominator as f64;
                    
                    let pool = Pool {
                         address: pubkey,
                         dex: "Saber".to_string(),
                         token_a: TokenInfo {
                             mint: pool_data.token_a_mint,
                             symbol: "UNKNOWN".to_string(),
                             decimals: 6,
                             price_usd: None,
                         },
                         token_b: TokenInfo {
                             mint: pool_data.token_b_mint,
                             symbol: "UNKNOWN".to_string(),
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

    fn is_saber_pool_account(&self, data: &[u8]) -> bool {
        if data.len() < 8 {
            return false;
        }
        
        // For now, we'll use a size-based heuristic since we don't have the exact discriminator
        data.len() >= 300 && data.len() <= 500
    }

    fn parse_saber_pool_data(&self, data: &[u8]) -> Result<SaberPool> {
        if data.len() < 300 {
            return Err(anyhow::anyhow!("Invalid Saber pool data size"));
        }
        
        // Parse the Saber stable swap pool structure
        let token_a_mint = Pubkey::try_from(&data[8..40])?;
        let token_b_mint = Pubkey::try_from(&data[40..72])?;
        let token_a_vault = Pubkey::try_from(&data[72..104])?;
        let token_b_vault = Pubkey::try_from(&data[104..136])?;
        let pool_mint = Pubkey::try_from(&data[136..168])?;
        
        let fee_numerator = u64::from_le_bytes([
            data[168], data[169], data[170], data[171],
            data[172], data[173], data[174], data[175],
        ]);
        
        let fee_denominator = u64::from_le_bytes([
            data[176], data[177], data[178], data[179],
            data[180], data[181], data[182], data[183],
        ]);
        
        let admin_fee_numerator = u64::from_le_bytes([
            data[184], data[185], data[186], data[187],
            data[188], data[189], data[190], data[191],
        ]);
        
        let admin_fee_denominator = u64::from_le_bytes([
            data[192], data[193], data[194], data[195],
            data[196], data[197], data[198], data[199],
        ]);
        
        let amp_factor = u64::from_le_bytes([
            data[200], data[201], data[202], data[203],
            data[204], data[205], data[206], data[207],
        ]);
        
        Ok(SaberPool {
            token_a_mint,
            token_b_mint,
            token_a_vault,
            token_b_vault,
            pool_mint,
            fee_numerator,
            fee_denominator,
            admin_fee_numerator,
            admin_fee_denominator,
            amp_factor,
        })
    }

    async fn get_token_account_balance(&self, vault_pubkey: &Pubkey) -> Result<f64> {
        match self.client.try_get_token_account_balance(vault_pubkey).await {
            Ok(Some(balance)) => {
                let amount = balance as f64 / 1e6; // Convert from raw amount to UI amount
                Ok(amount)
            }
            Ok(None) => Ok(0.0), // Account not found or invalid
            Err(_) => Ok(0.0), // Other errors
        }
    }

    pub async fn is_healthy(&self) -> bool {
        self.client.get_latest_blockhash().await.is_ok()
    }

    // Saber-specific stable swap calculation
    pub fn calculate_stable_swap_output(
        &self,
        input_amount: f64,
        input_reserve: f64,
        output_reserve: f64,
        amp_factor: u64,
    ) -> f64 {
        // Simplified stable swap formula
        // In a real implementation, this would use the full StableSwap invariant
        let _amp = amp_factor as f64;
        let d = input_reserve + output_reserve;
        
        // Simplified calculation - in practice, this requires iterative solving
        let new_input_reserve = input_reserve + input_amount;
        let new_output_reserve = d - new_input_reserve;
        
        output_reserve - new_output_reserve
    }
}

#[async_trait]
impl DexClient for SaberDex {
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
        // For Saber, we would need to fetch the latest vault balances
        // This is a simplified implementation
        if let Some(updated_pool) = self.get_pool_by_tokens(&pool.token_a.mint.to_string(), &pool.token_b.mint.to_string()).await? {
            pool.reserve_a = updated_pool.reserve_a;
            pool.reserve_b = updated_pool.reserve_b;
            pool.last_updated = chrono::Utc::now();
        }
        Ok(())
    }

    fn get_dex_name(&self) -> &'static str {
        "Saber"
    }

    fn set_console_manager(&mut self, console_manager: Arc<ConsoleManager>) {
        self.console_manager = Some(console_manager);
    }
}