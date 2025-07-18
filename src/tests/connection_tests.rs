use anyhow::Result;
use std::sync::Arc;
use tokio::time::{timeout, Duration};
use tracing::{info, error, warn};

use crate::{
    console::ConsoleManager,
    dex::{
        orca::OrcaClient,
        raydium::RaydiumClient,
        phoenix::PhoenixClient,
        DexClient,
    },
    dex_config::DexConfigs,
    utils::rpc::RpcClient,
    models::Pool,
};

#[derive(Debug, Clone)]
pub struct ConnectionTestResult {
    pub dex_name: String,
    pub success: bool,
    pub pools_count: Option<usize>,
    pub error_message: Option<String>,
    pub response_time_ms: u64,
}

#[derive(Clone)]
pub struct DexConnectionTester {
    rpc_client: Arc<RpcClient>,
    console_manager: Arc<ConsoleManager>,
}

impl DexConnectionTester {
    pub fn new(
        rpc_client: Arc<RpcClient>,
        console_manager: Arc<ConsoleManager>,
    ) -> Self {
        Self {
            rpc_client,
            console_manager,
        }
    }

    /// Test all enabled DEX connections concurrently
    pub async fn test_all_connections(&self) -> Result<Vec<ConnectionTestResult>> {
        info!("Starting comprehensive DEX connection tests...");
        
        let dex_configs = DexConfigs::new();
        let enabled_dexs = dex_configs.get_enabled();
        
        let mut test_tasks: Vec<tokio::task::JoinHandle<ConnectionTestResult>> = Vec::new();
        
        for dex_config in enabled_dexs {
            let self_clone = self.clone();
            let dex_name = dex_config.name.clone();
            let task = tokio::spawn(async move {
                self_clone.test_single_dex_connection(&dex_name).await
            });
            test_tasks.push(task);
        }
        
        let mut results = Vec::new();
        for task in test_tasks {
            match task.await {
                Ok(result) => results.push(result),
                Err(e) => {
                    error!("Connection test task failed: {}", e);
                }
            }
        }
        
        Ok(results)
    }

    /// Test connection to a specific DEX
    pub async fn test_single_dex_connection(&self, dex_name: &str) -> ConnectionTestResult {
        let start_time = std::time::Instant::now();
        
        info!("Testing {} connection...", dex_name);
        self.console_manager.update_service_status(
            dex_name,
            "Testing",
            "Connection test in progress",
            None,
        );

        let client_result = self.create_dex_client(dex_name).await;
        
        let result = match client_result {
            Ok(client) => {
                // Test with 30-second timeout
                match timeout(Duration::from_secs(30), client.fetch_pools()).await {
                    Ok(Ok(pools)) => {
                        let pools_count = pools.len();
                        info!(
                            "{} connection successful - fetched {} pools in {}ms",
                            dex_name,
                            pools_count,
                            start_time.elapsed().as_millis()
                        );
                        
                        self.console_manager.update_service_status(
                            dex_name,
                            "Connected",
                            "Healthy",
                            Some(format!("{} pools", pools_count)),
                        );
                        
                        ConnectionTestResult {
                            dex_name: dex_name.to_string(),
                            success: true,
                            pools_count: Some(pools_count),
                            error_message: None,
                            response_time_ms: start_time.elapsed().as_millis() as u64,
                        }
                    }
                    Ok(Err(e)) => {
                        error!("{} connection failed: {}", dex_name, e);
                        
                        self.console_manager.update_service_status(
                            dex_name,
                            "Failed",
                            "Connection error",
                            Some(e.to_string()),
                        );
                        
                        ConnectionTestResult {
                            dex_name: dex_name.to_string(),
                            success: false,
                            pools_count: None,
                            error_message: Some(e.to_string()),
                            response_time_ms: start_time.elapsed().as_millis() as u64,
                        }
                    }
                    Err(_) => {
                        error!("{} connection timed out after 30 seconds", dex_name);
                        
                        self.console_manager.update_service_status(
                            dex_name,
                            "Failed",
                            "Timeout",
                            Some("Connection timed out".to_string()),
                        );
                        
                        ConnectionTestResult {
                            dex_name: dex_name.to_string(),
                            success: false,
                            pools_count: None,
                            error_message: Some("Connection timed out after 30 seconds".to_string()),
                            response_time_ms: start_time.elapsed().as_millis() as u64,
                        }
                    }
                }
            }
            Err(e) => {
                error!("{} client creation failed: {}", dex_name, e);
                
                self.console_manager.update_service_status(
                    dex_name,
                    "Failed",
                    "Initialization error",
                    Some(e.to_string()),
                );
                
                ConnectionTestResult {
                    dex_name: dex_name.to_string(),
                    success: false,
                    pools_count: None,
                    error_message: Some(format!("Client creation failed: {}", e)),
                    response_time_ms: start_time.elapsed().as_millis() as u64,
                }
            }
        };
        
        result
    }

    /// Create a DEX client instance for testing
    async fn create_dex_client(&self, dex_name: &str) -> Result<Arc<dyn DexClient>> {
        let client: Arc<dyn DexClient> = match dex_name {
            "Orca" => Arc::new(OrcaClient::new(
                self.rpc_client.clone(),
                self.console_manager.clone(),
            )?),
            "Raydium" => Arc::new(RaydiumClient::new(
                self.rpc_client.clone(),
                self.console_manager.clone(),
            )?),
            "Phoenix" => Arc::new(PhoenixClient::new(
                self.rpc_client.clone(),
                self.console_manager.clone(),
            )?),
            _ => {
                return Err(anyhow::anyhow!("Unknown DEX: {}", dex_name));
            }
        };
        
        Ok(client)
    }

    /// Test existing DEX clients and cache their pools
    pub async fn test_and_cache_dex_clients(
        &self,
        dex_clients: &[Arc<dyn DexClient>],
    ) -> Result<(Vec<ConnectionTestResult>, Vec<Pool>)> {
        info!("Testing {} DEX clients and caching pools...", dex_clients.len());
        
        let mut test_tasks: Vec<tokio::task::JoinHandle<Result<(ConnectionTestResult, Vec<Pool>), anyhow::Error>>> = Vec::new();
        
        for (index, client) in dex_clients.iter().enumerate() {
            let client_clone = client.clone();
            let console_clone = self.console_manager.clone();
            
            let task = tokio::spawn(async move {
                let start_time = std::time::Instant::now();
                let dex_name = format!("DEX_{}", index); // We'll get the actual name from the client
                
                info!("Testing {} connection...", dex_name);
                console_clone.update_service_status(
                    &dex_name,
                    "Testing",
                    "Connection test in progress",
                    None,
                );

                // Test with 60-second timeout for better reliability
                match timeout(Duration::from_secs(60), client_clone.fetch_pools()).await {
                    Ok(Ok(pools)) => {
                        let pools_count = pools.len();
                        let response_time = start_time.elapsed().as_millis() as u64;
                        
                        info!(
                            "{} connection successful - fetched {} pools in {}ms",
                            dex_name, pools_count, response_time
                        );
                        
                        console_clone.update_service_status(
                            &dex_name,
                            "Connected",
                            "Healthy",
                            Some(format!("{} pools", pools_count)),
                        );
                        
                        Ok((ConnectionTestResult {
                            dex_name: dex_name.clone(),
                            success: true,
                            pools_count: Some(pools_count),
                            error_message: None,
                            response_time_ms: response_time,
                        }, pools))
                    }
                    Ok(Err(e)) => {
                        let response_time = start_time.elapsed().as_millis() as u64;
                        error!("{} connection failed: {}", dex_name, e);
                        
                        console_clone.update_service_status(
                            &dex_name,
                            "Failed",
                            "Connection error",
                            Some(e.to_string()),
                        );
                        
                        Ok((ConnectionTestResult {
                            dex_name: dex_name.clone(),
                            success: false,
                            pools_count: None,
                            error_message: Some(e.to_string()),
                            response_time_ms: response_time,
                        }, Vec::new()))
                    }
                    Err(_) => {
                        let response_time = start_time.elapsed().as_millis() as u64;
                        error!("{} connection timed out after 60 seconds", dex_name);
                        
                        console_clone.update_service_status(
                            &dex_name,
                            "Failed",
                            "Timeout",
                            Some("Connection timed out".to_string()),
                        );
                        
                        Ok((ConnectionTestResult {
                            dex_name: dex_name.clone(),
                            success: false,
                            pools_count: None,
                            error_message: Some("Connection timed out after 60 seconds".to_string()),
                            response_time_ms: response_time,
                        }, Vec::new()))
                    }
                }
            });
            test_tasks.push(task);
        }
        
        let mut results = Vec::new();
        let mut all_pools = Vec::new();
        
        for task in test_tasks {
            match task.await {
                Ok(Ok((result, pools))) => {
                    results.push(result);
                    all_pools.extend(pools);
                }
                Ok(Err(e)) => {
                    error!("Connection test failed: {}", e);
                }
                Err(e) => {
                    error!("Connection test task failed: {}", e);
                }
            }
        }
        
        let successful = results.iter().filter(|r| r.success).count();
        let total = results.len();
        
        info!(
            "DEX connection tests completed: {}/{} successful, {} total pools cached",
            successful, total, all_pools.len()
        );
        
        Ok((results, all_pools))
    }

    /// Run health checks on all DEXs
    pub async fn run_health_checks(&self) -> Result<Vec<ConnectionTestResult>> {
        info!("Running DEX health checks...");
        
        let results = self.test_all_connections().await?;
        
        // Print summary
        let successful = results.iter().filter(|r| r.success).count();
        let total = results.len();
        
        info!(
            "Health check complete: {}/{} DEXs healthy",
            successful, total
        );
        
        for result in &results {
            if result.success {
                info!(
                    "✅ {} - {} pools ({}ms)",
                    result.dex_name,
                    result.pools_count.unwrap_or(0),
                    result.response_time_ms
                );
            } else {
                warn!(
                    "❌ {} - {} ({}ms)",
                    result.dex_name,
                    result.error_message.as_deref().unwrap_or("Unknown error"),
                    result.response_time_ms
                );
            }
        }
        
        Ok(results)
    }

    /// Test specific functionality for each DEX
    pub async fn test_dex_functionality(&self, dex_name: &str) -> Result<()> {
        info!("Testing {} functionality...", dex_name);
        
        let client = self.create_dex_client(dex_name).await?;
        
        // Test pool fetching
        let pools = client.fetch_pools().await?;
        info!("{} fetched {} pools", dex_name, pools.len());
        
        if !pools.is_empty() {
            // Test pool lookup by tokens
            let first_pool = &pools[0];
            let token_a = first_pool.token_a.mint.to_string();
            let token_b = first_pool.token_b.mint.to_string();
            
            match client.get_pool_by_tokens(&token_a, &token_b).await? {
                Some(_) => info!("{} pool lookup successful", dex_name),
                None => warn!("{} pool lookup returned None", dex_name),
            }
            
            // Test reserve updates
            let mut test_pool = first_pool.clone();
            client.update_pool_reserves(&mut test_pool).await?;
            info!("{} reserve update successful", dex_name);
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    
    #[tokio::test]
    async fn test_connection_tester_creation() {
        let config = Config::default();
        let rpc_client = Arc::new(RpcClient::new(&config).unwrap());
        let console_manager = Arc::new(ConsoleManager::new());
        
        let _tester = DexConnectionTester::new(rpc_client, console_manager);
        // Test that the tester can be created successfully
    }
}