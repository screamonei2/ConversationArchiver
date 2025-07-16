# Solana Arbitrage Bot

## Overview

A fully automated Solana arbitrage bot built in Rust that monitors multiple DEXs (Orca, Raydium, Phoenix), detects profitable opportunities, and executes trades autonomously. The bot implements advanced strategies including mempool monitoring, whale tracking, and cross-DEX arbitrage with comprehensive risk management.

## User Preferences

Preferred communication style: Simple, everyday language.
Project goal: 100% automated arbitrage bot with zero manual intervention.

## System Architecture

### Core Framework
- **Language**: Rust (chosen for performance, memory safety, and low-latency requirements)
- **Build System**: Cargo with standard Rust project structure
- **Async Runtime**: Tokio for concurrent operations
- **Networking**: WebSocket connections for real-time data

### Project Structure
```
src/
├── main.rs                  # Entry point and main arbitrage loop
├── lib.rs                   # Module declarations
├── config.rs                # Configuration management (.env + .toml)
├── types.rs                 # Common type definitions
├── models.rs                # Data structures (Pool, Opportunity, etc.)
├── dex/                     # DEX integrations
│   ├── mod.rs
│   ├── orca.rs              # Orca Whirlpools integration
│   ├── raydium.rs           # Raydium AMM integration
│   └── phoenix.rs           # Phoenix orderbook integration
├── engine/
│   ├── screener.rs          # Arbitrage opportunity detection
│   └── executor.rs          # Transaction building and execution
├── monitor/
│   ├── mempool.rs           # Real-time transaction monitoring
│   └── whales.rs            # Large wallet activity tracking
└── utils/
    ├── math.rs              # AMM calculations and profit analysis
    └── rpc.rs               # Rate-limited Solana RPC client
```

### Key Dependencies
- **solana-client/sdk**: Solana blockchain interaction
- **anchor-client**: Solana program interaction framework
- **tokio-tungstenite**: WebSocket connections for real-time data
- **reqwest**: HTTP client for DEX APIs
- **rust_decimal**: Precise decimal arithmetic for financial calculations
- **governor**: Rate limiting for API requests
- **serde**: Serialization/deserialization
- **tracing**: Structured logging

## Strategy Implementation

### 1. Arbitrage Types
- **Direct Arbitrage**: A → B → A across different DEXs
- **Triangular Arbitrage**: A → B → C → A (limited to 1000 combinations for performance)
- **Cross-DEX Arbitrage**: Price differences between AMM and orderbook DEXs

### 2. Data Sources
- **Orca**: Whirlpool API + on-chain account data
- **Raydium**: Public SDK API + AMM pool states
- **Phoenix**: Market API + orderbook depth analysis

### 3. Monitoring Systems
- **Mempool Monitor**: WebSocket subscription to logsSubscribe for DEX program IDs
- **Whale Monitor**: accountSubscribe for configured whale wallet addresses
- **Pool Updates**: Continuous fetching of liquidity and reserve data

### 4. Execution Engine
- **Simulation First**: All transactions simulated before execution
- **Priority Fees**: Configurable priority fees for fast confirmation
- **Compute Budget**: Dynamic compute unit estimation
- **Multi-step Transactions**: Complex arbitrage routes in single atomic transaction

## Configuration System

### Environment Variables (.env)
- RPC endpoints (supports QuickNode for better performance)
- Private key for trading wallet
- Profit thresholds and risk parameters
- Execution flags (simulation vs live trading)

### Config File (config.toml)
- DEX enablement settings
- Risk management parameters
- Monitoring configuration
- Rate limiting settings

## Risk Management

### Built-in Protections
- **Simulation Mode**: Test strategies without real transactions
- **Profit Thresholds**: Minimum profit percentage requirements
- **Slippage Limits**: Maximum acceptable price impact
- **Liquidity Filters**: Minimum liquidity requirements
- **Position Sizing**: Maximum SOL per trade
- **Cooldown Periods**: Prevent overtrading
- **Rate Limiting**: API protection with configurable limits

### Monitoring & Alerting
- Consecutive failure tracking with exponential backoff
- Transaction confirmation monitoring
- Real-time profit/loss tracking
- Comprehensive logging with structured events

## Data Flow

1. **Initialization**: Load config, establish RPC connections, initialize DEX clients
2. **Pool Data Collection**: Fetch current pool states from all enabled DEXs
3. **Opportunity Screening**: Analyze price differences and calculate potential profits
4. **Risk Assessment**: Filter by liquidity, slippage, and confidence scores
5. **Mempool Analysis**: Monitor pending transactions for early signals
6. **Whale Tracking**: Track large wallet movements for market predictions
7. **Transaction Building**: Construct optimized swap instructions
8. **Simulation**: Validate transactions before execution
9. **Execution**: Send transactions with priority fees
10. **Confirmation**: Monitor transaction status and calculate actual profits

## Recent Changes

- **2025-01-16**: Built comprehensive Solana arbitrage bot from scratch based on conversation requirements
- **Architecture**: Implemented modular design with separate screener, executor, and monitoring components
- **DEX Integration**: Added support for Orca, Raydium, and Phoenix with API abstractions
- **Risk Management**: Implemented comprehensive risk controls and simulation capabilities
- **Configuration**: Created flexible config system supporting both .env and .toml files
- **Documentation**: Added comprehensive README with setup and usage instructions

## External Dependencies

### Blockchain Infrastructure
- **Solana RPC**: Primary blockchain interaction (supports custom endpoints)
- **DEX APIs**: Orca, Raydium, Phoenix public APIs for pool data
- **WebSocket Streams**: Real-time transaction and account monitoring

### Development Environment
- **Rust 1.85+**: Latest stable Rust compiler
- **Cargo**: Package management and build system
- **Replit**: Development and deployment environment

## Deployment Strategy

The bot is designed for continuous operation with:
- **Development**: `cargo run` for local testing
- **Production**: `cargo build --release` for optimized binary
- **Monitoring**: Structured logging with configurable levels
- **Configuration**: Environment-based settings for different deployment environments
- **Safety**: Simulation mode for testing before live deployment

## Performance Optimizations

- **Concurrent Operations**: Parallel DEX data fetching and monitoring
- **Memory Efficiency**: Rust's zero-cost abstractions and careful resource management
- **Rate Limiting**: Intelligent request throttling to avoid API limits
- **Connection Pooling**: Persistent WebSocket connections for real-time data
- **Caching**: Pool data caching with configurable refresh intervals