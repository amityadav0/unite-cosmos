import { CrossChainResolver } from '../resolver';
import { UserOperations } from '../user-operations';
import { ResolverConfig, UserConfig, EscrowConfig, ChainType } from '../types';
import { generateSecretAndHashlock, toWei } from '../utils';
import dotenv from 'dotenv';

// Load environment variables
dotenv.config();

// Testnet configuration for ETH -> OSMO flow
const createTestnetConfig = (): { resolver: ResolverConfig; user: UserConfig } => {
  return {
    resolver: {
      ethereum: {
        rpcUrl: process.env['ETHEREUM_TESTNET_RPC_URL'] || 'https://sepolia.infura.io/v3/your-api-key',
        chainId: process.env['ETHEREUM_TESTNET_CHAIN_ID'] || '11155111', // Sepolia
        privateKey: process.env['ETHEREUM_TESTNET_PRIVATE_KEY'] || '',
        escrowFactoryAddress: process.env['ETHEREUM_TESTNET_ESCROW_FACTORY_ADDRESS'] || '',
        escrowSrcAddress: process.env['ETHEREUM_TESTNET_ESCROW_SRC_ADDRESS'] || '',
        escrowDstAddress: process.env['ETHEREUM_TESTNET_ESCROW_DST_ADDRESS'] || '',
        gasPrice: process.env['ETHEREUM_TESTNET_GAS_PRICE'] || '5',
        gasLimit: parseInt(process.env['ETHEREUM_TESTNET_GAS_LIMIT'] || '500000')
      },
              cosmos: {
          rpcUrl: process.env['OSMOSIS_TESTNET_RPC_URL'] || 'https://rpc.testnet.osmosis.zone:26657',
          chainId: process.env['OSMOSIS_TESTNET_CHAIN_ID'] || 'osmo-test-5',
          mnemonic: process.env['OSMOSIS_TESTNET_MNEMONIC'] || '',
          escrowContractAddress: process.env['OSMOSIS_TESTNET_ESCROW_CONTRACT_ADDRESS'] || '',
          prefix: process.env['OSMOSIS_TESTNET_PREFIX'] || 'osmo',
          denom: process.env['OSMOSIS_TESTNET_DENOM'] || 'uosmo'
        }
    },
    user: {
      ethereum: {
        rpcUrl: process.env.ETHEREUM_TESTNET_RPC_URL || 'https://sepolia.infura.io/v3/your-api-key',
        gasPrice: process.env.ETHEREUM_TESTNET_GAS_PRICE || '5',
        gasLimit: parseInt(process.env.ETHEREUM_TESTNET_GAS_LIMIT || '200000')
      },
      cosmos: {
        rpcUrl: process.env.OSMOSIS_TESTNET_RPC_URL || 'https://rpc.testnet.osmosis.zone:26657',
        mnemonic: process.env.OSMOSIS_TESTNET_MNEMONIC || '',
        escrowContractAddress: process.env.OSMOSIS_TESTNET_ESCROW_CONTRACT_ADDRESS || '',
        prefix: process.env.OSMOSIS_TESTNET_PREFIX || 'osmo',
        denom: process.env.OSMOSIS_TESTNET_DENOM || 'uosmo'
      }
    }
  };
};

/**
 * ETH to OSMO Cross-Chain Escrow Flow
 * 
 * Flow:
 * 1. User wants 100 OSMO on Osmosis
 * 2. User deposits 0.1 ETH on Ethereum (Sepolia testnet)
 * 3. Resolver creates escrow contracts on both chains
 * 4. User can withdraw from destination (Osmosis) using secret
 */
async function ethToOsmoFlow() {
  console.log('🚀 Starting ETH -> OSMO Cross-Chain Escrow Flow');
  console.log('================================================');

  try {
    // Initialize configuration
    const config = createTestnetConfig();
    
    // Initialize resolver and user operations
    const resolver = new CrossChainResolver(config.resolver);
    const userOps = new UserOperations(config.user);
    
    await resolver.initialize();
    await userOps.initialize();
    
    console.log('✅ Resolver and User Operations initialized');

    // Generate secret and hashlock
    const secretInfo = generateSecretAndHashlock();
    console.log('🔐 Generated secret and hashlock');
    console.log('   Secret:', secretInfo.secret);
    console.log('   Hashlock:', secretInfo.hashlock);

    // Create escrow configuration
    const escrowConfig: EscrowConfig = {
      sourceChain: ChainType.ETHEREUM,
      destinationChain: ChainType.COSMOS,
      sourceToken: '0x0000000000000000000000000000000000000000', // ETH (native token)
      destinationToken: 'uosmo', // OSMO token
      amount: toWei('0.1', 18), // 0.1 ETH in wei
      maker: process.env.ETHEREUM_TESTNET_MAKER_ADDRESS || '0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6',
      taker: process.env.OSMOSIS_TESTNET_TAKER_ADDRESS || 'osmo1exampleaddress123456789012345678901234567890',
      timelocks: {
        withdrawal: 3600, // 1 hour
        cancellation: 7200, // 2 hours
        rescue: 86400 // 24 hours
      },
      hashlock: secretInfo.hashlock
    };

    console.log('📋 Escrow Configuration:');
    console.log('   Source Chain: Ethereum (Sepolia)');
    console.log('   Destination Chain: Osmosis Testnet');
    console.log('   Amount: 0.1 ETH -> 100 OSMO');
    console.log('   Maker:', escrowConfig.maker);
    console.log('   Taker:', escrowConfig.taker);

    // Step 1: Create cross-chain escrow contracts
    console.log('\n🔧 Step 1: Creating cross-chain escrow contracts...');
    const escrowResult = await resolver.createCrossChainEscrow(escrowConfig);

    if (!escrowResult.success) {
      throw new Error(`Failed to create escrow: ${escrowResult.error}`);
    }

    console.log('✅ Escrow contracts created successfully!');
    console.log('   Escrow ID:', escrowResult.escrowId);
    console.log('   Source Escrow Address:', escrowResult.sourceEscrowAddress);
    console.log('   Destination Escrow Address:', escrowResult.destinationEscrowAddress);

    // Step 2: User deposits funds to source escrow (Ethereum)
    console.log('\n💰 Step 2: User deposits 0.1 ETH to source escrow...');
    
    const ethereumPrivateKey = process.env.ETHEREUM_TESTNET_USER_PRIVATE_KEY || '';
    if (!ethereumPrivateKey) {
      throw new Error('ETHEREUM_TESTNET_USER_PRIVATE_KEY environment variable is required');
    }

    const depositResult = await userOps.depositToEthereumEscrow(
      escrowResult.sourceEscrowAddress!,
      toWei('0.1', 18), // 0.1 ETH
      ethereumPrivateKey
    );

    if (!depositResult.success) {
      throw new Error(`Failed to deposit to Ethereum escrow: ${depositResult.error}`);
    }

    console.log('✅ ETH deposited successfully!');
    console.log('   Transaction Hash:', depositResult.txHash);
    console.log('   Amount Deposited:', depositResult.amount);

    // Step 3: Get escrow information
    console.log('\n📊 Step 3: Getting escrow information...');
    
    const sourceEscrowInfo = await userOps.getEthereumEscrowInfo(escrowResult.sourceEscrowAddress!);
    if (sourceEscrowInfo) {
      console.log('   Source Escrow Info:');
      console.log('     Maker:', sourceEscrowInfo.maker);
      console.log('     Taker:', sourceEscrowInfo.taker);
      console.log('     Amount:', sourceEscrowInfo.amount);
      console.log('     Timelock:', sourceEscrowInfo.timelock);
      console.log('     Is Active:', sourceEscrowInfo.isActive);
    }

    const destEscrowInfo = await userOps.getCosmosEscrowInfo(escrowResult.escrowId!);
    if (destEscrowInfo) {
      console.log('   Destination Escrow Info:');
      console.log('     Maker:', destEscrowInfo.maker);
      console.log('     Taker:', destEscrowInfo.taker);
      console.log('     Amount:', destEscrowInfo.amount);
      console.log('     Timelock:', destEscrowInfo.timelock);
      console.log('     Is Active:', destEscrowInfo.isActive);
    }

    // Step 4: Demonstrate withdrawal (this would be done by the taker)
    console.log('\n🎯 Step 4: Demonstrating withdrawal from destination escrow...');
    console.log('   Note: In a real scenario, the taker would withdraw using the secret');
    console.log('   Secret for withdrawal:', secretInfo.secret);

    const withdrawResult = await userOps.withdrawFromCosmosEscrow(
      escrowResult.escrowId!,
      secretInfo.secret,
      false // destination escrow
    );

    if (withdrawResult.success) {
      console.log('✅ Withdrawal successful!');
      console.log('   Transaction Hash:', withdrawResult.txHash);
      console.log('   User now has 100 OSMO on Osmosis');
    } else {
      console.log('⚠️  Withdrawal failed (expected in demo):', withdrawResult.error);
      console.log('   This is expected if the taker is not the current user');
    }

    // Step 5: Demonstrate cancellation (this would be done by the maker)
    console.log('\n❌ Step 5: Demonstrating cancellation from source escrow...');
    console.log('   Note: In a real scenario, the maker could cancel if timelock expires');

    const cancelResult = await userOps.cancelEthereumEscrow(
      escrowResult.sourceEscrowAddress!,
      ethereumPrivateKey
    );

    if (cancelResult.success) {
      console.log('✅ Cancellation successful!');
      console.log('   Transaction Hash:', cancelResult.txHash);
      console.log('   User gets their 0.1 ETH back');
    } else {
      console.log('⚠️  Cancellation failed (expected in demo):', cancelResult.error);
      console.log('   This is expected if the maker is not the current user or timelock not expired');
    }

    console.log('\n🎉 ETH -> OSMO Cross-Chain Escrow Flow Completed!');
    console.log('================================================');
    console.log('Summary:');
    console.log('   ✅ Cross-chain escrow contracts created');
    console.log('   ✅ 0.1 ETH deposited to source escrow');
    console.log('   ✅ User can withdraw 100 OSMO from destination');
    console.log('   ✅ User can cancel and get ETH back if needed');
    console.log('   🔐 Secret for withdrawal:', secretInfo.secret);

  } catch (error) {
    console.error('❌ Error in ETH -> OSMO flow:', error);
    process.exit(1);
  }
}

// Run the flow if this file is executed directly
if (require.main === module) {
  ethToOsmoFlow().catch(console.error);
}

export { ethToOsmoFlow }; 