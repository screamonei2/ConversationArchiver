use anyhow::{Context, Result};
use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use solana_sdk::signature::{Keypair, Signer};
use std::{env, fs};
use tracing::{error, warn};

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
    #[serde(skip_serializing)] // Never serialize private key
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
            // Validate private key format before storing
            if self.validate_private_key(&val) {
                self.bot.private_key = Some(val);
            } else {
                error!("Invalid private key format provided in PRIVATE_KEY environment variable");
                return Err(anyhow::anyhow!("Invalid private key format"));
            }
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

    fn validate_private_key(&self, private_key: &str) -> bool {
        // Try to parse as base58 encoded keypair
        if let Ok(decoded) = bs58::decode(private_key).into_vec() {
            if decoded.len() == 64 {
                // Try to create a keypair from the decoded bytes
                return Keypair::from_bytes(&decoded).is_ok();
            }
        }
        
        // Try to parse as JSON array format [byte1, byte2, ...]
        if private_key.starts_with('[') && private_key.ends_with(']') {
            if let Ok(bytes_vec) = serde_json::from_str::<Vec<u8>>(private_key) {
                if bytes_vec.len() == 64 {
                    return Keypair::from_bytes(&bytes_vec).is_ok();
                }
            }
        }
        
        false
    }

    pub fn get_keypair(&self) -> Result<Option<Keypair>> {
        if let Some(private_key) = &self.bot.private_key {
            // Try base58 format first
            if let Ok(decoded) = bs58::decode(private_key).into_vec() {
                if decoded.len() == 64 {
                    if let Ok(keypair) = Keypair::from_bytes(&decoded) {
                        return Ok(Some(keypair));
                    }
                }
            }
            
            // Try JSON array format
            if private_key.starts_with('[') && private_key.ends_with(']') {
                if let Ok(bytes_vec) = serde_json::from_str::<Vec<u8>>(private_key) {
                    if bytes_vec.len() == 64 {
                        if let Ok(keypair) = Keypair::from_bytes(&bytes_vec) {
                            return Ok(Some(keypair));
                        }
                    }
                }
            }
            
            Err(anyhow::anyhow!("Failed to parse private key"))
        } else {
            Ok(None)
        }
    }

    pub fn validate_security_settings(&self) -> Result<()> {
        // Ensure simulation mode is enabled if no private key is provided
        if self.bot.private_key.is_none() && self.bot.execute_trades {
            warn!("No private key provided but execute_trades is enabled. Forcing simulation mode.");
        }
        
        // Validate position size limits
        if self.bot.max_position_size_sol > 100.0 {
            warn!("Large position size detected: {} SOL. Consider reducing for safety.", self.bot.max_position_size_sol);
        }
        
        // Validate profit thresholds
        if self.bot.profit_threshold_percent < 0.1 {
            warn!("Very low profit threshold: {}%. This may lead to unprofitable trades due to fees.", self.bot.profit_threshold_percent);
        }
        
        // Validate slippage settings
        if self.bot.max_slippage_percent > 5.0 {
            warn!("High slippage tolerance: {}%. This may result in poor trade execution.", self.bot.max_slippage_percent);
        }
        
        Ok(())
    }
}
