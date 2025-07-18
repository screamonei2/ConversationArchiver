use crate::{
    config::Config,
    models::WhaleActivity,
    types::TradeDirection,
    utils::rpc::RpcClient,
    console::ConsoleManager,
};
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use solana_sdk::pubkey::Pubkey;
use std::{collections::HashSet, str::FromStr, sync::Arc};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

pub struct WhaleMonitor {
    config: Config,
    rpc_client: Arc<RpcClient>,
    whale_addresses: HashSet<Pubkey>,
    detected_activities: tokio::sync::RwLock<Vec<WhaleActivity>>,
    console: Arc<ConsoleManager>,
}

impl WhaleMonitor {
    pub fn new(config: Config, rpc_client: Arc<RpcClient>, console: Arc<ConsoleManager>) -> Result<Self> {
        let whale_addresses: HashSet<Pubkey> = config
            .monitoring
            .whale_wallet_addresses
            .iter()
            .filter_map(|addr| Pubkey::from_str(addr).ok())
            .collect();

        Ok(Self {
            config,
            rpc_client,
            whale_addresses,
            detected_activities: tokio::sync::RwLock::new(Vec::new()),
            console,
        })
    }

    pub async fn start(&self) -> Result<()> {
        if !self.config.monitoring.whale_tracking_enabled {
            info!("Whale tracking disabled");
            return Ok(());
        }

        if self.whale_addresses.is_empty() {
            warn!("No whale addresses configured for monitoring");
            return Ok(());
        }

        info!("Starting whale monitor for {} addresses", self.whale_addresses.len());

        self.console.update_status("WhaleMonitor", "Connecting");
        let ws_url = &self.config.rpc.solana_ws_url;
        let (ws_stream, _) = connect_async(ws_url).await
            .context("Failed to connect to Solana WebSocket")?;

        self.console.update_status("WhaleMonitor", "Connected");

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        // Subscribe to account changes for whale addresses
        for whale_address in &self.whale_addresses {
            let subscription_request = json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "accountSubscribe",
                "params": [
                    whale_address.to_string(),
                    {
                        "commitment": "confirmed",
                        "encoding": "base64"
                    }
                ]
            });

            ws_sender.send(Message::Text(subscription_request.to_string())).await
                .context("Failed to send whale address subscription")?;
        }

        // Also subscribe to signature notifications
        self.subscribe_to_signature_notifications(&mut ws_sender).await?;

        info!("Subscribed to whale account changes");

        // Process incoming messages
        while let Some(message) = ws_receiver.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    if let Err(e) = self.process_whale_message(&text).await {
                        error!("Error processing whale message: {}", e);
                    }
                }
                Ok(Message::Close(_)) => {
                    warn!("WebSocket connection closed");
                    self.console.update_status("WhaleMonitor", "Disconnected");
                    break;
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    self.console.update_status("WhaleMonitor", &format!("Error: {}", e));
                    break;
                }
                _ => {}
            }
        }

        warn!("Whale monitor stopped");
        self.console.update_status("WhaleMonitor", "Stopped");
        Ok(())
    }

    async fn subscribe_to_signature_notifications(&self, ws_sender: &mut futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, tokio_tungstenite::tungstenite::Message>) -> Result<()> {
        // Subscribe to program logs that might indicate whale activity
        let subscription_request = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "logsSubscribe",
            "params": [
                {
                    "mentions": self.get_monitored_programs()
                },
                {
                    "commitment": "confirmed"
                }
            ]
        });

        ws_sender.send(Message::Text(subscription_request.to_string())).await
            .context("Failed to send program logs subscription")?;

        Ok(())
    }

    async fn process_whale_message(&self, message: &str) -> Result<()> {
        let parsed: Value = serde_json::from_str(message)?;
        
        if let Some(method) = parsed.get("method") {
            match method.as_str() {
                Some("accountNotification") => {
                    self.handle_account_notification(&parsed).await?;
                }
                Some("logsNotification") => {
                    self.handle_logs_notification(&parsed).await?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn handle_account_notification(&self, notification: &Value) -> Result<()> {
        if let Some(params) = notification.get("params") {
            if let Some(result) = params.get("result") {
                if let Some(value) = result.get("value") {
                    let pubkey = params.get("subscription")
                        .and_then(|s| s.as_str())
                        .context("No subscription ID")?;

                    debug!("Account change detected for whale: {}", pubkey);
                    
                    // Analyze the account change for trading activity
                    self.analyze_account_change(value).await?;
                }
            }
        }

        Ok(())
    }

    async fn handle_logs_notification(&self, notification: &Value) -> Result<()> {
        if let Some(params) = notification.get("params") {
            if let Some(result) = params.get("result") {
                if let Some(value) = result.get("value") {
                    if let Some(signature) = value.get("signature").and_then(|s| s.as_str()) {
                        // Get transaction details to check if it involves whale addresses
                        if let Ok(tx_info) = self.analyze_transaction_for_whales(signature).await {
                            if let Some(whale_activity) = tx_info {
                                self.store_whale_activity(whale_activity).await;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn analyze_account_change(&self, account_data: &Value) -> Result<()> {
        // Analyze account data changes
        // This would involve parsing the account data to understand what changed
        debug!("Analyzing account change: {:?}", account_data);
        Ok(())
    }

    async fn analyze_transaction_for_whales(&self, signature: &str) -> Result<Option<WhaleActivity>> {
        // Fetch transaction details
        let transaction_info = self.rpc_client.get_transaction_info(signature).await?;
        
        // Check if transaction involves any whale addresses
        for whale_address in &self.whale_addresses {
            if self.transaction_involves_address(&transaction_info, whale_address) {
                // Parse transaction to extract trading details
                if let Some(whale_activity) = self.extract_whale_activity(&transaction_info, whale_address, signature).await? {
                    info!("Detected whale activity: {} - {} SOL", 
                          whale_address, 
                          whale_activity.amount as f64 / 1_000_000_000.0);
                    return Ok(Some(whale_activity));
                }
            }
        }

        Ok(None)
    }

    fn transaction_involves_address(&self, transaction_info: &Value, address: &Pubkey) -> bool {
        // Check if the transaction involves the specified address
        // This is a simplified implementation
        if let Some(account_keys) = transaction_info.get("transaction")
            .and_then(|t| t.get("message"))
            .and_then(|m| m.get("accountKeys"))
            .and_then(|ak| ak.as_array()) {
            
            for key in account_keys {
                if let Some(key_str) = key.as_str() {
                    if let Ok(pubkey) = Pubkey::from_str(key_str) {
                        if pubkey == *address {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    async fn extract_whale_activity(&self, transaction_info: &Value, whale_address: &Pubkey, signature: &str) -> Result<Option<WhaleActivity>> {
        // Extract trading activity details from transaction
        // This is a simplified implementation that would need to be much more sophisticated
        
        // Check if this is a significant transaction
        let sol_amount = self.extract_sol_amount(transaction_info)?;
        
        if sol_amount < self.config.monitoring.min_whale_transaction_sol {
            return Ok(None);
        }

        // Determine trade direction and details
        let direction = if self.is_buy_transaction(transaction_info) {
            TradeDirection::Buy
        } else {
            TradeDirection::Sell
        };

        let whale_activity = WhaleActivity {
            wallet_address: *whale_address,
            transaction_signature: signature.to_string(),
            token_mint: self.extract_token_mint(transaction_info).unwrap_or("unknown".to_string()),
            amount: (sol_amount * 1_000_000_000.0) as u64, // Convert to lamports
            direction,
            dex: self.identify_dex(transaction_info).unwrap_or("unknown".to_string()),
            timestamp: chrono::Utc::now(),
            price_impact: None, // Would need to calculate
        };

        Ok(Some(whale_activity))
    }

    fn extract_sol_amount(&self, _transaction_info: &Value) -> Result<f64> {
        // Extract SOL amount from transaction (simplified)
        // In reality, this would need to parse instruction data and account changes
        Ok(0.0) // Placeholder
    }

    fn is_buy_transaction(&self, _transaction_info: &Value) -> bool {
        // Determine if this is a buy or sell transaction
        // This would involve analyzing the instruction data
        true // Placeholder
    }

    fn extract_token_mint(&self, _transaction_info: &Value) -> Option<String> {
        // Extract the token mint address from transaction
        None // Placeholder
    }

    fn identify_dex(&self, transaction_info: &Value) -> Option<String> {
        // Identify which DEX was used based on program IDs
        if let Some(instructions) = transaction_info.get("transaction")
            .and_then(|t| t.get("message"))
            .and_then(|m| m.get("instructions"))
            .and_then(|i| i.as_array()) {
            
            for instruction in instructions {
                if let Some(program_id_index) = instruction.get("programIdIndex").and_then(|i| i.as_u64()) {
                    // Map program ID to DEX name (simplified)
                    match program_id_index {
                        _ => return Some("unknown".to_string()),
                    }
                }
            }
        }
        None
    }

    async fn store_whale_activity(&self, activity: WhaleActivity) {
        let mut activities = self.detected_activities.write().await;
        activities.push(activity);
        
        // Keep only recent activities (last 1000)
        if activities.len() > 1000 {
            activities.drain(0..500);
        }
    }

    pub async fn get_recent_whale_activities(&self, count: usize) -> Vec<WhaleActivity> {
        let activities = self.detected_activities.read().await;
        activities.iter()
            .rev()
            .take(count)
            .cloned()
            .collect()
    }

    fn get_monitored_programs(&self) -> Vec<String> {
        vec![
            "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc".to_string(), // Orca Whirlpools
            "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".to_string(), // Raydium AMM
            "PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY".to_string(), // Phoenix
        ]
    }
}
