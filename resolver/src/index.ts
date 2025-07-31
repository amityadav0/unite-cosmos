import { CrossChainResolver } from './resolver';
import { ResolverConfig, EscrowConfig, ChainType } from './types';
import { generateSecretAndHashlock } from './utils';
import dotenv from 'dotenv';

// Load environment variables
dotenv.config();

/**
 * Example configuration for the resolver
 */
const createExampleConfig = (): ResolverConfig => {
  return {
    ethereum: {
      rpcUrl: process.env.ETHEREUM_RPC_URL || 'https://eth-mainnet.alchemyapi.io/v2/your-api-key',
      chainId: process.env.ETHEREUM_CHAIN_ID || '1',
      privateKey: process.env.ETHEREUM_PRIVATE_KEY || '',
      escrowFactoryAddress: process.env.ETHEREUM_ESCROW_FACTORY_ADDRESS || '',
      escrowSrcAddress: process.env.ETHEREUM_ESCROW_SRC_ADDRESS || '',
      escrowDstAddress: process.env.ETHEREUM_ESCROW_DST_ADDRESS || '',
      gasPrice: process.env.ETHEREUM_GAS_PRICE || '20',
      gasLimit: parseInt(process.env.ETHEREUM_GAS_LIMIT || '500000')
    },
    cosmos: {
      rpcUrl: process.env.COSMOS_RPC_URL || 'https://rpc.cosmos.network:26657',
      chainId: process.env.COSMOS_CHAIN_ID || 'cosmoshub-4',
      mnemonic: process.env.COSMOS_MNEMONIC || '',
      escrowContractAddress: process.env.COSMOS_ESCROW_CONTRACT_ADDRESS || '',
      prefix: process.env.COSMOS_PREFIX || 'cosmos',
      denom: process.env.COSMOS_DENOM || 'uatom'
    }
  };
};

/**
 * Example usage of the cross-chain resolver
 */
async function exampleUsage() {
  try {
    // Create resolver configuration
    const config = createExampleConfig();
    
    // Initialize resolver
    const resolver = new CrossChainResolver(config);
    await resolver.initialize();

    console.log('Resolver initialized successfully');

    // Generate secret and hashlock
    const secretInfo = generateSecretAndHashlock();
    console.log('Generated secret:', secretInfo.secret);
    console.log('Generated hashlock:', secretInfo.hashlock);

    // Example escrow configuration (Ethereum -> Cosmos)
    const escrowConfig: EscrowConfig = {
      sourceChain: ChainType.ETHEREUM,
      destinationChain: ChainType.COSMOS,
      sourceToken: '0xA0b86a33E6441b8c4C8C1C1B0BcC9C1C1C1C1C1C1', // Example ERC20 token
      destinationToken: 'uatom', // Cosmos ATOM
      amount: '1000000000000000000', // 1 token (in wei)
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
    console.log('Creating cross-chain escrow...');
    const result = await resolver.createCrossChainEscrow(escrowConfig);

    if (result.success) {
      console.log('Escrow created successfully!');
      console.log('Escrow ID:', result.escrowId);
      console.log('Source Escrow Address:', result.sourceEscrowAddress);
      console.log('Destination Escrow Address:', result.destinationEscrowAddress);
      console.log('Transaction Hash:', result.txHash);

      // Example: Withdraw from escrow
      console.log('\nWithdrawing from escrow...');
      const withdrawResult = await resolver.withdrawFromEscrow(
        result.escrowId!,
        secretInfo.secret,
        true // source escrow
      );

      if (withdrawResult.success) {
        console.log('Withdrawal successful!');
        console.log('Transaction Hash:', withdrawResult.txHash);
      } else {
        console.log('Withdrawal failed:', withdrawResult.error);
      }

      // Example: Cancel escrow
      console.log('\nCancelling escrow...');
      const cancelResult = await resolver.cancelEscrow(
        result.escrowId!,
        true // source escrow
      );

      if (cancelResult.success) {
        console.log('Cancellation successful!');
        console.log('Transaction Hash:', cancelResult.txHash);
      } else {
        console.log('Cancellation failed:', cancelResult.error);
      }

      // Get escrow state
      const escrowState = resolver.getEscrowState(result.escrowId!);
      console.log('\nEscrow State:', escrowState);

    } else {
      console.log('Failed to create escrow:', result.error);
    }

  } catch (error) {
    console.error('Error in example usage:', error);
  }
}

/**
 * Example usage for Cosmos -> Ethereum escrow
 */
async function exampleCosmosToEthereum() {
  try {
    const config = createExampleConfig();
    const resolver = new CrossChainResolver(config);
    await resolver.initialize();

    const secretInfo = generateSecretAndHashlock();

    const escrowConfig: EscrowConfig = {
      sourceChain: ChainType.COSMOS,
      destinationChain: ChainType.ETHEREUM,
      sourceToken: 'uatom', // Cosmos ATOM
      destinationToken: '0xA0b86a33E6441b8c4C8C1C1B0BcC9C1C1C1C1C1C1', // Example ERC20 token
      amount: '1000000', // 1 ATOM (in micro units)
      maker: 'cosmos1exampleaddress123456789012345678901234567890',
      taker: '0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6',
      timelocks: {
        withdrawal: 3600,
        cancellation: 7200,
        rescue: 86400
      },
      hashlock: secretInfo.hashlock
    };

    console.log('Creating Cosmos -> Ethereum escrow...');
    const result = await resolver.createCrossChainEscrow(escrowConfig);

    if (result.success) {
      console.log('Cosmos -> Ethereum escrow created successfully!');
      console.log('Escrow ID:', result.escrowId);
      console.log('Source Escrow Address:', result.sourceEscrowAddress);
      console.log('Destination Escrow Address:', result.destinationEscrowAddress);
    } else {
      console.log('Failed to create escrow:', result.error);
    }

  } catch (error) {
    console.error('Error in Cosmos -> Ethereum example:', error);
  }
}

/**
 * CLI interface for the resolver
 */
async function cli() {
  const args = process.argv.slice(2);
  const command = args[0];

  switch (command) {
    case 'example':
      await exampleUsage();
      break;
    case 'cosmos-to-eth':
      await exampleCosmosToEthereum();
      break;
    case 'help':
      console.log(`
Usage: npm start <command>

Commands:
  example        Run Ethereum -> Cosmos example
  cosmos-to-eth  Run Cosmos -> Ethereum example
  help           Show this help message

Environment Variables:
  ETHEREUM_RPC_URL                    Ethereum RPC URL
  ETHEREUM_CHAIN_ID                   Ethereum Chain ID
  ETHEREUM_PRIVATE_KEY                Ethereum Private Key
  ETHEREUM_ESCROW_FACTORY_ADDRESS     Ethereum Escrow Factory Address
  ETHEREUM_ESCROW_SRC_ADDRESS         Ethereum Escrow Source Address
  ETHEREUM_ESCROW_DST_ADDRESS         Ethereum Escrow Destination Address
  ETHEREUM_GAS_PRICE                  Ethereum Gas Price (in gwei)
  ETHEREUM_GAS_LIMIT                  Ethereum Gas Limit
  COSMOS_RPC_URL                      Cosmos RPC URL
  COSMOS_CHAIN_ID                     Cosmos Chain ID
  COSMOS_MNEMONIC                     Cosmos Mnemonic
  COSMOS_ESCROW_CONTRACT_ADDRESS      Cosmos Escrow Contract Address
  COSMOS_PREFIX                       Cosmos Address Prefix
  COSMOS_DENOM                        Cosmos Denomination
      `);
      break;
    default:
      console.log('Unknown command. Use "npm start help" for usage information.');
      break;
  }
}

// Export the main classes and types
export { CrossChainResolver } from './resolver';
export { EthereumClient } from './ethereum-client';
export { CosmosClient } from './cosmos-client';
export * from './types';
export * from './utils';

// Run CLI if this file is executed directly
if (require.main === module) {
  cli().catch(console.error);
} 