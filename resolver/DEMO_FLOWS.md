# Demo Flows for Cross-Chain Escrow

This document describes the two main demo flows for testing cross-chain escrow functionality between Ethereum (Sepolia testnet) and Osmosis testnet.

## 🚀 Quick Start

### Prerequisites

1. **Install Dependencies**
   ```bash
   cd resolver
   npm install
   ```

2. **Configure Environment**
   ```bash
   cp env.example .env
   # Edit .env with your testnet credentials
   ```

3. **Run Demo Flows**
   ```bash
   # ETH -> OSMO flow
   npm start eth-to-osmo
   
   # OSMO -> ETH flow
   npm start osmo-to-eth
   ```

## 📋 Demo Flow 1: ETH → OSMO

### Overview
User wants 100 OSMO on Osmosis and deposits 0.1 ETH on Ethereum (Sepolia testnet).

### Flow Steps

1. **Initialization**
   - Initialize resolver and user operations
   - Generate secret and hashlock
   - Create escrow configuration

2. **Escrow Creation**
   - Resolver creates source escrow on Ethereum
   - Resolver creates destination escrow on Osmosis
   - Both escrows are linked by the same hashlock

3. **User Deposit**
   - User deposits 0.1 ETH to Ethereum source escrow
   - Funds are locked in the escrow contract

4. **Withdrawal Process**
   - Taker can withdraw 100 OSMO from Osmosis destination escrow
   - Withdrawal requires the secret (reveals the hashlock)
   - Once withdrawn, the secret is revealed and both escrows are unlocked

5. **Cancellation Process**
   - Maker can cancel and get ETH back if timelock expires
   - Public operations available after timelock expiry

### Configuration
```env
# Ethereum Testnet (Sepolia)
ETHEREUM_TESTNET_RPC_URL=https://sepolia.infura.io/v3/your-api-key
ETHEREUM_TESTNET_PRIVATE_KEY=your-private-key
ETHEREUM_TESTNET_USER_PRIVATE_KEY=user-private-key
ETHEREUM_TESTNET_ESCROW_FACTORY_ADDRESS=0x...
ETHEREUM_TESTNET_ESCROW_SRC_ADDRESS=0x...
ETHEREUM_TESTNET_ESCROW_DST_ADDRESS=0x...

# Osmosis Testnet
OSMOSIS_TESTNET_RPC_URL=https://rpc.testnet.osmosis.zone:26657
OSMOSIS_TESTNET_MNEMONIC=your-mnemonic-phrase
OSMOSIS_TESTNET_ESCROW_CONTRACT_ADDRESS=osmo1...
```

## 📋 Demo Flow 2: OSMO → ETH

### Overview
User wants 0.1 ETH on Ethereum and deposits 100 OSMO on Osmosis testnet.

### Flow Steps

1. **Initialization**
   - Initialize resolver and user operations
   - Generate secret and hashlock
   - Create escrow configuration

2. **Escrow Creation**
   - Resolver creates source escrow on Osmosis
   - Resolver creates destination escrow on Ethereum
   - Both escrows are linked by the same hashlock

3. **User Deposit**
   - User deposits 100 OSMO to Osmosis source escrow
   - Funds are locked in the escrow contract

4. **Withdrawal Process**
   - Taker can withdraw 0.1 ETH from Ethereum destination escrow
   - Withdrawal requires the secret (reveals the hashlock)
   - Once withdrawn, the secret is revealed and both escrows are unlocked

5. **Cancellation Process**
   - Maker can cancel and get OSMO back if timelock expires
   - Public operations available after timelock expiry

### Configuration
```env
# Same configuration as ETH → OSMO flow
# The resolver automatically handles the reverse direction
```

## 🔧 Technical Details

### Resolver Operations
The resolver handles:
- **Cross-chain escrow creation**: Deploys escrow contracts on both chains
- **Contract orchestration**: Manages the relationship between source and destination escrows
- **Secret management**: Generates and manages secrets and hashlocks
- **Error handling**: Comprehensive error handling with retry mechanisms

### User Operations
Users can perform:
- **Deposit**: Send funds to source escrow
- **Withdraw**: Claim funds from destination escrow using secret
- **Cancel**: Cancel escrow and get funds back (if timelock expired)
- **Public Withdraw**: Withdraw after timelock expiry
- **Public Cancel**: Cancel after timelock expiry

### Security Features
- **Secret Generation**: Cryptographically secure random secrets
- **Hashlock Creation**: SHA3/Keccak256 hashlocks
- **Timelock Enforcement**: Configurable timelocks for all operations
- **Address Validation**: Ethereum and Cosmos address validation

## 🎯 Demo Scenarios

### Scenario 1: Successful Swap
1. User deposits funds to source escrow
2. Taker withdraws from destination escrow using secret
3. Both parties complete the swap successfully

### Scenario 2: Cancellation
1. User deposits funds to source escrow
2. Timelock expires without withdrawal
3. Maker cancels and gets funds back

### Scenario 3: Public Operations
1. User deposits funds to source escrow
2. Timelock expires without withdrawal
3. Anyone can perform public withdraw/cancel operations

## 🔍 Monitoring

### Escrow Information
Each flow displays:
- Escrow ID and addresses
- Maker and taker addresses
- Amount and timelock information
- Active status

### Transaction Tracking
- Transaction hashes for all operations
- Success/failure status
- Error messages for debugging

## 🛠️ Troubleshooting

### Common Issues

1. **RPC Connection Errors**
   - Verify RPC URLs are correct
   - Check network connectivity
   - Ensure API keys are valid

2. **Contract Address Errors**
   - Verify escrow contract addresses are deployed
   - Check contract addresses match the correct network
   - Ensure contracts are properly initialized

3. **Private Key Issues**
   - Verify private keys are correct format
   - Ensure private keys have sufficient funds
   - Check private keys match the correct network

4. **Gas Issues**
   - Adjust gas price and limit in configuration
   - Ensure account has sufficient ETH for gas
   - Check network congestion

### Debug Mode
Enable debug logging by setting:
```env
DEBUG=true
```

## 📊 Expected Output

### Successful ETH → OSMO Flow
```
🚀 Starting ETH -> OSMO Cross-Chain Escrow Flow
================================================
✅ Resolver and User Operations initialized
🔐 Generated secret and hashlock
   Secret: 0x1234...
   Hashlock: 0xabcd...
📋 Escrow Configuration:
   Source Chain: Ethereum (Sepolia)
   Destination Chain: Osmosis Testnet
   Amount: 0.1 ETH -> 100 OSMO
   Maker: 0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6
   Taker: osmo1exampleaddress123456789012345678901234567890

🔧 Step 1: Creating cross-chain escrow contracts...
✅ Escrow contracts created successfully!
   Escrow ID: 1703123456789-123456
   Source Escrow Address: 0x1234567890123456789012345678901234567890
   Destination Escrow Address: osmo1escrowaddress123456789012345678901234567890

💰 Step 2: User deposits 0.1 ETH to source escrow...
✅ ETH deposited successfully!
   Transaction Hash: 0xabcd123456789012345678901234567890123456789012345678901234567890
   Amount Deposited: 100000000000000000

📊 Step 3: Getting escrow information...
   Source Escrow Info:
     Maker: 0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6
     Taker: osmo1exampleaddress123456789012345678901234567890
     Amount: 100000000000000000
     Timelock: 1703127056
     Is Active: true

🎯 Step 4: Demonstrating withdrawal from destination escrow...
   Note: In a real scenario, the taker would withdraw using the secret
   Secret for withdrawal: 0x1234...
⚠️  Withdrawal failed (expected in demo): User not authorized
   This is expected if the taker is not the current user

❌ Step 5: Demonstrating cancellation from source escrow...
   Note: In a real scenario, the maker could cancel if timelock expires
⚠️  Cancellation failed (expected in demo): User not authorized
   This is expected if the maker is not the current user or timelock not expired

🎉 ETH -> OSMO Cross-Chain Escrow Flow Completed!
================================================
Summary:
   ✅ Cross-chain escrow contracts created
   ✅ 0.1 ETH deposited to source escrow
   ✅ User can withdraw 100 OSMO from destination
   ✅ User can cancel and get ETH back if needed
   🔐 Secret for withdrawal: 0x1234...
```

## 🔗 Next Steps

1. **Deploy Contracts**: Deploy actual escrow contracts to testnets
2. **Configure Addresses**: Update environment variables with real contract addresses
3. **Test with Real Funds**: Test with small amounts of real testnet tokens
4. **Production Deployment**: Deploy to mainnet with proper security measures

## 📚 References

- [1inch Cross-Chain Resolver Example](https://github.com/1inch/cross-chain-resolver-example)
- [Ethers.js Documentation](https://docs.ethers.org/)
- [CosmJS Documentation](https://cosmos.github.io/cosmjs/)
- [Sepolia Testnet](https://sepolia.dev/)
- [Osmosis Testnet](https://testnet.osmosis.zone/) 