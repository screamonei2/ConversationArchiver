use anyhow::{Context, Result};
use governor::{Quota, RateLimiter};
use reqwest::Client;
use serde_json::{json, Value};
use solana_client::{
    rpc_client::RpcClient as SolanaRpcClient,
    rpc_response::RpcSimulateTransactionResult,
};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    hash::Hash,
    pubkey::Pubkey,
    signature::Signature,
    transaction::Transaction,
    epoch_info::EpochInfo,
    account::Account,
};
use std::{num::NonZeroU32, sync::Arc, time::Duration};
use tracing::{debug, error, warn};

use crate::config::Config;

pub struct RpcClient {
    solana_client: SolanaRpcClient,
    http_client: Client,
    rate_limiter: Arc<RateLimiter<governor::state::direct::NotKeyed, governor::state::InMemoryState, governor::clock::DefaultClock>>,
    rpc_url: String,
}

impl Clone for RpcClient {
    fn clone(&self) -> Self {
        Self {
            solana_client: SolanaRpcClient::new_with_commitment(
                self.rpc_url.clone(),
                CommitmentConfig::confirmed(),
            ),
            http_client: self.http_client.clone(),
            rate_limiter: Arc::clone(&self.rate_limiter),
            rpc_url: self.rpc_url.clone(),
        }
    }
}

impl RpcClient {
    pub fn new(config: &Config) -> Result<Self> {
        let rpc_url = config.rpc.quicknode_rpc_url
            .as_ref()
            .unwrap_or(&config.rpc.solana_rpc_url)
            .clone();

        let solana_client = SolanaRpcClient::new_with_commitment(
            rpc_url.clone(),
            CommitmentConfig::confirmed(),
        );

        let http_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        // Create rate limiter
        let quota = Quota::per_second(NonZeroU32::new(config.rpc.max_requests_per_second).unwrap())
            .allow_burst(NonZeroU32::new(config.rpc.burst_size).unwrap());
        let rate_limiter = Arc::new(RateLimiter::direct(quota));

        Ok(Self {
            solana_client,
            http_client,
            rate_limiter,
            rpc_url,
        })
    }

    async fn wait_for_rate_limit(&self) {
        self.rate_limiter.until_ready().await;
    }

    pub fn get_url(&self) -> &str {
        &self.rpc_url
    }

    pub async fn get_latest_blockhash(&self) -> Result<Hash> {
        self.wait_for_rate_limit().await;
        
        let blockhash = self.solana_client
            .get_latest_blockhash()
            .context("Failed to get latest blockhash")?;
        
        debug!("Retrieved latest blockhash: {}", blockhash);
        Ok(blockhash)
    }

    pub async fn get_account(&self, address: &Pubkey) -> Result<Account> {
        self.wait_for_rate_limit().await;
        
        match self.solana_client.get_account(address) {
            Ok(account) => {
                debug!("Retrieved account for {}: {} bytes", address, account.data.len());
                Ok(account)
            }
            Err(e) => {
                // Check if it's a common "account not found" error
                let error_str = e.to_string();
                if error_str.contains("AccountNotFound") || error_str.contains("could not find account") {
                    debug!("Account not found: {}", address);
                } else {
                    warn!("Failed to get account for {}: {}", address, e);
                }
                anyhow::bail!("Account fetch failed: {}", e);
            }
        }
    }

    pub async fn get_account_data(&self, address: &Pubkey) -> Result<Vec<u8>> {
        self.wait_for_rate_limit().await;
        
        match self.solana_client.get_account_data(address) {
            Ok(data) => {
                debug!("Retrieved account data for {}: {} bytes", address, data.len());
                Ok(data)
            }
            Err(e) => {
                error!("Failed to get account data for {}: {}", address, e);
                anyhow::bail!("Account data fetch failed: {}", e);
            }
        }
    }

    pub async fn simulate_transaction(&self, transaction: &Transaction) -> Result<RpcSimulateTransactionResult> {
        self.wait_for_rate_limit().await;
        
        match self.solana_client.simulate_transaction(transaction) {
            Ok(result) => {
                debug!("Transaction simulation completed");
                Ok(result.value)
            }
            Err(e) => {
                error!("Transaction simulation failed: {}", e);
                anyhow::bail!("Simulation failed: {}", e);
            }
        }
    }

    pub async fn send_transaction(&self, transaction: &Transaction) -> Result<Signature> {
        self.wait_for_rate_limit().await;
        
        match self.solana_client.send_transaction(transaction) {
            Ok(signature) => {
                debug!("Transaction sent successfully: {}", signature);
                Ok(signature)
            }
            Err(e) => {
                error!("Failed to send transaction: {}", e);
                anyhow::bail!("Transaction send failed: {}", e);
            }
        }
    }

    pub async fn get_signature_status(&self, signature: &Signature) -> Result<bool> {
        self.wait_for_rate_limit().await;
        
        match self.solana_client.get_signature_status(signature) {
            Ok(Some(Ok(()))) => Ok(true),
            Ok(Some(Err(_))) => Ok(false),
            Ok(None) => Ok(false),
            Err(e) => {
                warn!("Failed to get signature status for {}: {}", signature, e);
                Ok(false)
            }
        }
    }

    pub async fn get_transaction_info(&self, signature: &str) -> Result<Value> {
        self.wait_for_rate_limit().await;
        
        let request_body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getTransaction",
            "params": [
                signature,
                {
                    "encoding": "json",
                    "commitment": "confirmed",
                    "maxSupportedTransactionVersion": 0
                }
            ]
        });

        let response = self.http_client
            .post(&self.rpc_url)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .context("Failed to send transaction info request")?;

        if !response.status().is_success() {
            anyhow::bail!("RPC request failed with status: {}", response.status());
        }

        let response_json: Value = response.json().await
            .context("Failed to parse transaction info response")?;

        if let Some(error) = response_json.get("error") {
            anyhow::bail!("RPC error: {}", error);
        }

        response_json.get("result")
            .cloned()
            .context("No result in transaction info response")
    }

    pub async fn get_multiple_accounts(&self, addresses: &[Pubkey]) -> Result<Vec<Option<Account>>> {
        self.wait_for_rate_limit().await;
        
        match self.solana_client.get_multiple_accounts(addresses) {
            Ok(accounts) => {
                debug!("Retrieved {} accounts", accounts.len());
                Ok(accounts)
            }
            Err(e) => {
                error!("Failed to get multiple accounts: {}", e);
                anyhow::bail!("Multiple accounts fetch failed: {}", e);
            }
        }
    }

    pub async fn get_token_account_balance(&self, token_account: &Pubkey) -> Result<u64> {
        self.wait_for_rate_limit().await;
        
        match self.solana_client.get_token_account_balance(token_account) {
            Ok(balance) => {
                let amount = balance.amount.parse::<u64>()
                    .context("Failed to parse token balance")?;
                debug!("Token account {} balance: {}", token_account, amount);
                Ok(amount)
            }
            Err(e) => {
                // Check for common token account errors and handle them gracefully
                let error_str = e.to_string();
                if error_str.contains("could not find account") {
                    debug!("Token account not found: {}", token_account);
                } else if error_str.contains("not a Token account") {
                    debug!("Address {} is not a valid token account", token_account);
                } else {
                    warn!("Failed to get token account balance for {}: {}", token_account, e);
                }
                anyhow::bail!("Token balance fetch failed: {}", e);
            }
        }
    }

    pub async fn get_sol_balance(&self, address: &Pubkey) -> Result<u64> {
        self.wait_for_rate_limit().await;
        
        match self.solana_client.get_balance(address) {
            Ok(balance) => {
                debug!("SOL balance for {}: {} lamports", address, balance);
                Ok(balance)
            }
            Err(e) => {
                error!("Failed to get SOL balance for {}: {}", address, e);
                anyhow::bail!("SOL balance fetch failed: {}", e);
            }
        }
    }

    pub async fn get_recent_blockhash(&self) -> Result<(Hash, u64)> {
        self.wait_for_rate_limit().await;
        
        match self.solana_client.get_latest_blockhash() {
            Ok(hash) => {
                debug!("Recent blockhash: {}", hash);
                Ok((hash, 0)) // Assuming fee_calculator is no longer needed or can be set to a default/dummy value
            }
            Err(e) => {
                error!("Failed to get recent blockhash: {}", e);
                anyhow::bail!("Recent blockhash fetch failed: {}", e);
            }
        }
    }

    pub async fn send_and_confirm_transaction(&self, transaction: &Transaction) -> Result<Signature> {
        self.wait_for_rate_limit().await;
        
        match self.solana_client.send_and_confirm_transaction(transaction) {
            Ok(signature) => {
                debug!("Transaction sent and confirmed: {}", signature);
                Ok(signature)
            }
            Err(e) => {
                error!("Failed to send and confirm transaction: {}", e);
                anyhow::bail!("Transaction send and confirm failed: {}", e);
            }
        }
    }

    pub async fn get_minimum_balance_for_rent_exemption(&self, data_len: usize) -> Result<u64> {
        self.wait_for_rate_limit().await;
        
        match self.solana_client.get_minimum_balance_for_rent_exemption(data_len) {
            Ok(balance) => {
                debug!("Minimum balance for {} bytes: {} lamports", data_len, balance);
                Ok(balance)
            }
            Err(e) => {
                error!("Failed to get minimum balance for rent exemption: {}", e);
                anyhow::bail!("Rent exemption fetch failed: {}", e);
            }
        }
    }

    pub async fn get_fees(&self) -> Result<u64> {
        self.wait_for_rate_limit().await;
        
        // Use a simple approach since get_fees is deprecated
        Ok(5000) // Default fee in lamports
    }

    pub async fn get_epoch_info(&self) -> Result<EpochInfo> {
        self.wait_for_rate_limit().await;
        
        match self.solana_client.get_epoch_info() {
            Ok(epoch_info) => {
                debug!("Epoch info: {:?}", epoch_info);
                Ok(epoch_info)
            }
            Err(e) => {
                error!("Failed to get epoch info: {}", e);
                anyhow::bail!("Epoch info fetch failed: {}", e);
            }
        }
    }

    pub async fn get_program_accounts(&self, program_id: &Pubkey) -> Result<Vec<(Pubkey, Account)>> {
        self.wait_for_rate_limit().await;
        
        match self.solana_client.get_program_accounts(program_id) {
            Ok(accounts) => {
                debug!("Retrieved {} program accounts for {}", accounts.len(), program_id);
                Ok(accounts)
            }
            Err(e) => {
                error!("Failed to get program accounts for {}: {}", program_id, e);
                anyhow::bail!("Program accounts fetch failed: {}", e);
            }
        }
    }

    pub async fn get_health(&self) -> Result<()> {
        self.wait_for_rate_limit().await;
        
        match self.solana_client.get_health() {
            Ok(_) => {
                debug!("RPC health check passed");
                Ok(())
            }
            Err(e) => {
                error!("RPC health check failed: {}", e);
                anyhow::bail!("Health check failed: {}", e);
            }
        }
    }

    // Helper methods that return Option instead of failing for common scenarios
    
    /// Get account if it exists, returns None if account not found
    pub async fn try_get_account(&self, address: &Pubkey) -> Result<Option<Account>> {
        self.wait_for_rate_limit().await;
        
        match self.solana_client.get_account(address) {
            Ok(account) => {
                debug!("Retrieved account for {}: {} bytes", address, account.data.len());
                Ok(Some(account))
            }
            Err(e) => {
                let error_str = e.to_string();
                if error_str.contains("AccountNotFound") || error_str.contains("could not find account") {
                    debug!("Account not found: {}", address);
                    Ok(None)
                } else {
                    warn!("Failed to get account for {}: {}", address, e);
                    anyhow::bail!("Account fetch failed: {}", e);
                }
            }
        }
    }

    /// Get token account balance if valid, returns None if account doesn't exist or isn't a token account
    pub async fn try_get_token_account_balance(&self, token_account: &Pubkey) -> Result<Option<u64>> {
        self.wait_for_rate_limit().await;
        
        match self.solana_client.get_token_account_balance(token_account) {
            Ok(balance) => {
                let amount = balance.amount.parse::<u64>()
                    .context("Failed to parse token balance")?;
                debug!("Token account {} balance: {}", token_account, amount);
                Ok(Some(amount))
            }
            Err(e) => {
                let error_str = e.to_string();
                if error_str.contains("could not find account") || error_str.contains("not a Token account") {
                    debug!("Token account {} not found or invalid", token_account);
                    Ok(None)
                } else {
                    warn!("Failed to get token account balance for {}: {}", token_account, e);
                    anyhow::bail!("Token balance fetch failed: {}", e);
                }
            }
        }
    }

    /// Get SOL balance if account exists, returns None if account not found
    pub async fn try_get_sol_balance(&self, address: &Pubkey) -> Result<Option<u64>> {
        self.wait_for_rate_limit().await;
        
        match self.solana_client.get_balance(address) {
            Ok(balance) => {
                debug!("SOL balance for {}: {} lamports", address, balance);
                Ok(Some(balance))
            }
            Err(e) => {
                let error_str = e.to_string();
                if error_str.contains("AccountNotFound") || error_str.contains("could not find account") {
                    debug!("Account not found for SOL balance: {}", address);
                    Ok(None)
                } else {
                    warn!("Failed to get SOL balance for {}: {}", address, e);
                    anyhow::bail!("SOL balance fetch failed: {}", e);
                }
            }
        }
    }
}
