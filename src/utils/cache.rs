use crate::models::Pool;
use anyhow::Result;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use tracing::{debug, warn};

#[derive(Clone)]
struct CacheEntry<T> {
    data: T,
    expires_at: Instant,
}

impl<T> CacheEntry<T> {
    fn new(data: T, ttl: Duration) -> Self {
        Self {
            data,
            expires_at: Instant::now() + ttl,
        }
    }

    fn is_expired(&self) -> bool {
        Instant::now() > self.expires_at
    }
}

pub struct PoolCache {
    pools: Arc<RwLock<HashMap<String, CacheEntry<Vec<Pool>>>>>,
    pool_reserves: Arc<RwLock<HashMap<String, CacheEntry<(u64, u64)>>>>,
    default_ttl: Duration,
    reserves_ttl: Duration,
}

impl PoolCache {
    pub fn new() -> Self {
        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
            pool_reserves: Arc::new(RwLock::new(HashMap::new())),
            default_ttl: Duration::from_secs(300), // 5 minutes for pool list
            reserves_ttl: Duration::from_secs(30), // 30 seconds for reserves
        }
    }

    pub fn with_ttl(pool_ttl: Duration, reserves_ttl: Duration) -> Self {
        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
            pool_reserves: Arc::new(RwLock::new(HashMap::new())),
            default_ttl: pool_ttl,
            reserves_ttl,
        }
    }

    pub async fn get_pools(&self, dex_name: &str) -> Option<Vec<Pool>> {
        let pools = self.pools.read().await;
        if let Some(entry) = pools.get(dex_name) {
            if !entry.is_expired() {
                debug!("Cache hit for {} pools", dex_name);
                return Some(entry.data.clone());
            } else {
                debug!("Cache expired for {} pools", dex_name);
            }
        }
        None
    }

    pub async fn set_pools(&self, dex_name: &str, pools: Vec<Pool>) {
        let mut cache = self.pools.write().await;
        cache.insert(
            dex_name.to_string(),
            CacheEntry::new(pools, self.default_ttl),
        );
        debug!("Cached {} pools for {}", cache.get(dex_name).unwrap().data.len(), dex_name);
    }

    pub async fn get_pool_reserves(&self, pool_address: &str) -> Option<(u64, u64)> {
        let reserves = self.pool_reserves.read().await;
        if let Some(entry) = reserves.get(pool_address) {
            if !entry.is_expired() {
                debug!("Cache hit for pool reserves: {}", pool_address);
                return Some(entry.data);
            } else {
                debug!("Cache expired for pool reserves: {}", pool_address);
            }
        }
        None
    }

    pub async fn set_pool_reserves(&self, pool_address: &str, reserves: (u64, u64)) {
        let mut cache = self.pool_reserves.write().await;
        cache.insert(
            pool_address.to_string(),
            CacheEntry::new(reserves, self.reserves_ttl),
        );
        debug!("Cached reserves for pool: {}", pool_address);
    }

    pub async fn invalidate_pool(&self, pool_address: &str) {
        let mut reserves = self.pool_reserves.write().await;
        reserves.remove(pool_address);
        debug!("Invalidated cache for pool: {}", pool_address);
    }

    pub async fn invalidate_dex(&self, dex_name: &str) {
        let mut pools = self.pools.write().await;
        pools.remove(dex_name);
        debug!("Invalidated cache for DEX: {}", dex_name);
    }

    pub async fn cleanup_expired(&self) {
        let mut pools_removed = 0;
        let mut reserves_removed = 0;

        // Clean up expired pool lists
        {
            let mut pools = self.pools.write().await;
            pools.retain(|dex_name, entry| {
                if entry.is_expired() {
                    debug!("Removing expired pool cache for: {}", dex_name);
                    pools_removed += 1;
                    false
                } else {
                    true
                }
            });
        }

        // Clean up expired pool reserves
        {
            let mut reserves = self.pool_reserves.write().await;
            reserves.retain(|pool_address, entry| {
                if entry.is_expired() {
                    debug!("Removing expired reserves cache for: {}", pool_address);
                    reserves_removed += 1;
                    false
                } else {
                    true
                }
            });
        }

        if pools_removed > 0 || reserves_removed > 0 {
            debug!("Cache cleanup: removed {} pool lists, {} reserve entries", 
                   pools_removed, reserves_removed);
        }
    }

    pub async fn get_cache_stats(&self) -> CacheStats {
        let pools = self.pools.read().await;
        let reserves = self.pool_reserves.read().await;
        
        let mut pool_entries = 0;
        let mut expired_pool_entries = 0;
        let mut reserve_entries = 0;
        let mut expired_reserve_entries = 0;

        for entry in pools.values() {
            pool_entries += 1;
            if entry.is_expired() {
                expired_pool_entries += 1;
            }
        }

        for entry in reserves.values() {
            reserve_entries += 1;
            if entry.is_expired() {
                expired_reserve_entries += 1;
            }
        }

        CacheStats {
            pool_entries,
            expired_pool_entries,
            reserve_entries,
            expired_reserve_entries,
        }
    }

    // Start a background task to periodically clean up expired entries
    pub fn start_cleanup_task(&self) -> tokio::task::JoinHandle<()> {
        let cache = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60)); // Clean every minute
            loop {
                interval.tick().await;
                cache.cleanup_expired().await;
            }
        })
    }
}

impl Clone for PoolCache {
    fn clone(&self) -> Self {
        Self {
            pools: Arc::clone(&self.pools),
            pool_reserves: Arc::clone(&self.pool_reserves),
            default_ttl: self.default_ttl,
            reserves_ttl: self.reserves_ttl,
        }
    }
}

#[derive(Debug)]
pub struct CacheStats {
    pub pool_entries: usize,
    pub expired_pool_entries: usize,
    pub reserve_entries: usize,
    pub expired_reserve_entries: usize,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total_entries = self.pool_entries + self.reserve_entries;
        let valid_entries = total_entries - self.expired_pool_entries - self.expired_reserve_entries;
        
        if total_entries == 0 {
            0.0
        } else {
            valid_entries as f64 / total_entries as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Pool, TokenInfo};
    use solana_sdk::pubkey::Pubkey;
    use rust_decimal::Decimal;

    fn create_test_pool() -> Pool {
        Pool {
            address: Pubkey::new_unique(),
            dex: "test".to_string(),
            token_a: TokenInfo {
                mint: Pubkey::new_unique(),
                symbol: "TESTA".to_string(),
                decimals: 9,
                price_usd: None,
            },
            token_b: TokenInfo {
                mint: Pubkey::new_unique(),
                symbol: "TESTB".to_string(),
                decimals: 6,
                price_usd: None,
            },
            reserve_a: 1000000,
            reserve_b: 2000000,
            liquidity_usd: Decimal::from(10000),
            fee_percent: Decimal::from_f64_retain(0.003).unwrap(),
            last_updated: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_pool_cache_basic() {
        let cache = PoolCache::new();
        let pools = vec![create_test_pool()];
        
        // Test cache miss
        assert!(cache.get_pools("test_dex").await.is_none());
        
        // Test cache set and hit
        cache.set_pools("test_dex", pools.clone()).await;
        let cached_pools = cache.get_pools("test_dex").await;
        assert!(cached_pools.is_some());
        assert_eq!(cached_pools.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_pool_reserves_cache() {
        let cache = PoolCache::new();
        let pool_address = "test_pool_address";
        let reserves = (1000000u64, 2000000u64);
        
        // Test cache miss
        assert!(cache.get_pool_reserves(pool_address).await.is_none());
        
        // Test cache set and hit
        cache.set_pool_reserves(pool_address, reserves).await;
        let cached_reserves = cache.get_pool_reserves(pool_address).await;
        assert!(cached_reserves.is_some());
        assert_eq!(cached_reserves.unwrap(), reserves);
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let cache = PoolCache::with_ttl(
            Duration::from_millis(50), // Very short TTL for testing
            Duration::from_millis(50),
        );
        
        let pools = vec![create_test_pool()];
        cache.set_pools("test_dex", pools).await;
        
        // Should be cached immediately
        assert!(cache.get_pools("test_dex").await.is_some());
        
        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(60)).await;
        
        // Should be expired now
        assert!(cache.get_pools("test_dex").await.is_none());
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache = PoolCache::new();
        let pools = vec![create_test_pool()];
        
        cache.set_pools("test_dex", pools).await;
        cache.set_pool_reserves("test_pool", (1000, 2000)).await;
        
        let stats = cache.get_cache_stats().await;
        assert_eq!(stats.pool_entries, 1);
        assert_eq!(stats.reserve_entries, 1);
        assert_eq!(stats.expired_pool_entries, 0);
        assert_eq!(stats.expired_reserve_entries, 0);
    }
}