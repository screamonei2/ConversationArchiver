# Solana Arbitrage Bot

A fully automated Solana arbitrage bot built in Rust that monitors multiple DEXs, detects profitable opportunities, and executes trades autonomously.

## 🚀 Features

### Core Arbitrage Strategies
- **Direct Arbitrage**: A → B → A across different DEXs
- **Triangular Arbitrage**: A → B → C → A within and across DEXs  
- **Cross-DEX Arbitrage**: Exploiting price differences between Orca, Raydium, and Phoenix
- **Multi-hop Route Discovery**: Complex arbitrage paths with multiple intermediate tokens

### Advanced Monitoring
- **Mempool Intelligence**: Real-time transaction monitoring via WebSocket connections
- **Whale Tracking**: Monitor large wallet movements and pre-position for opportunities
- **Front-running Detection**: Analyze pending transactions to act before price movements
- **Deep Orderbook Analysis**: Level 2 depth arbitrage on orderbook DEXs like Phoenix

### Risk Management
- **Simulation Mode**: Test strategies without real transactions
- **Slippage Protection**: Calculate and limit price impact
- **Position Sizing**: Dynamic capital allocation based on liquidity depth
- **Profit Thresholds**: Configurable minimum profit requirements
- **Rate Limiting**: Protect against API throttling

### Execution Engine
- **Transaction Simulation**: Pre-validate all trades before execution
- **Priority Fees**: Ensure fast transaction processing
- **Compute Budget Optimization**: Efficient compute unit allocation
- **Multi-step Transaction Building**: Complex arbitrage route execution
- **Confirmation Tracking**: Monitor transaction status until confirmed

## 🏗️ Architecture

### DEX Integration
- **Orca (Whirlpools)**: Concentrated liquidity pools
- **Raydium**: AMM and orderbook hybrid
- **Phoenix**: Pure orderbook DEX

### Core Components
- **Screener**: Identifies profitable arbitrage opportunities
- **Executor**: Builds and executes transactions
- **Monitor**: Tracks mempool and whale activities
- **RPC Client**: Rate-limited Solana RPC interactions
- **Math Utils**: AMM calculations and profit analysis

## 🛠️ Configuration

### Environment Variables
```bash
# RPC Configuration
SOLANA_RPC_URL=https://api.mainnet-beta.solana.com
QUICKNODE_RPC_URL=your_quicknode_url_here

# Bot Settings
PRIVATE_KEY=your_private_key_here
PROFIT_THRESHOLD_PERCENT=0.5
MAX_SLIPPAGE_PERCENT=1.0
MIN_LIQUIDITY_USD=10000
EXECUTE_TRADES=false
SIMULATION_MODE=true

# Risk Management
MAX_POSITION_SIZE_SOL=1.0
COOLDOWN_SECONDS=5
```

### Config File (config.toml)
```toml
[bot]
profit_threshold_percent = 0.5
max_slippage_percent = 1.0
min_liquidity_usd = 10000
execute_trades = false
simulation_mode = true

[dexs]
enabled = ["orca", "raydium", "phoenix"]

[monitoring]
mempool_enabled = true
whale_tracking_enabled = true
min_whale_transaction_sol = 10.0
```

## 🚀 Getting Started

### 1. Setup Environment
```bash
# Copy environment template
cp .env.example .env

# Edit your configuration
nano .env
```

### 2. Configure Trading
- Set `SIMULATION_MODE=true` for testing
- Add your `PRIVATE_KEY` for live trading
- Adjust profit thresholds and risk parameters

### 3. Run the Bot
```bash
# Start in simulation mode
cargo run

# Or build release version
cargo build --release
./target/release/solana-arbitrage-bot
```

## 💡 Strategy Overview

### The bot implements a comprehensive arbitrage strategy:

1. **Data Collection**: Continuously fetches pool data from all enabled DEXs
2. **Opportunity Screening**: Analyzes price differences and calculates potential profits
3. **Risk Assessment**: Evaluates liquidity depth, slippage, and confidence scores
4. **Execution Planning**: Builds optimized transaction instructions
5. **Simulation**: Pre-validates transactions to avoid failed trades
6. **Execution**: Sends transactions with priority fees for fast processing
7. **Monitoring**: Tracks confirmation and calculates actual profits

### Key Differentiators

- **Mempool Monitoring**: Act on information before it becomes public
- **Whale Tracking**: Follow large traders to anticipate market movements  
- **Multi-DEX Intelligence**: Simultaneous monitoring of AMM and orderbook DEXs
- **Smart Route Discovery**: Find complex arbitrage paths others miss
- **Risk-Aware Execution**: Comprehensive risk management and position sizing

## 🔒 Security

- Private keys are loaded from environment variables
- Rate limiting prevents API abuse
- Simulation mode for safe testing
- Configurable risk parameters
- Transaction confirmation tracking

## 📊 Performance

- **Low Latency**: Direct WebSocket connections and optimized execution paths
- **High Throughput**: Concurrent monitoring of multiple DEXs
- **Memory Efficient**: Rust's zero-cost abstractions and careful resource management
- **Scalable**: Modular architecture allows easy addition of new DEXs and strategies

## 🚨 Disclaimer

This is experimental software for educational purposes. Arbitrage trading involves significant risks including:
- Smart contract risks
- Market volatility
- Transaction failures
- Slippage and MEV
- Potential loss of funds

Always test thoroughly in simulation mode before risking real capital.

## 📈 Future Enhancements

- Flash loan integration for capital-free arbitrage
- ML-based opportunity scoring
- Cross-chain arbitrage opportunities  
- Advanced MEV protection strategies
- Real-time performance analytics dashboard