import { ethers } from 'ethers';
import { 
  SigningCosmWasmClient 
} from '@cosmjs/cosmwasm-stargate';
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing';
import { 
  UserConfig, 
  DepositResult, 
  WithdrawResult, 
  CancelResult,
  EscrowInfo
} from './types';
import { 
  formatError, 
  isValidEthereumAddress
} from './utils';

// ABI for user operations on Ethereum escrow contracts
const USER_ESCROW_ABI = [
  'function deposit() payable',
  'function withdraw(string memory secret)',
  'function cancel()',
  'function publicWithdraw()',
  'function publicCancel()',
  'function getEscrowInfo() view returns (address maker, address taker, uint256 amount, uint256 timelock, bool isActive)',
  'event Deposited(address indexed sender, uint256 amount)',
  'event Withdrawn(address indexed recipient, uint256 amount)',
  'event Cancelled(address indexed recipient, uint256 amount)'
];

export class UserOperations {
  private ethereumProvider: ethers.JsonRpcProvider;
  private cosmosClient!: SigningCosmWasmClient;
  private cosmosWallet!: DirectSecp256k1HdWallet;
  private config: UserConfig;

  constructor(config: UserConfig) {
    this.config = config;
    this.ethereumProvider = new ethers.JsonRpcProvider(config.ethereum.rpcUrl);
  }

  /**
   * Initialize the user operations
   */
  async initialize(): Promise<void> {
    try {
      // Initialize Cosmos wallet and client
      this.cosmosWallet = await DirectSecp256k1HdWallet.fromMnemonic(
        this.config.cosmos.mnemonic,
        { prefix: this.config.cosmos.prefix }
      );

      this.cosmosClient = await SigningCosmWasmClient.connectWithSigner(
        this.config.cosmos.rpcUrl,
        this.cosmosWallet
      );

      console.log('User operations initialized successfully');
    } catch (error) {
      throw new Error(`Failed to initialize user operations: ${formatError(error)}`);
    }
  }

  // ==================== DEPOSIT OPERATIONS ====================

  /**
   * Deposit funds to Ethereum source escrow
   */
  async depositToEthereumEscrow(
    escrowAddress: string,
    amount: string,
    privateKey: string
  ): Promise<DepositResult> {
    try {
      if (!isValidEthereumAddress(escrowAddress)) {
        return {
          success: false,
          error: 'Invalid Ethereum escrow address'
        };
      }

      const wallet = new ethers.Wallet(privateKey, this.ethereumProvider);
      const escrowContract = new ethers.Contract(escrowAddress, USER_ESCROW_ABI, wallet);

      const tx = await escrowContract.deposit({
        value: amount,
        gasLimit: this.config.ethereum.gasLimit || 200000,
        gasPrice: this.config.ethereum.gasPrice ? 
          ethers.parseUnits(this.config.ethereum.gasPrice, 'gwei') : undefined
      });

      const receipt = await tx.wait();

      return {
        success: true,
        txHash: receipt.hash,
        amount: amount
      };
    } catch (error) {
      return {
        success: false,
        error: `Failed to deposit to Ethereum escrow: ${formatError(error)}`
      };
    }
  }

  /**
   * Deposit funds to Cosmos source escrow
   */
  async depositToCosmosEscrow(
    escrowId: string,
    amount: string
  ): Promise<DepositResult> {
    try {
      const accounts = await this.cosmosWallet.getAccounts();
      if (!accounts[0]) {
        throw new Error('No accounts found in wallet');
      }
      const sender = accounts[0].address;

      const msg = {
        deposit_src: {
          escrow_id: escrowId
        }
      };

      const result = await this.cosmosClient.execute(
        sender,
        this.config.cosmos.escrowContractAddress,
        msg,
        'auto',
        undefined,
        [{ amount, denom: this.config.cosmos.denom }]
      );

      return {
        success: true,
        txHash: result.transactionHash,
        amount: amount
      };
    } catch (error) {
      return {
        success: false,
        error: `Failed to deposit to Cosmos escrow: ${formatError(error)}`
      };
    }
  }

  // ==================== WITHDRAW OPERATIONS ====================

  /**
   * Withdraw from Ethereum escrow using secret
   */
  async withdrawFromEthereumEscrow(
    escrowAddress: string,
    secret: string,
    privateKey: string
  ): Promise<WithdrawResult> {
    try {
      if (!isValidEthereumAddress(escrowAddress)) {
        return {
          success: false,
          error: 'Invalid Ethereum escrow address'
        };
      }

      const wallet = new ethers.Wallet(privateKey, this.ethereumProvider);
      const escrowContract = new ethers.Contract(escrowAddress, USER_ESCROW_ABI, wallet);

      const tx = await escrowContract.withdraw(secret, {
        gasLimit: this.config.ethereum.gasLimit || 200000,
        gasPrice: this.config.ethereum.gasPrice ? 
          ethers.parseUnits(this.config.ethereum.gasPrice, 'gwei') : undefined
      });

      const receipt = await tx.wait();

      return {
        success: true,
        txHash: receipt.hash
      };
    } catch (error) {
      return {
        success: false,
        error: `Failed to withdraw from Ethereum escrow: ${formatError(error)}`
      };
    }
  }

  /**
   * Withdraw from Cosmos escrow using secret
   */
  async withdrawFromCosmosEscrow(
    escrowId: string,
    secret: string,
    isSource: boolean = true
  ): Promise<WithdrawResult> {
    try {
      const accounts = await this.cosmosClient.getAccounts();
      const sender = accounts[0].address;

      const msg = {
        withdraw_src: isSource ? {
          escrow_id: escrowId,
          secret
        } : {
          withdraw_dst: {
            escrow_id: escrowId,
            secret
          }
        }
      };

      const result = await this.cosmosClient.execute(
        sender,
        this.config.cosmos.escrowContractAddress,
        msg,
        'auto'
      );

      return {
        success: true,
        txHash: result.transactionHash
      };
    } catch (error) {
      return {
        success: false,
        error: `Failed to withdraw from Cosmos escrow: ${formatError(error)}`
      };
    }
  }

  // ==================== CANCEL OPERATIONS ====================

  /**
   * Cancel Ethereum escrow
   */
  async cancelEthereumEscrow(
    escrowAddress: string,
    privateKey: string
  ): Promise<CancelResult> {
    try {
      if (!isValidEthereumAddress(escrowAddress)) {
        return {
          success: false,
          error: 'Invalid Ethereum escrow address'
        };
      }

      const wallet = new ethers.Wallet(privateKey, this.ethereumProvider);
      const escrowContract = new ethers.Contract(escrowAddress, USER_ESCROW_ABI, wallet);

      const tx = await escrowContract.cancel({
        gasLimit: this.config.ethereum.gasLimit || 200000,
        gasPrice: this.config.ethereum.gasPrice ? 
          ethers.parseUnits(this.config.ethereum.gasPrice, 'gwei') : undefined
      });

      const receipt = await tx.wait();

      return {
        success: true,
        txHash: receipt.hash
      };
    } catch (error) {
      return {
        success: false,
        error: `Failed to cancel Ethereum escrow: ${formatError(error)}`
      };
    }
  }

  /**
   * Cancel Cosmos escrow
   */
  async cancelCosmosEscrow(
    escrowId: string,
    isSource: boolean = true
  ): Promise<CancelResult> {
    try {
      const accounts = await this.cosmosClient.getAccounts();
      const sender = accounts[0].address;

      const msg = {
        cancel_src: isSource ? {
          escrow_id: escrowId
        } : {
          cancel_dst: {
            escrow_id: escrowId
          }
        }
      };

      const result = await this.cosmosClient.execute(
        sender,
        this.config.cosmos.escrowContractAddress,
        msg,
        'auto'
      );

      return {
        success: true,
        txHash: result.transactionHash
      };
    } catch (error) {
      return {
        success: false,
        error: `Failed to cancel Cosmos escrow: ${formatError(error)}`
      };
    }
  }

  // ==================== PUBLIC OPERATIONS ====================

  /**
   * Public withdraw from Ethereum escrow
   */
  async publicWithdrawFromEthereumEscrow(
    escrowAddress: string,
    privateKey: string
  ): Promise<WithdrawResult> {
    try {
      if (!isValidEthereumAddress(escrowAddress)) {
        return {
          success: false,
          error: 'Invalid Ethereum escrow address'
        };
      }

      const wallet = new ethers.Wallet(privateKey, this.ethereumProvider);
      const escrowContract = new ethers.Contract(escrowAddress, USER_ESCROW_ABI, wallet);

      const tx = await escrowContract.publicWithdraw({
        gasLimit: this.config.ethereum.gasLimit || 200000,
        gasPrice: this.config.ethereum.gasPrice ? 
          ethers.parseUnits(this.config.ethereum.gasPrice, 'gwei') : undefined
      });

      const receipt = await tx.wait();

      return {
        success: true,
        txHash: receipt.hash
      };
    } catch (error) {
      return {
        success: false,
        error: `Failed to public withdraw from Ethereum escrow: ${formatError(error)}`
      };
    }
  }

  /**
   * Public cancel Ethereum escrow
   */
  async publicCancelEthereumEscrow(
    escrowAddress: string,
    privateKey: string
  ): Promise<CancelResult> {
    try {
      if (!isValidEthereumAddress(escrowAddress)) {
        return {
          success: false,
          error: 'Invalid Ethereum escrow address'
        };
      }

      const wallet = new ethers.Wallet(privateKey, this.ethereumProvider);
      const escrowContract = new ethers.Contract(escrowAddress, USER_ESCROW_ABI, wallet);

      const tx = await escrowContract.publicCancel({
        gasLimit: this.config.ethereum.gasLimit || 200000,
        gasPrice: this.config.ethereum.gasPrice ? 
          ethers.parseUnits(this.config.ethereum.gasPrice, 'gwei') : undefined
      });

      const receipt = await tx.wait();

      return {
        success: true,
        txHash: receipt.hash
      };
    } catch (error) {
      return {
        success: false,
        error: `Failed to public cancel Ethereum escrow: ${formatError(error)}`
      };
    }
  }

  /**
   * Public withdraw from Cosmos escrow
   */
  async publicWithdrawFromCosmosEscrow(
    escrowId: string,
    isSource: boolean = true
  ): Promise<WithdrawResult> {
    try {
      const accounts = await this.cosmosClient.getAccounts();
      const sender = accounts[0].address;

      const msg = {
        public_withdraw_src: isSource ? {
          escrow_id: escrowId
        } : {
          public_withdraw_dst: {
            escrow_id: escrowId
          }
        }
      };

      const result = await this.cosmosClient.execute(
        sender,
        this.config.cosmos.escrowContractAddress,
        msg,
        'auto'
      );

      return {
        success: true,
        txHash: result.transactionHash
      };
    } catch (error) {
      return {
        success: false,
        error: `Failed to public withdraw from Cosmos escrow: ${formatError(error)}`
      };
    }
  }

  /**
   * Public cancel Cosmos escrow
   */
  async publicCancelCosmosEscrow(
    escrowId: string,
    isSource: boolean = true
  ): Promise<CancelResult> {
    try {
      const accounts = await this.cosmosClient.getAccounts();
      const sender = accounts[0].address;

      const msg = {
        public_cancel_src: isSource ? {
          escrow_id: escrowId
        } : {
          public_cancel_dst: {
            escrow_id: escrowId
          }
        }
      };

      const result = await this.cosmosClient.execute(
        sender,
        this.config.cosmos.escrowContractAddress,
        msg,
        'auto'
      );

      return {
        success: true,
        txHash: result.transactionHash
      };
    } catch (error) {
      return {
        success: false,
        error: `Failed to public cancel Cosmos escrow: ${formatError(error)}`
      };
    }
  }

  // ==================== QUERY OPERATIONS ====================

  /**
   * Get Ethereum escrow info
   */
  async getEthereumEscrowInfo(escrowAddress: string): Promise<EscrowInfo | null> {
    try {
      if (!isValidEthereumAddress(escrowAddress)) {
        return null;
      }

      const escrowContract = new ethers.Contract(escrowAddress, USER_ESCROW_ABI, this.ethereumProvider);
      const info = await escrowContract.getEscrowInfo();

      return {
        maker: info.maker,
        taker: info.taker,
        amount: info.amount.toString(),
        timelock: info.timelock.toNumber(),
        isActive: info.isActive
      };
    } catch (error) {
      console.error(`Failed to get Ethereum escrow info: ${formatError(error)}`);
      return null;
    }
  }

  /**
   * Get Cosmos escrow info
   */
  async getCosmosEscrowInfo(escrowId: string): Promise<EscrowInfo | null> {
    try {
      const queryMsg = {
        get_escrow: {
          escrow_id: escrowId
        }
      };

      const result = await this.cosmosClient.queryContractSmart(
        this.config.cosmos.escrowContractAddress,
        queryMsg
      );

      return {
        maker: result.maker,
        taker: result.taker,
        amount: result.amount,
        timelock: result.timelock,
        isActive: result.is_active
      };
    } catch (error) {
      console.error(`Failed to get Cosmos escrow info: ${formatError(error)}`);
      return null;
    }
  }

  /**
   * Get user's Ethereum address
   */
  async getEthereumAddress(privateKey: string): Promise<string> {
    const wallet = new ethers.Wallet(privateKey);
    return wallet.address;
  }

  /**
   * Get user's Cosmos address
   */
  async getCosmosAddress(): Promise<string> {
    const accounts = await this.cosmosClient.getAccounts();
    return accounts[0].address;
  }
} 