pub mod orca;
pub mod raydium;
pub mod phoenix;
pub mod mock_data;

use crate::models::Pool;
use anyhow::Result;
use async_trait::async_trait;
use crate::console::ConsoleManager;
use std::sync::Arc;

#[async_trait]
pub trait DexClient: Send + Sync {
    async fn fetch_pools(&self) -> Result<Vec<Pool>>;
    async fn get_pool_by_tokens(&self, token_a: &str, token_b: &str) -> Result<Option<Pool>>;
    async fn update_pool_reserves(&self, pool: &mut Pool) -> anyhow::Result<()>;
    fn get_dex_name(&self) -> &'static str;
    fn set_console_manager(&mut self, console: Arc<ConsoleManager>);
}
