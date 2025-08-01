use crate::{
    config::Config,
    console::ConsoleManager,
    models::MempoolTransaction,
    utils::rpc::RpcClient,
};
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

pub struct MempoolMonitor {
    config: Config,
    _rpc_client: Arc<RpcClient>,
    detected_transactions: tokio::sync::RwLock<Vec<MempoolTransaction>>,
    console: Arc<ConsoleManager>,
}

impl MempoolMonitor {
    pub fn new(config: Config, _rpc_client: Arc<RpcClient>, console: Arc<ConsoleManager>) -> Result<Self> {
        Ok(Self {
            config,
            _rpc_client,
            detected_transactions: tokio::sync::RwLock::new(Vec::new()),
            console,
        })
    }

    pub async fn start(&self) -> Result<()> {
        if !self.config.monitoring.mempool_enabled {
            info!("Mempool monitoring disabled");
            return Ok(());
        }

        info!("Starting mempool monitor");

        let ws_url = &self.config.rpc.solana_ws_url;
        let mut reconnect_attempts = 0;
        let max_reconnect_attempts = 10; // Increased from 5 to 10
        let mut reconnect_delay_ms = 1000; // Start with 1 second
        const MAX_RECONNECT_DELAY_MS: u64 = 30000; // 30 seconds max delay
        const BACKOFF_FACTOR: f32 = 1.5; // Exponential backoff factor

        loop {
            if reconnect_attempts > 0 {
                warn!("Attempting to reconnect to WebSocket (attempt {}/{})", reconnect_attempts, max_reconnect_attempts);
                // Add jitter to prevent thundering herd problem
                let jitter = rand::random::<u64>() % 1000; // Random jitter up to 1 second
                tokio::time::sleep(tokio::time::Duration::from_millis(reconnect_delay_ms + jitter)).await;
                
                // Exponential backoff with max cap
                reconnect_delay_ms = (reconnect_delay_ms as f32 * BACKOFF_FACTOR) as u64;
                reconnect_delay_ms = reconnect_delay_ms.min(MAX_RECONNECT_DELAY_MS);
            }

            info!("MempoolMonitor: Starting WebSocket connection to {}", ws_url);
            self.console.update_service_status("MempoolMonitor", "Connecting", "Connecting to WebSocket", None);
            let ws_stream_result = connect_async(ws_url).await;

            let ws_stream = match ws_stream_result {
                Ok((stream, _)) => Ok((stream, ())),
                Err(e) => Err((anyhow::Error::new(e).context("Failed to connect to Solana WebSocket"), ())), // Convert tungstenite::Error to anyhow::Error
            };

            match ws_stream {
                Ok((ws_stream, _)) => {
                    info!("Successfully connected to WebSocket");
                    self.console.update_service_status("MempoolMonitor", "Connected", "Monitoring mempool", None);
                    reconnect_attempts = 0; // Reset attempts on successful connection

                    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

                    // Subscribe to logs for DEX program IDs
                    let subscription_request = json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "method": "logsSubscribe",
                        "params": [
                            {
                                "mentions": self.get_dex_program_ids()
                            },
                            {
                                "commitment": "confirmed"
                            }
                        ]
                    });

                    if let Err(e) = ws_sender.send(Message::Text(subscription_request.to_string())).await {
                        error!("Failed to send subscription request: {}", e);
                        continue; // Try reconnecting
                    }

                    info!("Subscribed to mempool logs");

                    // Process incoming messages with heartbeat check
                    let mut last_message_time = tokio::time::Instant::now();
                    
                    loop {
                        let timeout_result = tokio::time::timeout(
                            tokio::time::Duration::from_secs(30), // 30 second timeout
                            ws_receiver.next()
                        ).await;

                        match timeout_result {
                            Ok(Some(message_result)) => {
                                match message_result {
                                    Ok(Message::Text(text)) => {
                                        last_message_time = tokio::time::Instant::now();
                                        if let Err(e) = self.process_log_message(&text).await {
                                            error!("Error processing log message: {}", e);
                                        }
                                    }
                                    Ok(Message::Ping(data)) => {
                                        if let Err(e) = ws_sender.send(Message::Pong(data)).await {
                                            error!("Failed to send Pong: {}", e);
                                            break;
                                        }
                                        last_message_time = tokio::time::Instant::now();
                                    }
                                    Ok(Message::Close(_)) => {
                                        warn!("WebSocket connection closed by peer");
                                        break; // Break inner loop to attempt reconnection
                                    }
                                    Err(e) => {
                                        error!("WebSocket error: {}", e);
                                        break; // Break inner loop to attempt reconnection
                                    }
                                    _ => {}
                                }
                            }
                            Ok(None) => {
                                // Stream ended
                                warn!("WebSocket stream ended, attempting to reconnect...");
                                break;
                            }
                            Err(_) => {
                                // Timeout occurred
                                warn!("WebSocket stream timed out, attempting to reconnect...");
                                break;
                            }
                        }
                        
                        // Check if we've received any messages recently
                        if last_message_time.elapsed() > tokio::time::Duration::from_secs(20) {
                            warn!("No messages received for 20 seconds, sending ping");
                            if let Err(e) = ws_sender.send(Message::Ping(vec![])).await {
                                error!("Failed to send Ping: {}", e);
                                break;
                            }
                        }
                    }
                    warn!("Mempool monitor connection lost, attempting to reconnect...");
                    self.console.update_service_status("MempoolMonitor", "Reconnecting", "Connection lost", None);
                }
                Err((e, _)) => {
                    error!("Failed to connect to WebSocket: {}", e);
                    self.console.update_service_status("MempoolMonitor", "Connection failed", &format!("Error: {}", e), None);
                    reconnect_attempts += 1;
                    if reconnect_attempts > max_reconnect_attempts {
                        error!("Max reconnection attempts reached. Giving up.");
                        self.console.update_service_status("MempoolMonitor", "Connection failed", "Max reconnection attempts reached", None);
                        return Err(e);
                    }
                }
            }
        }
    }

    async fn process_log_message(&self, message: &str) -> Result<()> {
        let parsed: Value = serde_json::from_str(message)?;
        
        if let Some(params) = parsed.get("params") {
            if let Some(result) = params.get("result") {
                if let Some(value) = result.get("value") {
                    self.analyze_transaction_log(value).await?;
                }
            }
        }

        Ok(())
    }

    async fn analyze_transaction_log(&self, log_data: &Value) -> Result<()> {
        let signature = log_data.get("signature")
            .and_then(|s| s.as_str())
            .context("No signature in log")?;

        let logs = log_data.get("logs")
            .and_then(|l| l.as_array())
            .context("No logs array")?;

        // Analyze logs for swap activities
        let mut is_swap = false;
        let mut amount_info = None;
        let mut token_info = None;

        for log in logs {
            if let Some(log_str) = log.as_str() {
                // Look for common swap patterns in logs
                if log_str.contains("Program log: Instruction: Swap") ||
                   log_str.contains("swap") ||
                   log_str.contains("exchange") {
                    is_swap = true;
                }

                // Extract amount information (this is simplified)
                if log_str.contains("amount") {
                    amount_info = self.extract_amount_from_log(log_str);
                }

                // Extract token information
                if log_str.contains("mint") {
                    token_info = self.extract_token_from_log(log_str);
                }
            }
        }

        if is_swap {
            let mempool_tx = MempoolTransaction {
                signature: signature.to_string(),
                from_address: "unknown".to_string(), // Would need to extract from transaction
                to_address: None,
                amount_sol: amount_info.unwrap_or(0.0),
                token_mint: token_info,
                program_id: self.extract_program_id(log_data)?,
                timestamp: chrono::Utc::now(),
            };

            self.store_detected_transaction(mempool_tx).await;
            debug!("Detected swap transaction: {}", signature);
        }

        Ok(())
    }

    async fn store_detected_transaction(&self, transaction: MempoolTransaction) {
        let mut transactions = self.detected_transactions.write().await;
        transactions.push(transaction);
        
        // Keep only recent transactions (last 1000)
        if transactions.len() > 1000 {
            transactions.drain(0..500);
        }
    }

    pub async fn get_recent_transactions(&self, count: usize) -> Vec<MempoolTransaction> {
        let transactions = self.detected_transactions.read().await;
        transactions.iter()
            .rev()
            .take(count)
            .cloned()
            .collect()
    }

    fn get_dex_program_ids(&self) -> Vec<String> {
        vec![
            "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc".to_string(), // Orca Whirlpools
            "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".to_string(), // Raydium AMM
            "PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY".to_string(), // Phoenix
            "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(), // Raydium v4
        ]
    }

    fn extract_amount_from_log(&self, log: &str) -> Option<f64> {
        // Simple regex-based extraction (would need more sophisticated parsing)
        if let Some(start) = log.find("amount: ") {
            let amount_str = &log[start + 8..];
            if let Some(end) = amount_str.find(' ') {
                let amount_str = &amount_str[..end];
                return amount_str.parse::<f64>().ok().map(|a| a / 1_000_000_000.0);
            }
        }
        None
    }

    fn extract_token_from_log(&self, log: &str) -> Option<String> {
        // Extract token mint from log (simplified)
        if let Some(start) = log.find("mint: ") {
            let mint_str = &log[start + 6..];
            if let Some(end) = mint_str.find(' ') {
                return Some(mint_str[..end].to_string());
            }
        }
        None
    }

    fn extract_program_id(&self, _log_data: &Value) -> Result<String> {
        // Extract program ID from log data
        Ok("unknown".to_string()) // Placeholder
    }
}
