use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct DexConfig {
    pub name: String,
    pub program_id: Pubkey,
    pub enabled: bool,
    pub description: String,
}

pub struct DexConfigs;

impl DexConfigs {
    pub fn new() -> Self {
        DexConfigs
    }
    
    pub fn get_enabled(&self) -> Vec<DexConfig> {
        // Enable all DEXs to maximize arbitrage opportunities across the ecosystem
        static ENABLED_DEXS: &[&str] = &[
            "Orca", "Raydium", "Phoenix", "Meteora", "Meteora DAMM",
            "Pump.fun", "Saber", "Serum", "Lifinity"
        ];
        
        Self::get_all_dexs().into_iter().filter(|dex| {
            ENABLED_DEXS.contains(&dex.name.as_str())
        }).collect()
    }
    
    pub fn get_all_dexs() -> Vec<DexConfig> {
        vec![
            // 1. Raydium - Leading AMM on Solana
            DexConfig {
                name: "Raydium".to_string(),
                program_id: Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8").unwrap(),
                enabled: true,
                description: "First automated market maker built on Solana".to_string(),
            },
            // 2. Orca - Whirlpool concentrated liquidity
            DexConfig {
                name: "Orca".to_string(),
                program_id: Pubkey::from_str("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc").unwrap(),
                enabled: true,
                description: "Concentrated liquidity DEX with Whirlpools".to_string(),
            },
            // 3. Meteora - DLMM (Dynamic Liquidity Market Maker)
            DexConfig {
                name: "Meteora".to_string(),
                program_id: Pubkey::from_str("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo").unwrap(),
                enabled: true,
                description: "Dynamic Liquidity Market Maker with optimized capital efficiency".to_string(),
            },

            // 5. Phoenix - Order book DEX
            DexConfig {
                name: "Phoenix".to_string(),
                program_id: Pubkey::from_str("PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY").unwrap(),
                enabled: true,
                description: "High-performance order book DEX".to_string(),
            },
            // 6. Pump.fun - Meme token launchpad and DEX
            DexConfig {
                name: "Pump.fun".to_string(),
                program_id: Pubkey::from_str("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P").unwrap(),
                enabled: true,
                description: "Meme token launchpad with integrated trading".to_string(),
            },
            // 7. Saber - Stable swap DEX
            DexConfig {
                name: "Saber".to_string(),
                program_id: Pubkey::from_str("SSwpkEEcbUqx4vtoEByFjSkhKdCT862DNVb52nZg1UZ").unwrap(),
                enabled: true,
                description: "Stable swap protocol for pegged assets".to_string(),
            },
            // 8. Serum - Order book DEX (used by Aldrin and others)
            DexConfig {
                name: "Serum".to_string(),
                program_id: Pubkey::from_str("9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin").unwrap(),
                enabled: true,
                description: "Decentralized order book exchange".to_string(),
            },
            // 9. Lifinity - Oracle-based proactive market maker
            DexConfig {
                name: "Lifinity".to_string(),
                program_id: Pubkey::from_str("EewxydAPCCVuNEyrVN68PuSYdQ7wKn27V9Gjeoi8dy3S").unwrap(),
                enabled: true,
                description: "First proactive market maker with oracle-based pricing".to_string(),
            },
            // 10. Meteora DAMM - Dynamic AMM Pools
            DexConfig {
                name: "Meteora DAMM".to_string(),
                program_id: Pubkey::from_str("Eo7WjKq67rjJQSZxS6z3YkapzY3eMj6Xy8X5EQVn5UaB").unwrap(),
                enabled: true,
                description: "Meteora Dynamic AMM Pools for enhanced liquidity".to_string(),
            },
        ]
    }
    
    pub fn get_enabled_dexs() -> Vec<DexConfig> {
        // Start with proven DEXs, gradually enable others after testing
        Self::get_all_dexs().into_iter().filter(|dex| {
            matches!(dex.name.as_str(), 
                "Orca" | "Raydium" | "Phoenix" | "Meteora"
            )
        }).collect()
    }
    
    pub fn get_dex_by_name(name: &str) -> Option<DexConfig> {
        Self::get_all_dexs().into_iter().find(|dex| dex.name.to_lowercase() == name.to_lowercase())
    }
    
    pub fn get_dex_by_program_id(program_id: &Pubkey) -> Option<DexConfig> {
        Self::get_all_dexs().into_iter().find(|dex| dex.program_id == *program_id)
    }
}