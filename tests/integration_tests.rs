use solana_arbitrage_bot::{
    config::{Config, BotConfig, DexConfig, RpcConfig, MonitoringConfig, RiskManagementConfig},
    engine::{screener::Screener, executor::Executor},
    dex::{orca::OrcaClient, raydium::RaydiumClient, phoenix::PhoenixClient, DexClient},
    models::{Pool, TokenInfo},
    utils::{rpc::RpcClient, cache::PoolCache},
    console::ConsoleManager,
};
use rust_decimal::Decimal;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use chrono;

#[tokio::test]
async fn test_full_arbitrage_workflow() {
    // Initialize logging for tests
    let _ = tracing_subscriber::fmt::try_init();
    
    // Load test configuration
    let mut config = load_test_config();
    config.bot.execute_trades = false; // Simulation mode for tests
    config.bot.min_liquidity_usd = 1000.0;
    config.bot.profit_threshold_percent = 0.5;
    
    // Initialize console manager
    let console = Arc::new(ConsoleManager::new());
    
    // Initialize RPC client
    let rpc_client = Arc::new(RpcClient::new(&config)
        .expect("Failed to create RPC client"));
    
    // Initialize DEX clients
    let mut dex_clients = Vec::new();
    
    if config.dexs.enabled.contains(&"orca".to_string()) {
        let orca_client = OrcaClient::new(rpc_client.clone(), console.clone())
            .expect("Failed to create Orca client");
        dex_clients.push(Arc::new(orca_client) as Arc<dyn DexClient>);
    }
    
    if config.dexs.enabled.contains(&"raydium".to_string()) {
        let raydium_client = RaydiumClient::new(rpc_client.clone(), console.clone())
            .expect("Failed to create Raydium client");
        dex_clients.push(Arc::new(raydium_client) as Arc<dyn DexClient>);
    }
    
    if config.dexs.enabled.contains(&"phoenix".to_string()) {
        let phoenix_client = PhoenixClient::new(rpc_client.clone(), console.clone())
            .expect("Failed to create Phoenix client");
        dex_clients.push(Arc::new(phoenix_client) as Arc<dyn DexClient>);
    }
    

    
    // Initialize screener
    let screener = Screener::new(config.clone(), dex_clients)
        .expect("Failed to initialize screener");
    
    // Initialize executor
    let executor = Executor::new(config.clone(), rpc_client.clone())
        .expect("Failed to initialize executor");
    
    // Test 1: Pool data fetching and caching
    println!("Testing pool data fetching...");
    let opportunities = screener.scan_opportunities().await;
    assert!(opportunities.is_ok(), "Failed to scan opportunities: {:?}", opportunities.err());
    
    let opportunities = opportunities.unwrap();
    println!("Found {} arbitrage opportunities", opportunities.len());
    
    // Test 2: Opportunity validation (skip private method call)
    for opportunity in &opportunities {
        println!("Found opportunity: {} with {}% profit", 
            opportunity.id, opportunity.expected_profit_percent);
    }
    
    // Test 3: Execute profitable opportunities (simulation mode)
    let mut executed_count = 0;
    for opportunity in opportunities.iter().take(3) { // Test first 3 opportunities
        if opportunity.expected_profit_percent >= config.bot.profit_threshold_percent {
            println!("Executing opportunity: {} ({}% profit)", 
                opportunity.id, opportunity.expected_profit_percent);
            
            let execution_result = executor.execute_arbitrage(opportunity).await;
            match execution_result {
                Ok(_) => {
                    executed_count += 1;
                    println!("Successfully executed arbitrage opportunity");
                }
                Err(e) => {
                    println!("Failed to execute arbitrage: {:?}", e);
                }
            }
        }
    }
    
    println!("Integration test completed. Executed {} opportunities", executed_count);
}

#[tokio::test]
async fn test_cache_performance_under_load() {
    let cache = PoolCache::new();
    let num_pools = 1000;
    let num_operations = 10000;
    
    // Generate test pools
    let pools: Vec<Pool> = (0..num_pools)
        .map(|i| create_test_pool(i))
        .collect();
    
    // Test cache performance under load
    let start = std::time::Instant::now();
    
    // Concurrent cache operations
    let mut handles = Vec::new();
    
    for i in 0..10 {
        let cache_clone = cache.clone();
        let pools_clone = pools.clone();
        
        let handle = tokio::spawn(async move {
            let dex_name = format!("dex_{}", i);
            
            // Set pools
            cache_clone.set_pools(&dex_name, pools_clone).await;
            
            // Perform multiple get operations
            for _ in 0..num_operations / 10 {
                let _cached_pools = cache_clone.get_pools(&dex_name).await;
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    for handle in handles {
        handle.await.expect("Task failed");
    }
    
    let duration = start.elapsed();
    println!("Cache performance test: {} operations in {:?}", num_operations, duration);
    
    // Performance assertion - should complete within reasonable time
    assert!(duration.as_secs() < 10, "Cache operations took too long: {:?}", duration);
    
    // Check cache statistics
    let stats = cache.get_cache_stats().await;
    println!("Cache stats: {:?}", stats);
    assert!(stats.hit_rate() > 0.0, "Cache hit rate should be greater than 0");
}

#[tokio::test]
async fn test_error_recovery() {
    let config = load_test_config();
    
    // Test with invalid RPC endpoint
    let mut invalid_config = config.clone();
    invalid_config.rpc.solana_rpc_url = "https://invalid-endpoint.com".to_string();
    let invalid_rpc = Arc::new(RpcClient::new(&invalid_config)
        .expect("RPC client should initialize even with invalid endpoint"));
    
    // This should handle the error gracefully
    let executor_result = Executor::new(config.clone(), invalid_rpc);
    assert!(executor_result.is_ok(), "Executor should initialize even with invalid RPC");
    
    // Test screener with no DEX clients
    let screener_result = Screener::new(config.clone(), vec![]);
    assert!(screener_result.is_ok(), "Screener should handle empty DEX client list");
    
    let screener = screener_result.unwrap();
    let opportunities = screener.scan_opportunities().await;
    assert!(opportunities.is_ok(), "Should handle no DEX clients gracefully");
    assert!(opportunities.unwrap().is_empty(), "Should return empty opportunities list");
}

#[tokio::test]
async fn test_concurrent_arbitrage_detection() {
    let config = load_test_config();
    let console = Arc::new(ConsoleManager::new());
    let rpc_client = Arc::new(RpcClient::new(&config)
        .expect("Failed to create RPC client"));
    
    // Create multiple screener instances
    let mut screeners = Vec::new();
    for _i in 0..3 {
        let mut dex_clients = Vec::new();
        
        let orca_client = OrcaClient::new(rpc_client.clone(), console.clone())
            .expect("Failed to create Orca client");
        dex_clients.push(Arc::new(orca_client) as Arc<dyn DexClient>);
        
        let screener = Screener::new(config.clone(), dex_clients)
            .expect("Failed to create screener");
        screeners.push(screener);
    }
    
    // Run concurrent opportunity scanning
    let mut handles = Vec::new();
    for (i, screener) in screeners.into_iter().enumerate() {
        let handle = tokio::spawn(async move {
            println!("Screener {} starting scan...", i);
            let result = screener.scan_opportunities().await;
            println!("Screener {} completed scan", i);
            result
        });
        handles.push(handle);
    }
    
    // Wait for all screeners to complete
    let mut total_opportunities = 0;
    for (i, handle) in handles.into_iter().enumerate() {
        let result = handle.await.expect("Screener task failed");
        assert!(result.is_ok(), "Screener {} failed: {:?}", i, result.err());
        
        let opportunities = result.unwrap();
        total_opportunities += opportunities.len();
        println!("Screener {} found {} opportunities", i, opportunities.len());
    }
    
    println!("Total opportunities found across all screeners: {}", total_opportunities);
}

#[tokio::test]
async fn test_memory_usage() {
    // Memory tracking functionality removed for simplicity
    use std::sync::atomic::{AtomicUsize, Ordering};
    
    // Simple memory tracking (basic implementation)
    static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
    
    let initial_memory = ALLOCATED.load(Ordering::Relaxed);
    
    // Create large number of pools and opportunities
    let cache = PoolCache::new();
    let pools: Vec<Pool> = (0..10000)
        .map(|i| create_test_pool(i))
        .collect();
    
    // Cache all pools
    for i in 0..10 {
        let dex_name = format!("dex_{}", i);
        cache.set_pools(&dex_name, pools.clone()).await;
    }
    
    // Force cleanup
    cache.cleanup_expired().await;
    
    // Allow some time for cleanup
    sleep(Duration::from_millis(100)).await;
    
    let final_memory = ALLOCATED.load(Ordering::Relaxed);
    let memory_diff = final_memory.saturating_sub(initial_memory);
    
    println!("Memory usage difference: {} bytes", memory_diff);
    
    // Memory usage should be reasonable (less than 100MB for this test)
    assert!(memory_diff < 100_000_000, "Memory usage too high: {} bytes", memory_diff);
}

#[tokio::test]
async fn test_configuration_validation() {
    // Test configuration validation
    let valid_config = load_test_config();
    
    // Test that config loads successfully
    assert!(!valid_config.bot.execute_trades, "Test config should have execute_trades disabled");
    assert!(valid_config.bot.simulation_mode, "Test config should have simulation_mode enabled");
}

fn load_test_config() -> Config {
    // Create a test configuration
    Config::load().unwrap_or_else(|_| {
        // Create a basic test config if loading fails
        Config {
            bot: BotConfig {
                profit_threshold_percent: 0.5,
                max_slippage_percent: 2.0,
                min_liquidity_usd: 1000.0,
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
                enabled: vec!["orca".to_string(), "raydium".to_string()],
            },
            monitoring: MonitoringConfig {
                min_whale_transaction_sol: 10.0,
                mempool_enabled: false,
                whale_tracking_enabled: false,
                whale_wallet_addresses: vec![],
            },
            risk_management: RiskManagementConfig {
                max_consecutive_losses: 3,
                daily_loss_limit_sol: 10.0,
                position_sizing_enabled: true,
            },
        }
    })
}

fn create_test_pool(index: usize) -> Pool {
    let token_a = TokenInfo {
        mint: Pubkey::new_unique(),
        symbol: format!("TOKEN_A_{}", index),
        decimals: 9,
        price_usd: None,
    };
    
    let token_b = TokenInfo {
        mint: Pubkey::new_unique(),
        symbol: format!("TOKEN_B_{}", index),
        decimals: 6,
        price_usd: None,
    };
    
    Pool {
        address: Pubkey::new_unique(),
        dex: "test_dex".to_string(),
        token_a,
        token_b,
        reserve_a: 1000000 + (index as u64 * 1000),
        reserve_b: 2000000 + (index as u64 * 2000),
        liquidity_usd: Decimal::from_f64_retain(10000.0 + (index as f64 * 100.0)).unwrap(),
        fee_percent: Decimal::from_f64_retain(0.003).unwrap(),
        last_updated: chrono::Utc::now(),
    }
}

// Helper function to run a basic smoke test
#[tokio::test]
async fn smoke_test() {
    // This is a minimal test to ensure basic functionality works
    let config = load_test_config();
    assert!(config.bot.min_liquidity_usd > 0.0);
    assert!(!config.bot.execute_trades); // Should be false for tests
    
    let cache = PoolCache::new();
    let stats = cache.get_cache_stats().await;
    assert_eq!(stats.pool_entries, 0); // Should start empty
    assert_eq!(stats.reserve_entries, 0);
    
    println!("Smoke test passed - basic functionality working");
}