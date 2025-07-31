# Unite Cosmos Cross-Chain Resolver

A TypeScript resolver for creating cross-chain escrow contracts between Ethereum and Cosmos chains. This resolver enables atomic swaps and cross-chain transactions by deploying escrow contracts on both chains.

## Features

- **Cross-Chain Escrow Creation**: Deploy escrow contracts on both Ethereum and Cosmos chains
- **Bidirectional Support**: Support for both Ethereum → Cosmos and Cosmos → Ethereum flows
- **Secret Management**: Automatic generation and management of secrets and hashlocks
- **Timelock Support**: Configurable timelocks for withdrawal, cancellation, and rescue operations
- **Error Handling**: Comprehensive error handling with retry mechanisms
- **Type Safety**: Full TypeScript support with strict type checking

## Architecture

The resolver consists of several key components:

- **CrossChainResolver**: Main orchestrator class that manages cross-chain operations
- **EthereumClient**: Handles Ethereum-specific operations using ethers.js
- **CosmosClient**: Handles Cosmos-specific operations using CosmJS
- **Utils**: Utility functions for secret generation, validation, and other helpers

## Installation

```bash
cd resolver
npm install
```

## Configuration

Copy the example environment file and configure your settings:

```bash
cp env.example .env
```

Edit `.env` with your configuration:

```env
# Ethereum Configuration
ETHEREUM_RPC_URL=https://eth-mainnet.alchemyapi.io/v2/your-api-key
ETHEREUM_CHAIN_ID=1
ETHEREUM_PRIVATE_KEY=your-ethereum-private-key-here
ETHEREUM_ESCROW_FACTORY_ADDRESS=0x1234567890123456789012345678901234567890
ETHEREUM_ESCROW_SRC_ADDRESS=0x1234567890123456789012345678901234567890
ETHEREUM_ESCROW_DST_ADDRESS=0x1234567890123456789012345678901234567890
ETHEREUM_GAS_PRICE=20
ETHEREUM_GAS_LIMIT=500000

# Cosmos Configuration
COSMOS_RPC_URL=https://rpc.cosmos.network:26657
COSMOS_CHAIN_ID=cosmoshub-4
COSMOS_MNEMONIC=your cosmos mnemonic phrase here with twelve or twenty four words
COSMOS_ESCROW_CONTRACT_ADDRESS=cosmos1contractaddress123456789012345678901234567890
COSMOS_PREFIX=cosmos
COSMOS_DENOM=uatom
```

## Usage

### Basic Usage

```typescript
import { CrossChainResolver } from './src/resolver';
import { ResolverConfig, EscrowConfig, ChainType } from './src/types';
import { generateSecretAndHashlock } from './src/utils';

// Create configuration
const config: ResolverConfig = {
  ethereum: {
    rpcUrl: 'https://eth-mainnet.alchemyapi.io/v2/your-api-key',
    chainId: '1',
    privateKey: 'your-private-key',
    escrowFactoryAddress: '0x...',
    escrowSrcAddress: '0x...',
    escrowDstAddress: '0x...'
  },
  cosmos: {
    rpcUrl: 'https://rpc.cosmos.network:26657',
    chainId: 'cosmoshub-4',
    mnemonic: 'your mnemonic phrase',
    escrowContractAddress: 'cosmos1...',
    prefix: 'cosmos',
    denom: 'uatom'
  }
};

// Initialize resolver
const resolver = new CrossChainResolver(config);
await resolver.initialize();

// Generate secret and hashlock
const secretInfo = generateSecretAndHashlock();

// Create escrow configuration
const escrowConfig: EscrowConfig = {
  sourceChain: ChainType.ETHEREUM,
  destinationChain: ChainType.COSMOS,
  sourceToken: '0xA0b86a33E6441b8c4C8C1C1B0BcC9C1C1C1C1C1C1',
  destinationToken: 'uatom',
  amount: '1000000000000000000', // 1 token in wei
  maker: '0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6',
  taker: 'cosmos1exampleaddress123456789012345678901234567890',
  timelocks: {
    withdrawal: 3600, // 1 hour
    cancellation: 7200, // 2 hours
    rescue: 86400 // 24 hours
  },
  hashlock: secretInfo.hashlock
};

// Create cross-chain escrow
const result = await resolver.createCrossChainEscrow(escrowConfig);

if (result.success) {
  console.log('Escrow created:', result.escrowId);
  console.log('Source address:', result.sourceEscrowAddress);
  console.log('Destination address:', result.destinationEscrowAddress);
}
```

### CLI Usage

Run examples using the CLI:

```bash
# Run Ethereum -> Cosmos example
npm start example

# Run Cosmos -> Ethereum example
npm start cosmos-to-eth

# Show help
npm start help
```

## API Reference

### CrossChainResolver

Main class for managing cross-chain escrow operations.

#### Methods

- `initialize()`: Initialize the resolver
- `createCrossChainEscrow(config)`: Create cross-chain escrow
- `withdrawFromEscrow(escrowId, secret, isSource)`: Withdraw from escrow
- `cancelEscrow(escrowId, isSource)`: Cancel escrow
- `publicWithdrawFromEscrow(escrowId, isSource)`: Public withdraw
- `publicCancelEscrow(escrowId, isSource)`: Public cancel
- `getEscrowState(escrowId)`: Get escrow state
- `getAllEscrowStates()`: Get all escrow states

### EthereumClient

Handles Ethereum-specific operations.

#### Methods

- `getEscrowSrcAddress(...)`: Get source escrow address
- `createDstEscrow(...)`: Create destination escrow
- `withdrawFromEscrow(...)`: Withdraw from escrow
- `cancelEscrow(...)`: Cancel escrow
- `publicWithdrawFromEscrow(...)`: Public withdraw
- `publicCancelEscrow(...)`: Public cancel
- `rescueFundsFromEscrow(...)`: Rescue funds

### CosmosClient

Handles Cosmos-specific operations.

#### Methods

- `initialize()`: Initialize Cosmos client
- `createSrcEscrow(...)`: Create source escrow
- `createDstEscrow(...)`: Create destination escrow
- `withdrawFromEscrow(...)`: Withdraw from escrow
- `cancelEscrow(...)`: Cancel escrow
- `publicWithdrawFromEscrow(...)`: Public withdraw
- `publicCancelEscrow(...)`: Public cancel
- `rescueFundsFromEscrow(...)`: Rescue funds

## Escrow Flow

1. **Initialization**: Resolver is initialized with chain configurations
2. **Secret Generation**: A secret and hashlock are generated
3. **Source Escrow Creation**: Source escrow is created on the source chain
4. **Destination Escrow Creation**: Destination escrow is created on the destination chain
5. **Fund Transfer**: Funds are transferred to the escrow contracts
6. **Withdrawal**: Parties can withdraw using the secret
7. **Cancellation**: Parties can cancel if timelocks expire

## Security Considerations

- **Secret Management**: Secrets should be securely generated and stored
- **Private Keys**: Never expose private keys in code or logs
- **Timelocks**: Configure appropriate timelocks for your use case
- **Gas Limits**: Set appropriate gas limits for Ethereum transactions
- **Error Handling**: Always handle errors gracefully

## Development

### Building

```bash
npm run build
```

### Testing

```bash
npm test
```

### Linting

```bash
npm run lint
```

### Formatting

```bash
npm run format
```

## Dependencies

- **ethers**: Ethereum library for smart contract interaction
- **@cosmjs/cosmwasm-stargate**: Cosmos SDK client
- **@cosmjs/proto-signing**: Cosmos signing utilities
- **@cosmjs/stargate**: Cosmos Stargate client
- **@cosmjs/tendermint-rpc**: Tendermint RPC client

## License

MIT License

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## References

- [1inch Cross-Chain Resolver Example](https://github.com/1inch/cross-chain-resolver-example)
- [1inch Cross-Chain Swap](https://github.com/1inch/cross-chain-swap)
- [Ethers.js Documentation](https://docs.ethers.org/)
- [CosmJS Documentation](https://cosmos.github.io/cosmjs/) 