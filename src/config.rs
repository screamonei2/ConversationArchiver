use anyhow::{Context, Result};
use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use std::{env, fs};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub bot: BotConfig,
    pub rpc: RpcConfig,
    pub dexs: DexConfig,
    pub monitoring: MonitoringConfig,
    pub risk_management: RiskManagementConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    pub profit_threshold_percent: f64,
    pub max_slippage_percent: f64,
    pub min_liquidity_usd: f64,
    pub cooldown_seconds: u64,
    pub max_position_size_sol: f64,
    pub execute_trades: bool,
    pub simulation_mode: bool,
    pub private_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    pub solana_rpc_url: String,
    pub solana_ws_url: String,
    pub quicknode_rpc_url: Option<String>,
    pub quicknode_ws_url: Option<String>,
    pub max_requests_per_second: u32,
    pub burst_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexConfig {
    pub enabled: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub min_whale_transaction_sol: f64,
    pub mempool_enabled: bool,
    pub whale_tracking_enabled: bool,
    pub whale_wallet_addresses: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskManagementConfig {
    pub max_consecutive_losses: u32,
    pub daily_loss_limit_sol: f64,
    pub position_sizing_enabled: bool,
}

impl Config {
    pub fn load() -> Result<Self> {
        // Load .env file if it exists
        if let Err(_) = dotenv() {
            tracing::warn!("No .env file found, using environment variables and config file");
        }

        // Try to load from config.toml first
        let config_path = "config.toml";
        let mut config = if let Ok(content) = fs::read_to_string(config_path) {
            toml::from_str::<Config>(&content)
                .context("Failed to parse config.toml")?
        } else {
            // Default configuration
            Config {
                bot: BotConfig {
                    profit_threshold_percent: 0.5,
                    max_slippage_percent: 1.0,
                    min_liquidity_usd: 10000.0,
                    cooldown_seconds: 5,
                    max_position_size_sol: 1.0,
                    execute_trades: false,
                    simulation_mode: true,
                    private_key: None,
                },
                rpc: RpcConfig {
                    solana_rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
                    solana_ws_url: "wss://api.mainnet-beta.solana.com".to_string(),
                    quicknode_rpc_url: None,
                    quicknode_ws_url: None,
                    max_requests_per_second: 10,
                    burst_size: 20,
                },
                dexs: DexConfig {
                    enabled: vec!["orca".to_string(), "raydium".to_string(), "phoenix".to_string()],
                },
                monitoring: MonitoringConfig {
                    min_whale_transaction_sol: 10.0,
                    mempool_enabled: true,
                    whale_tracking_enabled: true,
                    whale_wallet_addresses: vec![],
                },
                risk_management: RiskManagementConfig {
                    max_consecutive_losses: 5,
                    daily_loss_limit_sol: 10.0,
                    position_sizing_enabled: true,
                },
            }
        };

        // Override with environment variables
        config.override_with_env()?;

        Ok(config)
    }

    fn override_with_env(&mut self) -> Result<()> {
        // Bot configuration
        if let Ok(val) = env::var("PROFIT_THRESHOLD_PERCENT") {
            self.bot.profit_threshold_percent = val.parse()?;
        }
        if let Ok(val) = env::var("MAX_SLIPPAGE_PERCENT") {
            self.bot.max_slippage_percent = val.parse()?;
        }
        if let Ok(val) = env::var("MIN_LIQUIDITY_USD") {
            self.bot.min_liquidity_usd = val.parse()?;
        }
        if let Ok(val) = env::var("COOLDOWN_SECONDS") {
            self.bot.cooldown_seconds = val.parse()?;
        }
        if let Ok(val) = env::var("MAX_POSITION_SIZE_SOL") {
            self.bot.max_position_size_sol = val.parse()?;
        }
        if let Ok(val) = env::var("EXECUTE_TRADES") {
            self.bot.execute_trades = val.parse()?;
        }
        if let Ok(val) = env::var("SIMULATION_MODE") {
            self.bot.simulation_mode = val.parse()?;
        }
        if let Ok(val) = env::var("PRIVATE_KEY") {
            self.bot.private_key = Some(val);
        }

        // RPC configuration
        if let Ok(val) = env::var("SOLANA_RPC_URL") {
            self.rpc.solana_rpc_url = val;
        }
        if let Ok(val) = env::var("SOLANA_WS_URL") {
            self.rpc.solana_ws_url = val;
        }
        if let Ok(val) = env::var("QUICKNODE_RPC_URL") {
            self.rpc.quicknode_rpc_url = Some(val);
        }
        if let Ok(val) = env::var("QUICKNODE_WS_URL") {
            self.rpc.quicknode_ws_url = Some(val);
        }

        // Monitoring configuration
        if let Ok(val) = env::var("MIN_WHALE_TRANSACTION_SOL") {
            self.monitoring.min_whale_transaction_sol = val.parse()?;
        }
        if let Ok(val) = env::var("WHALE_WALLET_ADDRESSES") {
            self.monitoring.whale_wallet_addresses = val
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        Ok(())
    }
}
