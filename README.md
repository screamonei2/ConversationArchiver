# Solana Arbitrage Bot

A high-performance, multi-DEX arbitrage bot for the Solana blockchain that identifies and executes profitable trading opportunities across Orca, Raydium, and Phoenix DEXs.

## Features

### 🚀 Core Functionality
- **Multi-DEX Support**: Integrates with Orca, Raydium, and Phoenix DEXs
- **Multiple Arbitrage Types**: 
  - Direct arbitrage (same token pair across different DEXs)
  - Triangular arbitrage (three-token cycles within a single DEX)
  - Cross-DEX arbitrage (complex multi-hop opportunities)
- **Real-time Monitoring**: Continuous scanning for arbitrage opportunities
- **Intelligent Caching**: High-performance pool data caching with TTL
- **Risk Management**: Comprehensive position sizing and risk controls

### 🛡️ Security & Safety
- **Simulation Mode**: Test strategies without risking real funds
- **Transaction Validation**: Multi-layer security checks before execution
- **Private Key Protection**: Secure key handling with validation
- **Slippage Protection**: Configurable slippage tolerance
- **Position Limits**: Maximum position size controls

### 📊 Monitoring & Analytics
- **Real-time Console**: Live updates on opportunities and executions
- **Performance Metrics**: Track profits, success rates, and cache performance
- **Comprehensive Logging**: Detailed execution logs for analysis
- **Risk Scoring**: Confidence and risk assessment for each opportunity

### 🧪 Mock Data Support
- **Fallback Functionality**: Automatic fallback when DEX APIs are unavailable
- **Testing Environment**: Force mock data mode for development and testing
- **Realistic Data**: Generated pools with actual Solana token addresses
- **Multi-DEX Coverage**: Mock data for Orca, Raydium, and Phoenix

## Quick Start

### Prerequisites
- Rust 1.70+ installed
- Solana CLI tools
- RPC endpoint access (Helius, QuickNode, or public RPC)

### Installation

1. **Clone the repository**
```bash
git clone <repository-url>
cd solana-arbitrage-bot
```

2. **Install dependencies**
```bash
cargo build --release
```

3. **Configure the bot**
```bash
cp config.toml.example config.toml
# Edit config.toml with your settings
```

4. **Set environment variables**
```bash
export PRIVATE_KEY="your_base58_private_key_here"
export RPC_ENDPOINT="your_rpc_endpoint_here"
```

5. **Run the bot**
```bash
# Simulation mode (recommended for testing)
cargo run

# Live trading mode (use with caution)
EXECUTE_TRADES=true cargo run
```

## Configuration

### Basic Configuration (`config.toml`)

```toml
[bot]
execute_trades = false  # Set to true for live trading
min_liquidity_usd = 10000.0
profit_threshold_percent = 0.5
max_position_size_sol = 10.0
max_slippage_percent = 1.0

[rpc]
endpoint = "https://api.mainnet-beta.solana.com"
timeout_seconds = 30
max_retries = 3

[dexs]
enabled = ["orca", "raydium", "phoenix"]

[dexs.orca]
enabled = true
min_liquidity_usd = 5000.0

[dexs.raydium]
enabled = true
min_liquidity_usd = 5000.0

[dexs.phoenix]
enabled = true
min_liquidity_usd = 5000.0

[risk_management]
max_daily_loss_sol = 50.0
max_concurrent_trades = 5
stop_loss_percent = 2.0

[monitoring]
log_level = "info"
enable_metrics = true
metrics_port = 9090
```

### Environment Variables

| Variable | Description | Required |
|----------|-------------|----------|
| `PRIVATE_KEY` | Base58 encoded private key | Yes (for live trading) |
| `RPC_ENDPOINT` | Solana RPC endpoint URL | No (uses config default) |
| `EXECUTE_TRADES` | Enable live trading | No (defaults to false) |
| `USE_MOCK_DATA` | Force mock data mode | No (defaults to false) |
| `LOG_LEVEL` | Logging level (debug, info, warn, error) | No |

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

To get started with the Solana Arbitrage Bot, follow these steps:

### 1. Install Rust & Cargo
If you don't have Rust and Cargo installed, you can do so using `rustup`:
```bash
curl --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### 2. Setup Environment Variables
Copy the example environment file and then edit it with your specific configurations:
```bash
cp .env.example .env
nano .env
```

**Key variables to configure in `.env`:**
- `SOLANA_RPC_URL`: Your Solana RPC endpoint (e.g., a QuickNode or Ankr URL).
- `PRIVATE_KEY`: Your wallet's private key for executing trades (be extremely careful with this in live environments).
- `PROFIT_THRESHOLD_PERCENT`: Minimum profit percentage to trigger a trade.
- `SIMULATION_MODE`: Set to `true` for testing without real funds, `false` for live trading.

### 3. Run the Bot

**For testing in simulation mode:**
```bash
cargo run
```

**For live trading (build and run release version):**
```bash
cargo build --release
./target/release/solana-arbitrage-bot
```

Ensure `EXECUTE_TRADES=true` in your `.env` file for live trading.

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