pub mod orca;
pub mod raydium;
pub mod phoenix;

use crate::models::Pool;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait DexClient: Send + Sync {
    async fn fetch_pools(&self) -> Result<Vec<Pool>>;
    async fn get_pool_by_tokens(&self, token_a: &str, token_b: &str) -> Result<Option<Pool>>;
    async fn update_pool_reserves(&self, pool: &mut Pool) -> Result<()>;
    fn get_dex_name(&self) -> &'static str;
}
