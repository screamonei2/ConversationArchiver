# Bitquery Integration

This document describes the integration of Bitquery's GraphQL API into the Solana arbitrage bot.

## Overview

The Bitquery integration allows the bot to fetch real-time DEX data from multiple Solana DEXs through a unified GraphQL API. This provides an alternative data source to direct DEX API calls and can potentially offer more comprehensive market data.

## Features

- **Unified Data Source**: Access data from multiple DEXs (Raydium, Orca, Phoenix) through a single API
- **Real-time Updates**: WebSocket support for live market data
- **Mock Data Fallback**: Automatic fallback to mock data when API is unavailable
- **Pool Caching**: Efficient caching mechanism to reduce API calls
- **Error Handling**: Robust error handling with retry logic

## Configuration

### Environment Variables

Add your Bitquery API key to the `.env` file:

```env
BITQUERY_API_KEY=your_bitquery_api_key_here
```

### Config File

Enable Bitquery in `config.toml`:

```toml
[dexs]
enabled = ["orca", "raydium", "phoenix", "bitquery"]
```

## API Key Setup

1. Visit [Bitquery.io](https://bitquery.io)
2. Create an account or log in
3. Navigate to your account dashboard
4. Generate a new API key
5. Add the key to your `.env` file

## Implementation Details

### GraphQL Queries

The integration uses GraphQL queries to fetch:
- DEX trade data
- Pool reserves and liquidity
- Token information and prices
- Market statistics

### Data Structures

Key data structures include:
- `BitqueryClient`: Main client implementation
- `BitqueryResponse`: GraphQL response wrapper
- `DexTradeData`: Trade and pool information
- `CurrencyData`: Token metadata

### Error Handling

The client handles various error scenarios:
- Network connectivity issues
- API rate limiting
- Invalid API keys
- Malformed responses

## Usage

The Bitquery client is automatically initialized when enabled in the configuration. It implements the same `DexClient` trait as other DEX clients, making it a drop-in replacement.

```rust
// The client is automatically created in main.rs
let bitquery_client = Arc::new(BitqueryClient::new(
    rpc_client.clone(),
    console_manager.clone(),
).await?);
```

## Testing

The integration includes comprehensive testing:
- Unit tests for data parsing
- Mock data generation for testing
- Integration with existing test suites

## Limitations

- Requires valid Bitquery API key
- Subject to API rate limits
- May have slight latency compared to direct DEX APIs
- Dependent on Bitquery's data accuracy and availability

## Troubleshooting

### Common Issues

1. **Invalid API Key**: Ensure your API key is correctly set in `.env`
2. **Rate Limiting**: Implement appropriate delays between requests
3. **Network Issues**: Check internet connectivity and firewall settings
4. **Data Inconsistencies**: Verify Bitquery's data against direct DEX APIs

### Debug Mode

Enable debug logging to troubleshoot issues:

```bash
RUST_LOG=debug cargo run
```

## Future Enhancements

- WebSocket streaming for real-time updates
- Advanced filtering and querying capabilities
- Historical data analysis
- Performance optimizations
- Additional DEX support