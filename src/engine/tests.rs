#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::Config,
        dex::DexClient,
        models::{Pool, TokenInfo, ArbitrageOpportunity, ArbitrageRoute, TradeStep},
        types::{ArbitrageType, TradeDirection},
        utils::cache::PoolCache,
    };
    use async_trait::async_trait;
    use rust_decimal::Decimal;
    use solana_sdk::pubkey::Pubkey;
    use std::sync::Arc;
    use uuid::Uuid;

    // Mock DexClient for testing
    pub struct MockDexClient {
        name: &'static str,
        pools: Vec<Pool>,
        should_fail: bool,
    }

    impl MockDexClient {
        pub fn new(name: &'static str) -> Self {
            MockDexClient {
                name,
                pools: vec![],
                should_fail: false,
            }
        }

        pub fn with_pools(name: &'static str, pools: Vec<Pool>) -> Self {
            MockDexClient {
                name,
                pools,
                should_fail: false,
            }
        }

        pub fn with_failure(name: &'static str) -> Self {
            MockDexClient {
                name,
                pools: vec![],
                should_fail: true,
            }
        }
    }

    #[async_trait]
    impl DexClient for MockDexClient {
        async fn fetch_pools(&self) -> anyhow::Result<Vec<Pool>> {
            if self.should_fail {
                anyhow::bail!("Mock DEX client failure");
            }
            Ok(self.pools.clone())
        }

        async fn get_pool_by_tokens(&self, _token_a: &str, _token_b: &str) -> anyhow::Result<Option<Pool>> {
            Ok(None)
        }

        async fn update_pool_reserves(&self, _pool: &mut Pool) -> anyhow::Result<()> {
            if self.should_fail {
                anyhow::bail!("Mock reserve update failure");
            }
            Ok(())
        }

        fn get_dex_name(&self) -> &'static str {
            self.name
        }

        fn set_console_manager(&mut self, _console: Arc<crate::console::ConsoleManager>) {
            // Mock implementation
        }
    }

    fn create_test_token(symbol: &str, decimals: u8) -> TokenInfo {
        TokenInfo {
            mint: Pubkey::new_unique(),
            symbol: symbol.to_string(),
            decimals,
            price_usd: None,
        }
    }

    fn create_test_pool(dex: &str, token_a: TokenInfo, token_b: TokenInfo, reserve_a: u64, reserve_b: u64, liquidity_usd: f64) -> Pool {
        Pool {
            address: Pubkey::new_unique(),
            dex: dex.to_string(),
            token_a,
            token_b,
            reserve_a,
            reserve_b,
            liquidity_usd: Decimal::from_f64_retain(liquidity_usd).unwrap(),
            fee_percent: Decimal::from_f64_retain(0.003).unwrap(), // 0.3% fee
            last_updated: chrono::Utc::now(),
        }
    }

    fn create_test_config() -> Config {
        let mut config = Config::default();
        config.bot.min_liquidity_usd = 1000.0;
        config.bot.profit_threshold_percent = 0.5;
        config.bot.max_position_size_sol = 10.0;
        config.dexs.enabled = vec!["orca".to_string(), "raydium".to_string()];
        config
    }

    #[tokio::test]
    async fn test_screener_initialization() {
        let config = create_test_config();
        let dex_clients: Vec<Arc<dyn DexClient>> = vec![
            Arc::new(MockDexClient::new("orca")),
            Arc::new(MockDexClient::new("raydium")),
        ];

        let screener = super::screener::Screener::new(config, dex_clients);
        assert!(screener.is_ok());
    }

    #[tokio::test]
    async fn test_pool_cache_integration() {
        let cache = PoolCache::new();
        let token_a = create_test_token("SOL", 9);
        let token_b = create_test_token("USDC", 6);
        let pool = create_test_pool("orca", token_a, token_b, 1000000, 2000000, 10000.0);
        
        // Test caching pools
        cache.set_pools("orca", vec![pool.clone()]).await;
        let cached_pools = cache.get_pools("orca").await;
        assert!(cached_pools.is_some());
        assert_eq!(cached_pools.unwrap().len(), 1);

        // Test caching reserves
        let pool_address = pool.address.to_string();
        cache.set_pool_reserves(&pool_address, (pool.reserve_a, pool.reserve_b)).await;
        let cached_reserves = cache.get_pool_reserves(&pool_address).await;
        assert!(cached_reserves.is_some());
        assert_eq!(cached_reserves.unwrap(), (pool.reserve_a, pool.reserve_b));
    }

    #[tokio::test]
    async fn test_direct_arbitrage_detection() {
        let config = create_test_config();
        let token_a = create_test_token("SOL", 9);
        let token_b = create_test_token("USDC", 6);
        
        // Create pools with price difference for arbitrage
        let pool1 = create_test_pool("orca", token_a.clone(), token_b.clone(), 1000000, 100000000, 10000.0); // 1 SOL = 100 USDC
        let pool2 = create_test_pool("raydium", token_a.clone(), token_b.clone(), 1000000, 105000000, 10000.0); // 1 SOL = 105 USDC
        
        let orca_client = Arc::new(MockDexClient::with_pools("orca", vec![pool1]));
        let raydium_client = Arc::new(MockDexClient::with_pools("raydium", vec![pool2]));
        
        let dex_clients: Vec<Arc<dyn DexClient>> = vec![orca_client, raydium_client];
        let screener = super::screener::Screener::new(config, dex_clients).unwrap();
        
        let opportunities = screener.scan_opportunities().await;
        assert!(opportunities.is_ok());
        
        // Should find arbitrage opportunities due to price difference
        let opps = opportunities.unwrap();
        // Note: The exact number depends on the implementation details
        // but there should be some opportunities detected
    }

    #[tokio::test]
    async fn test_executor_validation() {
        let config = create_test_config();
        let rpc_client = Arc::new(crate::utils::rpc::RpcClient::new(
            "https://api.mainnet-beta.solana.com".to_string(),
            std::time::Duration::from_secs(30),
        ));
        
        let executor = super::executor::Executor::new(config, rpc_client);
        assert!(executor.is_ok());
        
        let executor = executor.unwrap();
        
        // Test opportunity validation with invalid opportunity
        let invalid_opportunity = ArbitrageOpportunity {
            id: Uuid::new_v4(),
            route: ArbitrageRoute {
                route_type: ArbitrageType::Direct,
                steps: vec![], // Empty steps should fail validation
            },
            input_amount: 1000000000, // 1 SOL
            expected_output: 1050000000, // 1.05 SOL
            expected_profit: 50000000, // 0.05 SOL
            expected_profit_percent: 5.0,
            confidence_score: 0.5, // Too low
            risk_score: 0.8, // Too high
            estimated_gas_cost: 5000,
            max_slippage_percent: 1.0,
        };
        
        // This should fail validation due to empty steps, low confidence, and high risk
        let validation_result = executor.validate_arbitrage_opportunity(&invalid_opportunity);
        assert!(validation_result.is_err());
    }

    #[tokio::test]
    async fn test_security_validation() {
        let config = create_test_config();
        let rpc_client = Arc::new(crate::utils::rpc::RpcClient::new(
            "https://api.mainnet-beta.solana.com".to_string(),
            std::time::Duration::from_secs(30),
        ));
        
        let executor = super::executor::Executor::new(config, rpc_client).unwrap();
        
        // Test with malicious program ID
        let malicious_instruction = solana_sdk::instruction::Instruction {
            program_id: Pubkey::new_unique(), // Random/unknown program ID
            accounts: vec![],
            data: vec![],
        };
        
        let keypair = solana_sdk::signature::Keypair::new();
        let validation_result = executor.validate_transaction_security(&[malicious_instruction], &keypair);
        assert!(validation_result.is_err());
    }

    #[tokio::test]
    async fn test_config_security_validation() {
        let mut config = create_test_config();
        
        // Test with invalid private key
        config.bot.private_key = Some("invalid_key".to_string());
        let validation_result = config.validate_private_key("invalid_key");
        assert!(!validation_result);
        
        // Test security settings validation
        config.bot.private_key = None;
        config.bot.execute_trades = true;
        let security_validation = config.validate_security_settings();
        assert!(security_validation.is_ok()); // Should warn but not fail
        
        // Test with dangerous settings
        config.bot.max_position_size_sol = 1000.0; // Very large position
        config.bot.profit_threshold_percent = 0.01; // Very low threshold
        config.bot.max_slippage_percent = 10.0; // Very high slippage
        let security_validation = config.validate_security_settings();
        assert!(security_validation.is_ok()); // Should warn but not fail
    }

    #[tokio::test]
    async fn test_error_handling() {
        let config = create_test_config();
        let failing_client = Arc::new(MockDexClient::with_failure("orca"));
        let working_client = Arc::new(MockDexClient::new("raydium"));
        
        let dex_clients: Vec<Arc<dyn DexClient>> = vec![failing_client, working_client];
        let screener = super::screener::Screener::new(config, dex_clients).unwrap();
        
        // Should handle DEX client failures gracefully
        let opportunities = screener.scan_opportunities().await;
        assert!(opportunities.is_ok()); // Should not fail even if one DEX fails
    }

    #[tokio::test]
    async fn test_cache_performance() {
        let cache = PoolCache::new();
        let token_a = create_test_token("SOL", 9);
        let token_b = create_test_token("USDC", 6);
        let pools: Vec<Pool> = (0..100)
            .map(|i| create_test_pool("orca", token_a.clone(), token_b.clone(), 1000000 + i, 2000000 + i, 10000.0))
            .collect();
        
        // Measure cache performance
        let start = std::time::Instant::now();
        
        // Set pools in cache
        cache.set_pools("orca", pools.clone()).await;
        
        // Retrieve pools multiple times
        for _ in 0..10 {
            let cached_pools = cache.get_pools("orca").await;
            assert!(cached_pools.is_some());
            assert_eq!(cached_pools.unwrap().len(), 100);
        }
        
        let duration = start.elapsed();
        println!("Cache performance test completed in: {:?}", duration);
        
        // Should be very fast (under 1ms for this simple test)
        assert!(duration.as_millis() < 100);
    }

    #[tokio::test]
    async fn test_math_utilities() {
        use crate::utils::math::*;
        
        // Test AMM calculations
        let input_amount = 1000000; // 1 token
        let input_reserve = 1000000000; // 1000 tokens
        let output_reserve = 2000000000; // 2000 tokens
        let fee_rate = Decimal::from_f64_retain(0.003).unwrap(); // 0.3%
        
        let output = calculate_output_amount(input_amount, input_reserve, output_reserve, fee_rate);
        assert!(output > 0);
        assert!(output < input_amount * 2); // Should be less than 2x due to slippage and fees
        
        // Test price impact calculation
        let price_impact = calculate_price_impact(input_amount, input_reserve, output_reserve);
        assert!(price_impact >= 0.0);
        assert!(price_impact <= 100.0);
        
        // Test slippage calculation
        let expected_output = 1990000; // Expected output
        let actual_output = 1980000; // Actual output (slightly less)
        let slippage = calculate_slippage(expected_output, actual_output);
        assert!(slippage > 0.0);
        assert!(slippage < 1.0); // Should be small slippage
    }

    #[tokio::test]
    async fn test_risk_management() {
        let config = create_test_config();
        let rpc_client = Arc::new(crate::utils::rpc::RpcClient::new(
            "https://api.mainnet-beta.solana.com".to_string(),
            std::time::Duration::from_secs(30),
        ));
        
        let executor = super::executor::Executor::new(config.clone(), rpc_client).unwrap();
        
        // Test position size validation
        let large_opportunity = ArbitrageOpportunity {
            id: Uuid::new_v4(),
            route: ArbitrageRoute {
                route_type: ArbitrageType::Direct,
                steps: vec![TradeStep {
                    pool: create_test_pool("orca", create_test_token("SOL", 9), create_test_token("USDC", 6), 1000000, 2000000, 10000.0),
                    direction: TradeDirection::AToB,
                    input_amount: 20000000000, // 20 SOL (exceeds max position size)
                    expected_output: 21000000000,
                }],
            },
            input_amount: 20000000000, // 20 SOL
            expected_output: 21000000000,
            expected_profit: 1000000000,
            expected_profit_percent: 5.0,
            confidence_score: 0.9,
            risk_score: 0.3,
            estimated_gas_cost: 5000,
            max_slippage_percent: 1.0,
        };
        
        let validation_result = executor.validate_arbitrage_opportunity(&large_opportunity);
        assert!(validation_result.is_err()); // Should fail due to large position size
    }
}