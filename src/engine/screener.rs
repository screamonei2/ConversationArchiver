use crate::{
    config::Config,
    dex::{DexClient},
    models::{ArbitrageOpportunity, ArbitrageRoute, Pool, TradeStep},
    types::{ArbitrageType, TradeDirection},
    utils::{
        cache::PoolCache,
        math::{calculate_output_amount, calculate_price_impact, calculate_slippage},
    },
};
use anyhow::Result;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::dex::DexClient;
    use std::sync::Arc;
    use async_trait::async_trait;

    // Mock DexClient for testing
    pub struct MockDexClient {
        name: &'static str,
    }

    impl MockDexClient {
        pub fn new(name: &'static str) -> Self {
            MockDexClient { name }
        }
    }

    #[async_trait]
    impl DexClient for MockDexClient {
        async fn fetch_pools(&self) -> Result<Vec<Pool>> {
            Ok(vec![])
        }
        async fn get_pool_by_tokens(&self, _token_a: &str, _token_b: &str) -> Result<Option<Pool>> {
            Ok(None)
        }
        async fn update_pool_reserves(&self, _pool: &mut Pool) -> anyhow::Result<()> {
            Ok(())
        }
        fn get_dex_name(&self) -> &'static str {
            self.name
        }
        fn set_console_manager(&mut self, _console: Arc<crate::console::ConsoleManager>) {
            // Mock implementation, does nothing
        }
    }

    #[tokio::test]
    async fn test_screener_new() {
        let config = Config::load().unwrap();

        let orca_client = Arc::new(MockDexClient::new("orca")) as Arc<dyn DexClient>;
        let raydium_client = Arc::new(MockDexClient::new("raydium")) as Arc<dyn DexClient>;
        let phoenix_client = Arc::new(MockDexClient::new("phoenix")) as Arc<dyn DexClient>;

        let dex_clients: Vec<Arc<dyn DexClient>> = vec![
            orca_client,
            raydium_client,
            phoenix_client,
        ];

        let screener = Screener::new(
            config,
            dex_clients,
        );

        assert!(screener.is_ok());
    }
}

pub struct Screener {
    config: Config,
    dex_clients: Vec<Arc<dyn DexClient>>,
    all_pools: tokio::sync::RwLock<Vec<Pool>>,
    cache: PoolCache,
}

impl Screener {
    pub fn new(
        config: Config,
        dex_clients: Vec<Arc<dyn DexClient>>,
    ) -> Result<Self> {
        let cache = PoolCache::new();
        
        // Start background cache cleanup task
        cache.start_cleanup_task();
        
        Ok(Self {
            config,
            dex_clients,
            all_pools: tokio::sync::RwLock::new(Vec::new()),
            cache,
        })
    }

    pub async fn scan_opportunities(&self) -> Result<Vec<ArbitrageOpportunity>> {
        // Update pool data from all DEXs
        self.update_all_pools().await?;
        
        let pools = self.all_pools.read().await;
        let mut opportunities = Vec::new();

        info!("Scanning {} pools for arbitrage opportunities", pools.len());

        // Scan for direct arbitrage opportunities
        opportunities.extend(self.scan_direct_arbitrage(&pools).await?);
        
        // Scan for triangular arbitrage opportunities
        opportunities.extend(self.scan_triangular_arbitrage(&pools).await?);
        
        // Scan for cross-DEX arbitrage opportunities
        opportunities.extend(self.scan_cross_dex_arbitrage(&pools).await?);

        // Filter and sort opportunities
        let filtered_opportunities = self.filter_opportunities(opportunities).await?;
        
        info!("Found {} profitable opportunities", filtered_opportunities.len());
        Ok(filtered_opportunities)
    }

    async fn update_all_pools(&self) -> Result<()> {
        let mut all_pools = Vec::new();

        // Fetch pools from all enabled DEXs with caching
        for client in &self.dex_clients {
            let dex_name = client.get_dex_name();
            if self.config.dexs.enabled.contains(&dex_name.to_string()) {
                // Try to get from cache first
                if let Some(cached_pools) = self.cache.get_pools(dex_name).await {
                    debug!("Using cached pools for {}", dex_name);
                    all_pools.extend(cached_pools);
                } else {
                    // Fetch from DEX and cache the result
                    match client.fetch_pools().await {
                        Ok(pools) => {
                            debug!("Fetched {} pools from {}", pools.len(), dex_name);
                            self.cache.set_pools(dex_name, pools.clone()).await;
                            all_pools.extend(pools);
                        },
                        Err(e) => {
                            warn!("Failed to fetch {} pools: {}", dex_name, e);
                            // Invalidate cache on error
                            self.cache.invalidate_dex(dex_name).await;
                        },
                    }
                }
            }
        }

        // Update pool reserves with caching
        for pool in &mut all_pools {
            let pool_address = pool.address.to_string();
            
            // Try to get reserves from cache first
            if let Some((reserve_a, reserve_b)) = self.cache.get_pool_reserves(&pool_address).await {
                pool.reserve_a = reserve_a;
                pool.reserve_b = reserve_b;
            } else {
                // Fetch fresh reserves and cache them
                for client in &self.dex_clients {
                    if client.get_dex_name() == pool.dex {
                        if let Err(e) = client.update_pool_reserves(pool).await {
                            warn!("Failed to update reserves for pool {}: {}", pool_address, e);
                            self.cache.invalidate_pool(&pool_address).await;
                        } else {
                            // Cache the updated reserves
                            self.cache.set_pool_reserves(&pool_address, (pool.reserve_a, pool.reserve_b)).await;
                        }
                        break;
                    }
                }
            }
        }

        // Filter pools by minimum liquidity
        let filtered_pools: Vec<Pool> = all_pools
            .into_iter()
            .filter(|pool| pool.liquidity_usd >= Decimal::from_f64_retain(self.config.bot.min_liquidity_usd).unwrap())
            .collect();

        let mut pools_lock = self.all_pools.write().await;
        *pools_lock = filtered_pools;

        debug!("Updated pool data: {} pools available", pools_lock.len());
        
        // Log cache statistics
        let cache_stats = self.cache.get_cache_stats().await;
        debug!("Cache stats - Pool entries: {}, Reserve entries: {}, Hit rate: {:.2}%", 
               cache_stats.pool_entries, 
               cache_stats.reserve_entries,
               cache_stats.hit_rate() * 100.0);
        
        Ok(())
    }

    async fn scan_direct_arbitrage(&self, pools: &[Pool]) -> Result<Vec<ArbitrageOpportunity>> {
        let mut opportunities = Vec::new();

        // Group pools by token pairs
        let mut token_pair_pools: std::collections::HashMap<(String, String), Vec<&Pool>> = 
            std::collections::HashMap::new();

        for pool in pools {
            let token_a = pool.token_a.mint.to_string();
            let token_b = pool.token_b.mint.to_string();
            let pair = if token_a < token_b {
                (token_a, token_b)
            } else {
                (token_b, token_a)
            };
            
            token_pair_pools.entry(pair).or_insert_with(Vec::new).push(pool);
        }

        // Look for arbitrage opportunities between different pools for the same pair
        for (_pair, pair_pools) in token_pair_pools {
            if pair_pools.len() < 2 {
                continue;
            }

            for i in 0..pair_pools.len() {
                for j in i + 1..pair_pools.len() {
                    let pool1 = pair_pools[i];
                    let pool2 = pair_pools[j];

                    // Skip if same DEX
                    if pool1.dex == pool2.dex {
                        continue;
                    }

                    // Calculate potential arbitrage
                    if let Ok(opportunity) = self.calculate_direct_arbitrage(pool1, pool2).await {
                        opportunities.push(opportunity);
                    }
                }
            }
        }

        debug!("Found {} direct arbitrage opportunities", opportunities.len());
        Ok(opportunities)
    }

    async fn scan_triangular_arbitrage(&self, pools: &[Pool]) -> Result<Vec<ArbitrageOpportunity>> {
        let mut opportunities = Vec::new();

        // This is computationally expensive, so we limit the search
        const MAX_TRIANGULAR_COMBINATIONS: usize = 1000;
        let mut combinations_checked = 0;

        for pool1 in pools.iter() {
            if combinations_checked >= MAX_TRIANGULAR_COMBINATIONS {
                break;
            }

            for pool2 in pools.iter() {
                if combinations_checked >= MAX_TRIANGULAR_COMBINATIONS {
                    break;
                }

                if pool1.address == pool2.address {
                    continue;
                }

                // Check if pools can form a triangle
                let common_token = self.find_common_token(pool1, pool2);
                if common_token.is_none() {
                    continue;
                }

                for pool3 in pools.iter() {
                    combinations_checked += 1;
                    if combinations_checked >= MAX_TRIANGULAR_COMBINATIONS {
                        break;
                    }

                    if pool3.address == pool1.address || pool3.address == pool2.address {
                        continue;
                    }

                    if let Ok(opportunity) = self.calculate_triangular_arbitrage(pool1, pool2, pool3).await {
                        opportunities.push(opportunity);
                    }
                }
            }
        }

        debug!("Found {} triangular arbitrage opportunities", opportunities.len());
        Ok(opportunities)
    }

    async fn scan_cross_dex_arbitrage(&self, pools: &[Pool]) -> Result<Vec<ArbitrageOpportunity>> {
        let mut opportunities = Vec::new();

        // Group pools by token pairs across different DEXs
        let mut cross_dex_pairs: std::collections::HashMap<(String, String), Vec<&Pool>> = 
            std::collections::HashMap::new();

        for pool in pools {
            let token_a = pool.token_a.mint.to_string();
            let token_b = pool.token_b.mint.to_string();
            let pair = if token_a < token_b {
                (token_a, token_b)
            } else {
                (token_b, token_a)
            };
            
            cross_dex_pairs.entry(pair).or_insert_with(Vec::new).push(pool);
        }

        // Look for cross-DEX arbitrage opportunities
        for (_pair, pair_pools) in cross_dex_pairs {
            let mut dex_pools: std::collections::HashMap<&str, &Pool> = std::collections::HashMap::new();
            
            // Get one pool per DEX for this pair
            for pool in pair_pools {
                if !dex_pools.contains_key(pool.dex.as_str()) {
                    dex_pools.insert(&pool.dex, pool);
                }
            }

            // If we have pools from multiple DEXs, check for arbitrage
            if dex_pools.len() >= 2 {
                let pools_vec: Vec<&Pool> = dex_pools.values().cloned().collect();
                for i in 0..pools_vec.len() {
                    for j in i + 1..pools_vec.len() {
                        if let Ok(opportunity) = self.calculate_cross_dex_arbitrage(pools_vec[i], pools_vec[j]).await {
                            opportunities.push(opportunity);
                        }
                    }
                }
            }
        }

        debug!("Found {} cross-DEX arbitrage opportunities", opportunities.len());
        Ok(opportunities)
    }

    async fn calculate_direct_arbitrage(&self, pool1: &Pool, pool2: &Pool) -> Result<ArbitrageOpportunity> {
        let input_amount = (self.config.bot.max_position_size_sol * 1_000_000_000.0) as u64; // Convert SOL to lamports
        
        // Calculate price difference between pools
        let _price1 = self.calculate_pool_price(pool1, true)?; // token_a -> token_b
        let _price2 = self.calculate_pool_price(pool2, false)?; // token_b -> token_a

        let expected_output1 = calculate_output_amount(
            input_amount,
            pool1.reserve_a,
            pool1.reserve_b,
            pool1.fee_percent,
        )?;

        let expected_output2 = calculate_output_amount(
            expected_output1,
            pool2.reserve_b,
            pool2.reserve_a,
            pool2.fee_percent,
        )?;

        if expected_output2 <= input_amount {
            anyhow::bail!("Not profitable");
        }

        let profit = expected_output2 - input_amount;
        let profit_percent = (profit as f64 / input_amount as f64) * 100.0;

        let route = ArbitrageRoute {
            route_type: ArbitrageType::Direct,
            from_token: pool1.token_a.mint.to_string(),
            to_token: pool1.token_a.mint.to_string(),
            intermediate_token: Some(pool1.token_b.mint.to_string()),
            steps: vec![
                TradeStep {
                    pool: pool1.clone(),
                    direction: TradeDirection::Buy,
                    input_amount,
                    expected_output: expected_output1,
                    price_impact: calculate_price_impact(input_amount, pool1.reserve_a, pool1.reserve_b)?,
                    slippage: calculate_slippage(expected_output1, pool1.reserve_b, self.config.bot.max_slippage_percent)?,
                },
                TradeStep {
                    pool: pool2.clone(),
                    direction: TradeDirection::Sell,
                    input_amount: expected_output1,
                    expected_output: expected_output2,
                    price_impact: calculate_price_impact(expected_output1, pool2.reserve_b, pool2.reserve_a)?,
                    slippage: calculate_slippage(expected_output2, pool2.reserve_a, self.config.bot.max_slippage_percent)?,
                },
            ],
            total_fee_percent: pool1.fee_percent + pool2.fee_percent,
        };

        let opportunity = ArbitrageOpportunity {
            id: Uuid::new_v4().to_string(),
            route,
            input_amount,
            expected_output: expected_output2,
            expected_profit: profit,
            expected_profit_percent: profit_percent,
            confidence_score: self.calculate_confidence_score(&[pool1, pool2]),
            risk_score: self.calculate_risk_score(&[pool1, pool2]),
            timestamp: chrono::Utc::now(),
            expiry: chrono::Utc::now() + chrono::Duration::seconds(30), // 30-second expiry
        };

        Ok(opportunity)
    }

    async fn calculate_triangular_arbitrage(&self, pool1: &Pool, pool2: &Pool, pool3: &Pool) -> Result<ArbitrageOpportunity> {
        // Find the triangular path: A -> B -> C -> A
        let path = self.find_triangular_path(pool1, pool2, pool3)?;
        if path.is_empty() {
            anyhow::bail!("No valid triangular path found");
        }

        let input_amount = (self.config.bot.max_position_size_sol * 1_000_000_000.0) as u64;
        let mut current_amount = input_amount;
        let mut steps = Vec::new();
        let mut total_fees = Decimal::ZERO;

        // Execute the triangular path
        for (i, (pool, direction)) in path.iter().enumerate() {
            let (reserve_in, reserve_out) = if *direction {
                (pool.reserve_a, pool.reserve_b)
            } else {
                (pool.reserve_b, pool.reserve_a)
            };

            let output_amount = calculate_output_amount(
                current_amount,
                reserve_in,
                reserve_out,
                pool.fee_percent,
            )?;

            steps.push(TradeStep {
                pool: (*pool).clone(),
                direction: if *direction { TradeDirection::Buy } else { TradeDirection::Sell },
                input_amount: current_amount,
                expected_output: output_amount,
                price_impact: calculate_price_impact(current_amount, reserve_in, reserve_out)?,
                slippage: calculate_slippage(output_amount, reserve_out, self.config.bot.max_slippage_percent)?,
            });

            current_amount = output_amount;
            total_fees += pool.fee_percent;
        }

        // Check if profitable
        if current_amount <= input_amount {
            anyhow::bail!("Triangular arbitrage not profitable");
        }

        let profit = current_amount - input_amount;
        let profit_percent = (profit as f64 / input_amount as f64) * 100.0;

        let route = ArbitrageRoute {
            route_type: ArbitrageType::Triangular,
            from_token: steps[0].pool.token_a.mint.to_string(),
            to_token: steps[0].pool.token_a.mint.to_string(),
            intermediate_token: Some(steps[1].pool.token_a.mint.to_string()),
            steps,
            total_fee_percent: total_fees,
        };

        let opportunity = ArbitrageOpportunity {
            id: Uuid::new_v4().to_string(),
            route,
            input_amount,
            expected_output: current_amount,
            expected_profit: profit,
            expected_profit_percent: profit_percent,
            confidence_score: self.calculate_confidence_score(&[pool1, pool2, pool3]),
            risk_score: self.calculate_risk_score(&[pool1, pool2, pool3]),
            timestamp: chrono::Utc::now(),
            expiry: chrono::Utc::now() + chrono::Duration::seconds(30),
        };

        Ok(opportunity)
    }

    fn find_triangular_path<'a>(&self, pool1: &'a Pool, pool2: &'a Pool, pool3: &'a Pool) -> Result<Vec<(&'a Pool, bool)>> {
        // Try to find a valid triangular path through the three pools
        // This is a simplified implementation that checks common patterns
        
        let pools = [pool1, pool2, pool3];
        let mut tokens = std::collections::HashSet::new();
        
        // Collect all unique tokens
        for pool in &pools {
            tokens.insert(pool.token_a.mint.to_string());
            tokens.insert(pool.token_b.mint.to_string());
        }
        
        // For triangular arbitrage, we need exactly 3 tokens
        if tokens.len() != 3 {
            anyhow::bail!("Invalid token configuration for triangular arbitrage");
        }
        
        let token_vec: Vec<String> = tokens.into_iter().collect();
        let start_token = &token_vec[0];
        
        // Try to find a path that starts and ends with the same token
        if let Some(path) = self.build_triangular_path(&pools, start_token, start_token, Vec::new()) {
            if path.len() == 3 {
                return Ok(path);
            }
        }
        
        anyhow::bail!("No valid triangular path found")
    }
    
    fn build_triangular_path<'a>(&self, pools: &[&'a Pool], current_token: &str, target_token: &str, mut path: Vec<(&'a Pool, bool)>) -> Option<Vec<(&'a Pool, bool)>> {
        if path.len() == 3 {
            return if current_token == target_token { Some(path) } else { None };
        }
        
        for pool in pools {
            // Skip if pool already used
            if path.iter().any(|(p, _)| p.address == pool.address) {
                continue;
            }
            
            // Check if current token is in this pool
            let (next_token, direction) = if pool.token_a.mint.to_string() == current_token {
                (pool.token_b.mint.to_string(), true)
            } else if pool.token_b.mint.to_string() == current_token {
                (pool.token_a.mint.to_string(), false)
            } else {
                continue;
            };
            
            path.push((pool, direction));
            if let Some(result) = self.build_triangular_path(pools, &next_token, target_token, path.clone()) {
                return Some(result);
            }
            path.pop();
        }
        
        None
    }

    async fn calculate_cross_dex_arbitrage(&self, pool1: &Pool, pool2: &Pool) -> Result<ArbitrageOpportunity> {
        // Similar to direct arbitrage but across different DEXs
        self.calculate_direct_arbitrage(pool1, pool2).await
    }

    fn find_common_token(&self, pool1: &Pool, pool2: &Pool) -> Option<String> {
        let pool1_tokens = [pool1.token_a.mint.to_string(), pool1.token_b.mint.to_string()];
        let pool2_tokens = [pool2.token_a.mint.to_string(), pool2.token_b.mint.to_string()];

        for token1 in &pool1_tokens {
            for token2 in &pool2_tokens {
                if token1 == token2 {
                    return Some(token1.clone());
                }
            }
        }
        None
    }

    fn calculate_pool_price(&self, pool: &Pool, direction: bool) -> Result<Decimal> {
        if direction {
            // token_a -> token_b
            Ok(Decimal::from(pool.reserve_b) / Decimal::from(pool.reserve_a))
        } else {
            // token_b -> token_a
            Ok(Decimal::from(pool.reserve_a) / Decimal::from(pool.reserve_b))
        }
    }

    fn calculate_confidence_score(&self, pools: &[&Pool]) -> f64 {
        // Calculate confidence based on liquidity, age of data, etc.
        let total_liquidity: f64 = pools.iter()
            .map(|p| p.liquidity_usd.to_f64().unwrap_or(0.0))
            .sum();
        
        // Higher liquidity = higher confidence
        (total_liquidity / 100000.0).min(1.0)
    }

    fn calculate_risk_score(&self, pools: &[&Pool]) -> f64 {
        // Calculate risk based on volatility, slippage, etc.
        let avg_liquidity: f64 = pools.iter()
            .map(|p| p.liquidity_usd.to_f64().unwrap_or(0.0))
            .sum::<f64>() / pools.len() as f64;
        
        // Lower liquidity = higher risk
        if avg_liquidity < 10000.0 {
            0.8
        } else if avg_liquidity < 50000.0 {
            0.5
        } else {
            0.2
        }
    }

    async fn filter_opportunities(&self, mut opportunities: Vec<ArbitrageOpportunity>) -> Result<Vec<ArbitrageOpportunity>> {
        // Filter by profitability threshold
        opportunities.retain(|opp| opp.expected_profit_percent >= self.config.bot.profit_threshold_percent);
        
        // Filter by confidence score
        opportunities.retain(|opp| opp.confidence_score >= 0.3);
        
        // Filter by risk score
        opportunities.retain(|opp| opp.risk_score <= 0.7);
        
        // Sort by expected profit percentage (descending)
        opportunities.sort_by(|a, b| b.expected_profit_percent.partial_cmp(&a.expected_profit_percent).unwrap());
        
        // Limit to top opportunities
        opportunities.truncate(10);
        
        Ok(opportunities)
    }
}
