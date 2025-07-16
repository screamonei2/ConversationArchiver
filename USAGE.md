# Solana Arbitrage Bot - Quick Start Guide

## 🚀 Getting Started

### 1. First Time Setup

The bot is already configured and ready to run in **simulation mode** by default. This means it will:
- Monitor DEXs for arbitrage opportunities
- Calculate potential profits
- Show you what trades it would make
- **NOT execute any real transactions**

### 2. Run in Simulation Mode (Safe)

```bash
cargo run
```

This will start the bot and you'll see output like:
```
INFO Starting Solana Arbitrage Bot
INFO Configuration loaded successfully
INFO RPC client initialized
INFO DEX clients initialized
INFO Starting main arbitrage loop
INFO Found 3 potential opportunities
INFO Simulating arbitrage opportunity: direct_arb_123
```

### 3. Understanding the Output

The bot will continuously:
- Fetch pool data from Orca, Raydium, and Phoenix
- Scan for profitable arbitrage opportunities
- Display potential profits in simulation mode
- Monitor mempool for whale activity

### 4. When You're Ready for Live Trading

⚠️ **WARNING**: Only do this when you understand the risks and have tested thoroughly.

1. **Add your private key** to `.env`:
   ```bash
   PRIVATE_KEY=your_base58_private_key_here
   ```

2. **Enable live trading**:
   ```bash
   EXECUTE_TRADES=true
   SIMULATION_MODE=false
   ```

3. **Start with small amounts**:
   ```bash
   MAX_POSITION_SIZE_SOL=0.1  # Start with 0.1 SOL
   ```

### 5. Configuration Options

#### Risk Management (in `.env` or `config.toml`)
- `PROFIT_THRESHOLD_PERCENT=0.5` - Minimum profit % to execute
- `MAX_SLIPPAGE_PERCENT=1.0` - Maximum acceptable slippage
- `MIN_LIQUIDITY_USD=10000` - Minimum pool liquidity required
- `MAX_POSITION_SIZE_SOL=1.0` - Maximum SOL per trade

#### Performance Tuning
- `COOLDOWN_SECONDS=5` - Wait time between trades
- `MAX_REQUESTS_PER_SECOND=10` - API rate limiting

#### Advanced Features
- `QUICKNODE_RPC_URL` - Use faster RPC (recommended for live trading)
- `WHALE_WALLET_ADDRESSES` - Monitor specific wallets
- `MIN_WHALE_TRANSACTION_SOL=10.0` - Whale activity threshold

### 6. What the Bot Does

1. **Monitors Multiple DEXs**: Orca, Raydium, Phoenix
2. **Finds Price Differences**: Looks for profitable arbitrage opportunities
3. **Calculates Risks**: Considers slippage, fees, liquidity
4. **Executes Trades**: In the optimal order with priority fees
5. **Tracks Performance**: Logs all activities and profits

### 7. Common Arbitrage Patterns

- **Direct Arbitrage**: Buy token X on Orca, sell on Raydium
- **Cross-DEX**: Exploit price differences between AMM and orderbook
- **Triangular**: A→B→C→A cycles for profit

### 8. Safety Features

- **Simulation First**: All transactions are simulated before execution
- **Rate Limiting**: Protects against API abuse
- **Failure Handling**: Automatic retries with exponential backoff
- **Profit Validation**: Only executes if profit exceeds thresholds

### 9. Monitoring Your Bot

Watch the logs for:
- `Found X potential opportunities` - Bot is working
- `Executing arbitrage` - Live trade happening
- `Trade executed successfully` - Successful arbitrage
- `No profitable opportunities found` - Normal during low volatility

### 10. Troubleshooting

**Bot not finding opportunities?**
- Lower `PROFIT_THRESHOLD_PERCENT`
- Check `MIN_LIQUIDITY_USD` isn't too high
- Verify RPC connection is working

**Compilation errors?**
- Run `cargo clean && cargo build`
- Check all dependencies are installed

**High gas fees?**
- Adjust priority fees in the code
- Consider using different RPC endpoints

## 📊 Expected Performance

- **Opportunities**: 0-50 per hour depending on market conditions
- **Profit per trade**: 0.5-5% typically
- **Success rate**: 70-90% in simulation testing
- **Gas costs**: ~0.001-0.005 SOL per transaction

## 🔒 Security Reminders

1. **Never share your private key**
2. **Start with small amounts**
3. **Monitor the bot actively**
4. **Keep SIMULATION_MODE=true until confident**
5. **Have stop-loss limits**

## 🆘 Getting Help

- Check logs for error messages
- Verify configuration settings
- Test in simulation mode first
- Monitor market conditions

Remember: Arbitrage is competitive and profits depend on market volatility and your execution speed!