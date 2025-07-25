# Cross-Chain Escrow for Cosmos

This directory contains the Cosmos implementation of the cross-chain escrow system, adapted from the Ethereum/Solidity version to work with Cosmos SDK and CosmWasm smart contracts.

## Architecture Overview

The Cosmos implementation maintains the same core concepts as the Ethereum version but adapts them to Cosmos's architecture:

### Key Differences from Ethereum Version

1. **Smart Contract Platform**: Uses CosmWasm (Rust) instead of Solidity
2. **Token Standards**: CW20 (CosmWasm tokens) and native tokens
3. **Address Format**: Bech32 addresses instead of hex addresses

### Core Components

- **Escrow Contract**: Main escrow logic implemented in Rust
- **Factory Contract**: Creates and manages escrow instances
- **Query Handlers**: Provide read access to escrow state
- **Execute Handlers**: Handle state-changing operations

## Setup Instructions

### Prerequisites

1. Install Rust and Cargo
2. Install `wasm-opt` for optimization
3. Install `cargo-generate` for project templates

### Building

```bash
cd cosmos/escrow-contract
cargo build --release
```

### Testing

```bash
cargo test
```

### Deployment

```bash
# Deploy to local testnet
wasmd tx wasm store target/wasm32-unknown-unknown/release/escrow.wasm --from wallet --gas auto --gas-adjustment 1.3 -y
```

## Contract Structure

- `src/state.rs` - Data structures and storage
- `src/msg.rs` - Message types for execute and query
- `src/contract.rs` - Main contract logic
- `src/execute.rs` - Execute handlers
- `src/query.rs` - Query handlers
- `src/ibc.rs` - IBC handlers for cross-chain communication
- `src/error.rs` - Custom error types
- `src/lib.rs` - Contract entry points

## Security Features

- Deterministic address generation using Create2 equivalent
- Time-based security windows
- Secret-based unlocking mechanism
- Access control with native tokens
- Fund rescue functionality
- IBC-based cross-chain verification
