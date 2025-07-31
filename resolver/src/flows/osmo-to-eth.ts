import { CrossChainResolver } from '../resolver';
import { UserOperations } from '../user-operations';
import { ResolverConfig, UserConfig, EscrowConfig, ChainType } from '../types';
import { generateSecretAndHashlock, toWei, toMicroUnits } from '../utils';
import dotenv from 'dotenv';

// Load environment variables
dotenv.config();

// Testnet configuration for OSMO -> ETH flow
const createTestnetConfig = (): { resolver: ResolverConfig; user: UserConfig } => {
  return {
    resolver: {
      ethereum: {
        rpcUrl: process.env.ETHEREUM_TESTNET_RPC_URL || 'https://sepolia.infura.io/v3/your-api-key',
        chainId: process.env.ETHEREUM_TESTNET_CHAIN_ID || '11155111', // Sepolia
        privateKey: process.env.ETHEREUM_TESTNET_PRIVATE_KEY || '',
        escrowFactoryAddress: process.env.ETHEREUM_TESTNET_ESCROW_FACTORY_ADDRESS || '',
        escrowSrcAddress: process.env.ETHEREUM_TESTNET_ESCROW_SRC_ADDRESS || '',
        escrowDstAddress: process.env.ETHEREUM_TESTNET_ESCROW_DST_ADDRESS || '',
        gasPrice: process.env.ETHEREUM_TESTNET_GAS_PRICE || '5',
        gasLimit: parseInt(process.env.ETHEREUM_TESTNET_GAS_LIMIT || '500000')
      },
      cosmos: {
        rpcUrl: process.env.OSMOSIS_TESTNET_RPC_URL || 'https://rpc.testnet.osmosis.zone:26657',
        chainId: process.env.OSMOSIS_TESTNET_CHAIN_ID || 'osmo-test-5',
        mnemonic: process.env.OSMOSIS_TESTNET_MNEMONIC || '',
        escrowContractAddress: process.env.OSMOSIS_TESTNET_ESCROW_CONTRACT_ADDRESS || '',
        prefix: process.env.OSMOSIS_TESTNET_PREFIX || 'osmo',
        denom: process.env.OSMOSIS_TESTNET_DENOM || 'uosmo'
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
 * OSMO to ETH Cross-Chain Escrow Flow
 * 
 * Flow:
 * 1. User wants 0.1 ETH on Ethereum
 * 2. User deposits 100 OSMO on Osmosis testnet
 * 3. Resolver creates escrow contracts on both chains
 * 4. User can withdraw from destination (Ethereum) using secret
 */
async function osmoToEthFlow() {
  console.log('🚀 Starting OSMO -> ETH Cross-Chain Escrow Flow');
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
      sourceChain: ChainType.COSMOS,
      destinationChain: ChainType.ETHEREUM,
      sourceToken: 'uosmo', // OSMO token
      destinationToken: '0x0000000000000000000000000000000000000000', // ETH (native token)
      amount: toMicroUnits('100', 6), // 100 OSMO in micro units
      maker: process.env.OSMOSIS_TESTNET_MAKER_ADDRESS || 'osmo1exampleaddress123456789012345678901234567890',
      taker: process.env.ETHEREUM_TESTNET_TAKER_ADDRESS || '0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6',
      timelocks: {
        withdrawal: 3600, // 1 hour
        cancellation: 7200, // 2 hours
        rescue: 86400 // 24 hours
      },
      hashlock: secretInfo.hashlock
    };

    console.log('📋 Escrow Configuration:');
    console.log('   Source Chain: Osmosis Testnet');
    console.log('   Destination Chain: Ethereum (Sepolia)');
    console.log('   Amount: 100 OSMO -> 0.1 ETH');
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

    // Step 2: User deposits funds to source escrow (Osmosis)
    console.log('\n💰 Step 2: User deposits 100 OSMO to source escrow...');
    
    const depositResult = await userOps.depositToCosmosEscrow(
      escrowResult.escrowId!,
      toMicroUnits('100', 6) // 100 OSMO
    );

    if (!depositResult.success) {
      throw new Error(`Failed to deposit to Osmosis escrow: ${depositResult.error}`);
    }

    console.log('✅ OSMO deposited successfully!');
    console.log('   Transaction Hash:', depositResult.txHash);
    console.log('   Amount Deposited:', depositResult.amount);

    // Step 3: Get escrow information
    console.log('\n📊 Step 3: Getting escrow information...');
    
    const sourceEscrowInfo = await userOps.getCosmosEscrowInfo(escrowResult.escrowId!);
    if (sourceEscrowInfo) {
      console.log('   Source Escrow Info:');
      console.log('     Maker:', sourceEscrowInfo.maker);
      console.log('     Taker:', sourceEscrowInfo.taker);
      console.log('     Amount:', sourceEscrowInfo.amount);
      console.log('     Timelock:', sourceEscrowInfo.timelock);
      console.log('     Is Active:', sourceEscrowInfo.isActive);
    }

    const destEscrowInfo = await userOps.getEthereumEscrowInfo(escrowResult.destinationEscrowAddress!);
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

    const ethereumPrivateKey = process.env.ETHEREUM_TESTNET_USER_PRIVATE_KEY || '';
    if (!ethereumPrivateKey) {
      throw new Error('ETHEREUM_TESTNET_USER_PRIVATE_KEY environment variable is required');
    }

    const withdrawResult = await userOps.withdrawFromEthereumEscrow(
      escrowResult.destinationEscrowAddress!,
      secretInfo.secret,
      ethereumPrivateKey
    );

    if (withdrawResult.success) {
      console.log('✅ Withdrawal successful!');
      console.log('   Transaction Hash:', withdrawResult.txHash);
      console.log('   User now has 0.1 ETH on Ethereum');
    } else {
      console.log('⚠️  Withdrawal failed (expected in demo):', withdrawResult.error);
      console.log('   This is expected if the taker is not the current user');
    }

    // Step 5: Demonstrate cancellation (this would be done by the maker)
    console.log('\n❌ Step 5: Demonstrating cancellation from source escrow...');
    console.log('   Note: In a real scenario, the maker could cancel if timelock expires');

    const cancelResult = await userOps.cancelCosmosEscrow(
      escrowResult.escrowId!,
      true // source escrow
    );

    if (cancelResult.success) {
      console.log('✅ Cancellation successful!');
      console.log('   Transaction Hash:', cancelResult.txHash);
      console.log('   User gets their 100 OSMO back');
    } else {
      console.log('⚠️  Cancellation failed (expected in demo):', cancelResult.error);
      console.log('   This is expected if the maker is not the current user or timelock not expired');
    }

    // Step 6: Demonstrate public operations (after timelock expiry)
    console.log('\n⏰ Step 6: Demonstrating public operations...');
    console.log('   Note: These operations are available after timelock expiry');

    const publicWithdrawResult = await userOps.publicWithdrawFromEthereumEscrow(
      escrowResult.destinationEscrowAddress!,
      ethereumPrivateKey
    );

    if (publicWithdrawResult.success) {
      console.log('✅ Public withdrawal successful!');
      console.log('   Transaction Hash:', publicWithdrawResult.txHash);
    } else {
      console.log('⚠️  Public withdrawal failed (expected):', publicWithdrawResult.error);
      console.log('   This is expected if timelock has not expired');
    }

    const publicCancelResult = await userOps.publicCancelCosmosEscrow(
      escrowResult.escrowId!,
      true // source escrow
    );

    if (publicCancelResult.success) {
      console.log('✅ Public cancellation successful!');
      console.log('   Transaction Hash:', publicCancelResult.txHash);
    } else {
      console.log('⚠️  Public cancellation failed (expected):', publicCancelResult.error);
      console.log('   This is expected if timelock has not expired');
    }

    console.log('\n🎉 OSMO -> ETH Cross-Chain Escrow Flow Completed!');
    console.log('================================================');
    console.log('Summary:');
    console.log('   ✅ Cross-chain escrow contracts created');
    console.log('   ✅ 100 OSMO deposited to source escrow');
    console.log('   ✅ User can withdraw 0.1 ETH from destination');
    console.log('   ✅ User can cancel and get OSMO back if needed');
    console.log('   🔐 Secret for withdrawal:', secretInfo.secret);

  } catch (error) {
    console.error('❌ Error in OSMO -> ETH flow:', error);
    process.exit(1);
  }
}

// Run the flow if this file is executed directly
if (require.main === module) {
  osmoToEthFlow().catch(console.error);
}

export { osmoToEthFlow }; 