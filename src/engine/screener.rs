use crate::{
    config::Config,
    dex::{orca::OrcaClient, raydium::RaydiumClient, phoenix::PhoenixClient, DexClient},
    models::{ArbitrageOpportunity, ArbitrageRoute, Pool, TradeStep, ProfitabilityAnalysis},
    types::{ArbitrageType, TradeDirection},
    utils::math::{calculate_output_amount, calculate_price_impact, calculate_slippage},
};
use anyhow::{Context, Result};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

pub struct Screener {
    config: Config,
    orca_client: Arc<OrcaClient>,
    raydium_client: Arc<RaydiumClient>,
    phoenix_client: Arc<PhoenixClient>,
    all_pools: tokio::sync::RwLock<Vec<Pool>>,
}

impl Screener {
    pub fn new(
        config: Config,
        orca_client: Arc<OrcaClient>,
        raydium_client: Arc<RaydiumClient>,
        phoenix_client: Arc<PhoenixClient>,
    ) -> Result<Self> {
        Ok(Self {
            config,
            orca_client,
            raydium_client,
            phoenix_client,
            all_pools: tokio::sync::RwLock::new(Vec::new()),
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

        // Fetch pools from all enabled DEXs
        if self.config.dexs.enabled.contains(&"orca".to_string()) {
            match self.orca_client.fetch_pools().await {
                Ok(mut pools) => all_pools.append(&mut pools),
                Err(e) => warn!("Failed to fetch Orca pools: {}", e),
            }
        }

        if self.config.dexs.enabled.contains(&"raydium".to_string()) {
            match self.raydium_client.fetch_pools().await {
                Ok(mut pools) => all_pools.append(&mut pools),
                Err(e) => warn!("Failed to fetch Raydium pools: {}", e),
            }
        }

        if self.config.dexs.enabled.contains(&"phoenix".to_string()) {
            match self.phoenix_client.fetch_pools().await {
                Ok(mut pools) => all_pools.append(&mut pools),
                Err(e) => warn!("Failed to fetch Phoenix pools: {}", e),
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
        let price1 = self.calculate_pool_price(pool1, true)?; // token_a -> token_b
        let price2 = self.calculate_pool_price(pool2, false)?; // token_b -> token_a

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
        // This is a simplified implementation
        // A real triangular arbitrage would need to carefully match token pairs
        anyhow::bail!("Triangular arbitrage calculation not yet implemented");
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
