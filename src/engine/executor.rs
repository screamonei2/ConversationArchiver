use crate::{
    config::Config,
    models::ArbitrageOpportunity,
    utils::rpc::RpcClient,
};
use anyhow::{Context, Result};
use solana_client::rpc_response::RpcSimulateTransactionResult;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    instruction::Instruction,
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    transaction::Transaction,
};
use std::{collections::HashSet, str::FromStr, sync::Arc};
use tracing::{debug, info, warn};

pub struct Executor {
    config: Config,
    rpc_client: Arc<RpcClient>,
    trading_keypair: Option<Keypair>,
}

impl Executor {
    pub fn new(config: Config, rpc_client: Arc<RpcClient>) -> Result<Self> {
        let trading_keypair = if let Some(private_key) = &config.bot.private_key {
            Some(Self::keypair_from_private_key(private_key)?)
        } else {
            None
        };

        Ok(Self {
            config,
            rpc_client,
            trading_keypair,
        })
    }

    pub async fn execute_arbitrage(&self, opportunity: &ArbitrageOpportunity) -> Result<String> {
        if self.config.bot.simulation_mode {
            return self.simulate_arbitrage(opportunity).await;
        }

        if !self.config.bot.execute_trades {
            info!("Trade execution disabled in configuration");
            return Ok("execution_disabled".to_string());
        }

        let trading_keypair = self.trading_keypair.as_ref()
            .context("No trading keypair configured")?;

        info!("Executing arbitrage opportunity: {}", opportunity.id);

        // Validate opportunity before execution
        self.validate_arbitrage_opportunity(opportunity)?;

        // Build transaction instructions
        let instructions = self.build_arbitrage_instructions(opportunity).await?;
        
        // Validate transaction security
        self.validate_transaction_security(&instructions, trading_keypair)?;
        
        // Simulate transaction first
        let simulation_result = self.simulate_transaction(&instructions, trading_keypair).await?;
        
        if !self.is_simulation_successful(&simulation_result) {
            anyhow::bail!("Transaction simulation failed: {:?}", simulation_result.err);
        }

        // Validate simulation results
        self.validate_simulation_results(&simulation_result)?;

        info!("Simulation successful, proceeding with execution");

        // Execute the transaction
        let signature = self.send_transaction(instructions, trading_keypair).await?;
        
        // Wait for confirmation
        self.wait_for_confirmation(&signature).await?;
        
        info!("Arbitrage executed successfully: {}", signature);
        Ok(signature.to_string())
    }

    async fn simulate_arbitrage(&self, opportunity: &ArbitrageOpportunity) -> Result<String> {
        info!("Simulating arbitrage opportunity: {}", opportunity.id);
        
        // In simulation mode, we just validate the opportunity and log details
        debug!("Route: {:?}", opportunity.route.route_type);
        debug!("Expected profit: {:.4} SOL ({:.2}%)", 
               opportunity.expected_profit as f64 / 1_000_000_000.0,
               opportunity.expected_profit_percent);
        debug!("Confidence: {:.2}, Risk: {:.2}", 
               opportunity.confidence_score, 
               opportunity.risk_score);

        // Simulate some processing time
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        Ok(format!("simulated_{}", opportunity.id))
    }

    async fn build_arbitrage_instructions(&self, opportunity: &ArbitrageOpportunity) -> Result<Vec<Instruction>> {
        let mut instructions = Vec::new();

        // Add compute budget instruction to ensure enough compute units
        let compute_units = self.estimate_compute_units(opportunity)?;
        instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(compute_units));

        // Add priority fee instruction for faster processing
        let priority_fee = 1000; // microlamports per compute unit
        instructions.push(ComputeBudgetInstruction::set_compute_unit_price(priority_fee));

        // Build swap instructions for each step in the route
        for (i, step) in opportunity.route.steps.iter().enumerate() {
            let swap_instruction = self.build_swap_instruction(step, i == 0).await?;
            instructions.push(swap_instruction);
        }

        Ok(instructions)
    }

    async fn build_swap_instruction(&self, step: &crate::models::TradeStep, _is_first: bool) -> Result<Instruction> {
        // This is a placeholder implementation
        // In a real implementation, you would build actual swap instructions
        // based on the DEX (Orca, Raydium, Phoenix) and the specific program interfaces

        match step.pool.dex.as_str() {
            "orca" => self.build_orca_swap_instruction(step).await,
            "raydium" => self.build_raydium_swap_instruction(step).await,
            "phoenix" => self.build_phoenix_swap_instruction(step).await,
            _ => anyhow::bail!("Unsupported DEX: {}", step.pool.dex),
        }
    }

    async fn build_orca_swap_instruction(&self, step: &crate::models::TradeStep) -> Result<Instruction> {
        use solana_sdk::instruction::AccountMeta;
        
        let program_id = Pubkey::from_str("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc")?; // Orca Whirlpool program ID
        
        let trading_keypair = self.trading_keypair.as_ref()
            .context("No trading keypair configured")?;
        
        // Get associated token accounts for the trader
        let token_a_ata = spl_associated_token_account::get_associated_token_address(
            &trading_keypair.pubkey(),
            &step.pool.token_a.mint,
        );
        let token_b_ata = spl_associated_token_account::get_associated_token_address(
            &trading_keypair.pubkey(),
            &step.pool.token_b.mint,
        );
        
        // Build Orca Whirlpool swap instruction
        let accounts = vec![
            AccountMeta::new_readonly(spl_token::id(), false), // Token program
            AccountMeta::new(trading_keypair.pubkey(), true), // Trader
            AccountMeta::new(step.pool.address, false), // Whirlpool
            AccountMeta::new(token_a_ata, false), // Token A account
            AccountMeta::new(token_b_ata, false), // Token B account
            AccountMeta::new_readonly(step.pool.token_a.mint, false), // Token A mint
            AccountMeta::new_readonly(step.pool.token_b.mint, false), // Token B mint
        ];
        
        // Simplified instruction data for Orca swap
        // In production, use proper Orca SDK instruction builders
        let mut instruction_data = vec![0x09]; // Swap instruction discriminator
        instruction_data.extend_from_slice(&step.input_amount.to_le_bytes());
        instruction_data.extend_from_slice(&step.expected_output.to_le_bytes());
        
        Ok(Instruction {
            program_id,
            accounts,
            data: instruction_data,
        })
    }

    async fn build_raydium_swap_instruction(&self, step: &crate::models::TradeStep) -> Result<Instruction> {
        use solana_sdk::instruction::AccountMeta;
        
        let program_id = Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8")?; // Raydium AMM program ID
        
        let trading_keypair = self.trading_keypair.as_ref()
            .context("No trading keypair configured")?;
        
        // Get associated token accounts for the trader
        let token_a_ata = spl_associated_token_account::get_associated_token_address(
            &trading_keypair.pubkey(),
            &step.pool.token_a.mint,
        );
        let token_b_ata = spl_associated_token_account::get_associated_token_address(
            &trading_keypair.pubkey(),
            &step.pool.token_b.mint,
        );
        
        // Build Raydium AMM swap instruction
        let accounts = vec![
            AccountMeta::new_readonly(spl_token::id(), false), // Token program
            AccountMeta::new(step.pool.address, false), // AMM pool
            AccountMeta::new_readonly(trading_keypair.pubkey(), true), // User authority
            AccountMeta::new(token_a_ata, false), // User token A account
            AccountMeta::new(token_b_ata, false), // User token B account
            AccountMeta::new_readonly(step.pool.token_a.mint, false), // Token A mint
            AccountMeta::new_readonly(step.pool.token_b.mint, false), // Token B mint
        ];
        
        // Simplified instruction data for Raydium swap
        // In production, use proper Raydium SDK instruction builders
        let mut instruction_data = vec![0x09]; // Swap instruction discriminator
        instruction_data.extend_from_slice(&step.input_amount.to_le_bytes());
        instruction_data.extend_from_slice(&step.expected_output.to_le_bytes());
        
        Ok(Instruction {
            program_id,
            accounts,
            data: instruction_data,
        })
    }

    async fn build_phoenix_swap_instruction(&self, step: &crate::models::TradeStep) -> Result<Instruction> {
        use solana_sdk::instruction::AccountMeta;
        
        let program_id = Pubkey::from_str("PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY")?; // Phoenix program ID
        
        let trading_keypair = self.trading_keypair.as_ref()
            .context("No trading keypair configured")?;
        
        // Get associated token accounts for the trader
        let token_a_ata = spl_associated_token_account::get_associated_token_address(
            &trading_keypair.pubkey(),
            &step.pool.token_a.mint,
        );
        let token_b_ata = spl_associated_token_account::get_associated_token_address(
            &trading_keypair.pubkey(),
            &step.pool.token_b.mint,
        );
        
        // Build Phoenix swap instruction
        let accounts = vec![
            AccountMeta::new_readonly(spl_token::id(), false), // Token program
            AccountMeta::new(trading_keypair.pubkey(), true), // Trader
            AccountMeta::new(step.pool.address, false), // Phoenix market
            AccountMeta::new(token_a_ata, false), // Token A account
            AccountMeta::new(token_b_ata, false), // Token B account
            AccountMeta::new_readonly(step.pool.token_a.mint, false), // Token A mint
            AccountMeta::new_readonly(step.pool.token_b.mint, false), // Token B mint
        ];
        
        // Simplified instruction data for Phoenix swap
        // In production, use proper Phoenix SDK instruction builders
        let mut instruction_data = vec![0x01]; // Swap instruction discriminator
        instruction_data.extend_from_slice(&step.input_amount.to_le_bytes());
        instruction_data.extend_from_slice(&step.expected_output.to_le_bytes());
        
        Ok(Instruction {
            program_id,
            accounts,
            data: instruction_data,
        })
    }

    fn estimate_compute_units(&self, opportunity: &ArbitrageOpportunity) -> Result<u32> {
        // Estimate compute units based on the number of steps and complexity
        let base_units = 50_000u32;
        let per_step_units = 100_000u32;
        
        let total_units = base_units + (opportunity.route.steps.len() as u32 * per_step_units);
        
        // Cap at maximum allowed compute units
        Ok(total_units.min(1_400_000))
    }

    async fn simulate_transaction(&self, instructions: &[Instruction], keypair: &Keypair) -> Result<RpcSimulateTransactionResult> {
        let recent_blockhash = self.rpc_client.get_latest_blockhash().await?;
        
        let message = Message::new(instructions, Some(&keypair.pubkey()));
        let transaction = Transaction::new(&[keypair], message, recent_blockhash);
        
        let simulation_result = self.rpc_client.simulate_transaction(&transaction).await?;
        
        debug!("Transaction simulation result: {:?}", simulation_result);
        Ok(simulation_result)
    }

    fn is_simulation_successful(&self, result: &RpcSimulateTransactionResult) -> bool {
        result.err.is_none()
    }

    async fn send_transaction(&self, instructions: Vec<Instruction>, keypair: &Keypair) -> Result<Signature> {
        let recent_blockhash = self.rpc_client.get_latest_blockhash().await?;
        
        let message = Message::new(&instructions, Some(&keypair.pubkey()));
        let transaction = Transaction::new(&[keypair], message, recent_blockhash);
        
        let signature = self.rpc_client.send_transaction(&transaction).await?;
        
        debug!("Transaction sent with signature: {}", signature);
        Ok(signature)
    }

    async fn wait_for_confirmation(&self, signature: &Signature) -> Result<()> {
        let max_retries = 30;
        let retry_delay = tokio::time::Duration::from_secs(2);
        
        for attempt in 0..max_retries {
            match self.rpc_client.get_signature_status(signature).await {
                Ok(true) => {
                    info!("Transaction confirmed: {}", signature);
                    return Ok(());
                }
                Ok(false) => {
                    debug!("Transaction not yet confirmed, attempt {}/{}", attempt + 1, max_retries);
                }
                Err(e) => {
                    warn!("Error checking transaction status: {}", e);
                }
            }
            
            if attempt < max_retries - 1 {
                tokio::time::sleep(retry_delay).await;
            }
        }
        
        anyhow::bail!("Transaction confirmation timeout after {} attempts", max_retries)
    }

    fn keypair_from_private_key(private_key: &str) -> Result<Keypair> {
        // Handle different private key formats
        if private_key.starts_with('[') && private_key.ends_with(']') {
            // JSON array format
            let bytes: Vec<u8> = serde_json::from_str(private_key)?;
            Ok(Keypair::from_bytes(&bytes)?)
        } else if private_key.len() == 88 || private_key.len() == 87 {
            // Base58 format
            let bytes = bs58::decode(private_key).into_vec()?;
            Ok(Keypair::from_bytes(&bytes)?)
        } else {
            anyhow::bail!("Unsupported private key format");
        }
    }

    fn validate_arbitrage_opportunity(&self, opportunity: &ArbitrageOpportunity) -> Result<()> {
        // Validate profit threshold
        if opportunity.expected_profit_percent < self.config.bot.profit_threshold_percent {
            anyhow::bail!("Opportunity profit {:.2}% below threshold {:.2}%", 
                         opportunity.expected_profit_percent, 
                         self.config.bot.profit_threshold_percent);
        }

        // Validate position size
        let position_size_sol = opportunity.input_amount as f64 / 1_000_000_000.0;
        if position_size_sol > self.config.bot.max_position_size_sol {
            anyhow::bail!("Position size {:.2} SOL exceeds maximum {:.2} SOL", 
                         position_size_sol, 
                         self.config.bot.max_position_size_sol);
        }

        // Validate confidence and risk scores
        if opportunity.confidence_score < 0.7 {
            anyhow::bail!("Opportunity confidence score {:.2} too low", opportunity.confidence_score);
        }

        if opportunity.risk_score > 0.5 {
            anyhow::bail!("Opportunity risk score {:.2} too high", opportunity.risk_score);
        }

        // Validate route steps
        if opportunity.route.steps.is_empty() {
            anyhow::bail!("Empty arbitrage route");
        }

        if opportunity.route.steps.len() > 5 {
            anyhow::bail!("Arbitrage route too complex: {} steps", opportunity.route.steps.len());
        }

        Ok(())
    }

    fn validate_transaction_security(&self, instructions: &[Instruction], keypair: &Keypair) -> Result<()> {
        // Validate instruction count
        if instructions.len() > 10 {
            anyhow::bail!("Too many instructions in transaction: {}", instructions.len());
        }

        // Validate program IDs - only allow known DEX programs
        let allowed_programs = self.get_allowed_program_ids();
        for instruction in instructions {
            if !allowed_programs.contains(&instruction.program_id) && 
               instruction.program_id != solana_sdk::compute_budget::id() {
                anyhow::bail!("Unauthorized program ID: {}", instruction.program_id);
            }
        }

        // Validate that all writable accounts belong to the trader
        for instruction in instructions {
            for account_meta in &instruction.accounts {
                if account_meta.is_writable && account_meta.is_signer {
                    if account_meta.pubkey != keypair.pubkey() {
                        anyhow::bail!("Unauthorized signer account: {}", account_meta.pubkey);
                    }
                }
            }
        }

        // Validate instruction data size
        for instruction in instructions {
            if instruction.data.len() > 1024 {
                anyhow::bail!("Instruction data too large: {} bytes", instruction.data.len());
            }
        }

        Ok(())
    }

    fn validate_simulation_results(&self, result: &RpcSimulateTransactionResult) -> Result<()> {
        // Check for any errors
        if let Some(err) = &result.err {
            anyhow::bail!("Simulation error: {:?}", err);
        }

        // Validate compute units consumed
        if let Some(units_consumed) = result.units_consumed {
            if units_consumed > 1_400_000 {
                anyhow::bail!("Transaction consumes too many compute units: {}", units_consumed);
            }
        }

        // Check for suspicious log messages
        if let Some(logs) = &result.logs {
            for log in logs {
                if log.contains("error") || log.contains("failed") || log.contains("insufficient") {
                    warn!("Suspicious log message: {}", log);
                }
            }
        }

        Ok(())
    }

    fn get_allowed_program_ids(&self) -> HashSet<Pubkey> {
        let mut allowed = HashSet::new();
        
        // Add known DEX program IDs
        if let Ok(orca_id) = Pubkey::from_str("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc") {
            allowed.insert(orca_id);
        }
        if let Ok(raydium_id) = Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8") {
            allowed.insert(raydium_id);
        }
        if let Ok(phoenix_id) = Pubkey::from_str("PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY") {
            allowed.insert(phoenix_id);
        }
        
        // Add system programs
        allowed.insert(spl_token::id());
        allowed.insert(spl_associated_token_account::id());
        allowed.insert(solana_sdk::system_program::id());
        allowed.insert(solana_sdk::compute_budget::id());
        
        allowed
    }
}
