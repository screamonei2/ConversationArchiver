use anyhow::Result;
use solana_arbitrage_bot::{
    config::Config,
    console::{ConsoleManager, OpportunityDisplay},
    dex::{
        orca::OrcaClient,
        raydium::RaydiumClient,
        phoenix::PhoenixClient,
        meteora::MeteoraDex,
        saber::SaberDex,
        serum::SerumDex,
        lifinity::LifinityDex,
        pumpfun::PumpFunDex,
        DexClient,
    },
    dex_config::DexConfigs,
    engine::{executor::Executor, screener::Screener},
    monitor::{mempool::MempoolMonitor, whales::WhaleMonitor},
    tests,
    utils::rpc::RpcClient,
};
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{info, error, warn};
use chrono::Utc;
use uuid;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting Solana Arbitrage Bot");

    // Load configuration
    let config = Config::load()?;
    info!("Configuration loaded successfully");

    // Initialize console manager early
    let console_manager = Arc::new(ConsoleManager::new());
    console_manager.update_status("Application", "Started");

    // Initialize RPC client
    let rpc_client = Arc::new(RpcClient::new(&config)?);
    info!("RPC client initialized");

    // Initialize DEX clients dynamically from config
    info!("Initializing DEX clients...");
    
    let mut dex_clients: Vec<Arc<dyn DexClient>> = Vec::new();
    let dex_configs = DexConfigs::new();
    
    for dex_config in dex_configs.get_enabled() {
        info!("Initializing {} DEX...", dex_config.name);
        
        let client: Arc<dyn DexClient> = match dex_config.name.as_str() {
            "Orca" => Arc::new(OrcaClient::new(rpc_client.clone(), console_manager.clone())?),
            "Raydium" => Arc::new(RaydiumClient::new(rpc_client.clone(), console_manager.clone())?),
            "Phoenix" => Arc::new(PhoenixClient::new(rpc_client.clone(), console_manager.clone())?),
            "Meteora" => Arc::new(MeteoraDex::new(rpc_client.clone(), console_manager.clone())?),
            "Meteora DAMM" => Arc::new(MeteoraDex::new(rpc_client.clone(), console_manager.clone())?),
            "Saber" => Arc::new(SaberDex::new(rpc_client.clone(), console_manager.clone())?),
            "Serum" => Arc::new(SerumDex::new(rpc_client.clone(), console_manager.clone())?),
            "Lifinity" => Arc::new(LifinityDex::new(rpc_client.clone(), console_manager.clone())?),
            "Pump.fun" => Arc::new(PumpFunDex::new(rpc_client.clone(), console_manager.clone())?),
            _ => {
                warn!("Unknown DEX: {}, skipping...", dex_config.name);
                continue;
            }
        };
        
        dex_clients.push(client);
    }
    info!("DEX clients initialized");

    // Initialize core components

    let screener = Arc::new(Screener::new(
        config.clone(),
        dex_clients.clone(),
    )?);

    let executor = Arc::new(Executor::new(
        config.clone(),
        rpc_client.clone(),
    )?);

    // Initialize monitoring components
    let mempool_monitor = Arc::new(MempoolMonitor::new(
        config.clone(),
        rpc_client.clone(),
        console_manager.clone(),
    )?);

    let whale_monitor = Arc::new(WhaleMonitor::new(
        config.clone(),
        rpc_client.clone(),
        console_manager.clone(),
    )?);

    info!("All components initialized successfully");

    // Test DEX connections at startup using the actual DEX clients and cache pools
    info!("Testing DEX connections and caching pools...");
    
    let connection_tester = tests::DexConnectionTester::new(config.clone(), rpc_client.clone(), console_manager.clone());
    let (test_results, cached_pools) = connection_tester.test_and_cache_dex_clients(&dex_clients).await?;
    
    info!("Cached {} pools from {} DEX clients", cached_pools.len(), dex_clients.len());
    
    // Log connection test results with actual DEX names
    let enabled_dexs = dex_configs.get_enabled();
    for (i, result) in test_results.iter().enumerate() {
        let dex_name = if i < enabled_dexs.len() {
            enabled_dexs.get(i).map(|config| config.name.as_str()).unwrap_or("Unknown")
        } else {
            "Unknown"
        };
        
        match &result.error_message {
            Some(error) => {
                error!("{} connection test failed: {}", dex_name, error);
                console_manager.update_service_status(
                    dex_name,
                    "Failed",
                    "Connection error",
                    Some(error.to_string()),
                );
            }
            None => {
                let pool_count = result.pools_count.unwrap_or(0);
                info!("{} connection test passed: {} pools fetched in {}ms", 
                      dex_name, pool_count, result.response_time_ms);
                console_manager.update_service_status(
                    dex_name,
                    "Connected",
                    "Healthy",
                    Some(format!("{} pools cached", pool_count)),
                );
            }
        }
    }

    // Start monitoring tasks
    let mempool_handle = {
        let monitor = mempool_monitor.clone();
        tokio::spawn(async move {
            if let Err(e) = monitor.start().await {
                error!("Mempool monitor error: {}", e);
            }
        })
    };

    let whale_handle = {
        let monitor = whale_monitor.clone();
        tokio::spawn(async move {
            if let Err(e) = monitor.start().await {
                error!("Whale monitor error: {}", e);
            }
        })
    };

    // Main arbitrage loop
    let mut interval = interval(Duration::from_secs(config.bot.cooldown_seconds));
    let mut consecutive_failures = 0;
    const MAX_CONSECUTIVE_FAILURES: u32 = 10;

    info!("Starting main arbitrage loop");
    
    // Initialize console with service statuses
    console_manager.update_service_status("Application", "Running", "Healthy", None);

    loop {
        interval.tick().await;

        match run_arbitrage_cycle(&screener, &executor, &config, &console_manager).await {
            Ok(()) => {
                consecutive_failures = 0;
                info!("Arbitrage cycle completed successfully.");
                console_manager.update_status("ArbitrageCycle", "Completed");
            }
            Err(e) => {
                consecutive_failures += 1;
                error!("Arbitrage cycle failed: {} (consecutive failures: {})", e, consecutive_failures);
                console_manager.update_status("ArbitrageCycle", &format!("Failed: {}", e));
                
                if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                    error!("Too many consecutive failures, shutting down");
                    break;
                }
                
                // Exponential backoff on failures
                let backoff_duration = Duration::from_secs(2_u64.pow(consecutive_failures.min(6)));
                warn!("Backing off for {:?} due to failures", backoff_duration);
                console_manager.update_status("ArbitrageCycle", &format!("Backing off for {:?} (failures: {})", backoff_duration, consecutive_failures));
                tokio::time::sleep(backoff_duration).await;
            }
        }
    }

    // Cleanup
    mempool_handle.abort();
    whale_handle.abort();
    
    info!("Solana Arbitrage Bot shutting down");
    Ok(())
}

async fn run_arbitrage_cycle(
    screener: &Arc<Screener>,
    executor: &Arc<Executor>,
    config: &Config,
    console: &Arc<ConsoleManager>,
) -> Result<()> {
    // Screen for arbitrage opportunities
    console.update_status("ArbitrageCycle", "Scanning opportunities");
    let opportunities = screener.scan_opportunities().await?;
    
    if opportunities.is_empty() {
        info!("No profitable opportunities found");
        console.update_status("ArbitrageCycle", "No opportunities found");
        return Ok(());
    }

    info!("Found {} potential opportunities", opportunities.len());
    console.update_status_with_info(
        "ArbitrageCycle", 
        "Opportunities found", 
        &format!("{} opportunities", opportunities.len())
    );

    // Display opportunities in console
    for opportunity in &opportunities {
        let opportunity_display = OpportunityDisplay {
            id: format!("arb_{}", uuid::Uuid::new_v4().to_string()[..8].to_string()),
            dex_pair: format!("{} -> {}", 
                opportunity.route.steps[0].pool.dex,
                opportunity.route.steps.last().unwrap().pool.dex
            ),
            token_pair: format!("{}/{}", 
                opportunity.route.from_token,
                opportunity.route.to_token
            ),
            profit_percent: opportunity.expected_profit_percent,
            profit_usd: opportunity.expected_profit as f64 / 1_000_000_000.0, // Convert lamports to SOL
            timestamp: Utc::now(),
        };
        console.add_opportunity(opportunity_display);
    }

    // Execute profitable opportunities
    let mut executed_count = 0;
    for opportunity in opportunities {
        if opportunity.expected_profit_percent >= config.bot.profit_threshold_percent {
            info!(
                "Executing arbitrage: {} -> {} (expected profit: {:.2}%)",
                opportunity.route.from_token,
                opportunity.route.to_token,
                opportunity.expected_profit_percent
            );

            console.update_status_with_info(
                "ArbitrageCycle", 
                "Executing trade", 
                &format!("{:.2}% profit expected", opportunity.expected_profit_percent)
            );

            match executor.execute_arbitrage(&opportunity).await {
                Ok(signature) => {
                    info!("Trade executed successfully: {}", signature);
                    executed_count += 1;
                }
                Err(e) => {
                    error!("Trade execution failed: {}", e);
                }
            }

            // Cooldown between trades
            tokio::time::sleep(Duration::from_secs(config.bot.cooldown_seconds)).await;
        }
    }

    if executed_count > 0 {
        console.update_status_with_info(
            "ArbitrageCycle", 
            "Completed", 
            &format!("{} trades executed", executed_count)
        );
    } else {
        console.update_status("ArbitrageCycle", "No profitable trades");
    }

    Ok(())
}
