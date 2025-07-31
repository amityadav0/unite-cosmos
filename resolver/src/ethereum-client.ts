import { ethers } from 'ethers';
import { EthereumConfig, EscrowConfig, EscrowResult, WithdrawResult, CancelResult } from './types';

// ABI for Escrow Factory
const ESCROW_FACTORY_ABI = [
  'function addressOfEscrowSrc(bytes32 hashlock, address maker, address taker, address token, uint256 amount, uint256 timelock) view returns (address)',
  'function createDstEscrow(bytes32 hashlock, address maker, address taker, address token, uint256 amount, uint256 timelock) returns (address)',
  'event EscrowCreated(address indexed escrow, bytes32 indexed hashlock, address indexed maker)'
];

// ABI for Escrow Source
const ESCROW_SRC_ABI = [
  'function withdraw(string memory secret)',
  'function cancel()',
  'function publicWithdraw()',
  'function publicCancel()',
  'function rescueFunds()',
  'event Withdrawn(address indexed recipient, uint256 amount)',
  'event Cancelled(address indexed recipient, uint256 amount)',
  'event PublicWithdrawn(address indexed recipient, uint256 amount)',
  'event PublicCancelled(address indexed recipient, uint256 amount)'
];

// ABI for Escrow Destination
const ESCROW_DST_ABI = [
  'function withdraw(string memory secret)',
  'function cancel()',
  'function publicWithdraw()',
  'function publicCancel()',
  'function rescueFunds()',
  'event Withdrawn(address indexed recipient, uint256 amount)',
  'event Cancelled(address indexed recipient, uint256 amount)',
  'event PublicWithdrawn(address indexed recipient, uint256 amount)',
  'event PublicCancelled(address indexed recipient, uint256 amount)'
];

export class EthereumClient {
  private provider: ethers.JsonRpcProvider;
  private wallet: ethers.Wallet;
  private factoryContract: ethers.Contract;
  private config: EthereumConfig;

  constructor(config: EthereumConfig) {
    this.config = config;
    this.provider = new ethers.JsonRpcProvider(config.rpcUrl);
    this.wallet = new ethers.Wallet(config.privateKey, this.provider);
    this.factoryContract = new ethers.Contract(
      config.escrowFactoryAddress,
      ESCROW_FACTORY_ABI,
      this.wallet
    );
  }

  /**
   * Get the future address of an escrow source contract
   */
  async getEscrowSrcAddress(
    hashlock: string,
    maker: string,
    taker: string,
    token: string,
    amount: string,
    timelock: number
  ): Promise<string> {
    try {
      const address = await this.factoryContract.addressOfEscrowSrc(
        hashlock,
        maker,
        taker,
        token,
        amount,
        timelock
      );
      return address;
    } catch (error) {
      throw new Error(`Failed to get escrow source address: ${error}`);
    }
  }

  /**
   * Create destination escrow contract
   */
  async createDstEscrow(
    hashlock: string,
    maker: string,
    taker: string,
    token: string,
    amount: string,
    timelock: number
  ): Promise<string> {
    try {
      const tx = await this.factoryContract.createDstEscrow(
        hashlock,
        maker,
        taker,
        token,
        amount,
        timelock,
        {
          gasLimit: this.config.gasLimit || 500000,
          gasPrice: this.config.gasPrice ? ethers.parseUnits(this.config.gasPrice, 'gwei') : undefined
        }
      );

      const receipt = await tx.wait();
      
      // Find the EscrowCreated event
      const event = receipt.logs.find((log: any) => {
        try {
          const parsed = this.factoryContract.interface.parseLog(log);
          return parsed.name === 'EscrowCreated';
        } catch {
          return false;
        }
      });

      if (event) {
        const parsed = this.factoryContract.interface.parseLog(event);
        return parsed.args.escrow;
      }

      throw new Error('EscrowCreated event not found in transaction receipt');
    } catch (error) {
      throw new Error(`Failed to create destination escrow: ${error}`);
    }
  }

  /**
   * Withdraw from escrow using secret
   */
  async withdrawFromEscrow(
    escrowAddress: string,
    secret: string,
    isSource: boolean = true
  ): Promise<WithdrawResult> {
    try {
      const abi = isSource ? ESCROW_SRC_ABI : ESCROW_DST_ABI;
      const escrowContract = new ethers.Contract(escrowAddress, abi, this.wallet);

      const tx = await escrowContract.withdraw(secret, {
        gasLimit: this.config.gasLimit || 200000,
        gasPrice: this.config.gasPrice ? ethers.parseUnits(this.config.gasPrice, 'gwei') : undefined
      });

      const receipt = await tx.wait();
      
      return {
        success: true,
        txHash: receipt.hash
      };
    } catch (error) {
      return {
        success: false,
        error: `Failed to withdraw from escrow: ${error}`
      };
    }
  }

  /**
   * Cancel escrow
   */
  async cancelEscrow(
    escrowAddress: string,
    isSource: boolean = true
  ): Promise<CancelResult> {
    try {
      const abi = isSource ? ESCROW_SRC_ABI : ESCROW_DST_ABI;
      const escrowContract = new ethers.Contract(escrowAddress, abi, this.wallet);

      const tx = await escrowContract.cancel({
        gasLimit: this.config.gasLimit || 200000,
        gasPrice: this.config.gasPrice ? ethers.parseUnits(this.config.gasPrice, 'gwei') : undefined
      });

      const receipt = await tx.wait();
      
      return {
        success: true,
        txHash: receipt.hash
      };
    } catch (error) {
      return {
        success: false,
        error: `Failed to cancel escrow: ${error}`
      };
    }
  }

  /**
   * Public withdraw from escrow
   */
  async publicWithdrawFromEscrow(
    escrowAddress: string,
    isSource: boolean = true
  ): Promise<WithdrawResult> {
    try {
      const abi = isSource ? ESCROW_SRC_ABI : ESCROW_DST_ABI;
      const escrowContract = new ethers.Contract(escrowAddress, abi, this.wallet);

      const tx = await escrowContract.publicWithdraw({
        gasLimit: this.config.gasLimit || 200000,
        gasPrice: this.config.gasPrice ? ethers.parseUnits(this.config.gasPrice, 'gwei') : undefined
      });

      const receipt = await tx.wait();
      
      return {
        success: true,
        txHash: receipt.hash
      };
    } catch (error) {
      return {
        success: false,
        error: `Failed to public withdraw from escrow: ${error}`
      };
    }
  }

  /**
   * Public cancel escrow
   */
  async publicCancelEscrow(
    escrowAddress: string,
    isSource: boolean = true
  ): Promise<CancelResult> {
    try {
      const abi = isSource ? ESCROW_SRC_ABI : ESCROW_DST_ABI;
      const escrowContract = new ethers.Contract(escrowAddress, abi, this.wallet);

      const tx = await escrowContract.publicCancel({
        gasLimit: this.config.gasLimit || 200000,
        gasPrice: this.config.gasPrice ? ethers.parseUnits(this.config.gasPrice, 'gwei') : undefined
      });

      const receipt = await tx.wait();
      
      return {
        success: true,
        txHash: receipt.hash
      };
    } catch (error) {
      return {
        success: false,
        error: `Failed to public cancel escrow: ${error}`
      };
    }
  }

  /**
   * Rescue funds from escrow
   */
  async rescueFundsFromEscrow(
    escrowAddress: string,
    isSource: boolean = true
  ): Promise<WithdrawResult> {
    try {
      const abi = isSource ? ESCROW_SRC_ABI : ESCROW_DST_ABI;
      const escrowContract = new ethers.Contract(escrowAddress, abi, this.wallet);

      const tx = await escrowContract.rescueFunds({
        gasLimit: this.config.gasLimit || 200000,
        gasPrice: this.config.gasPrice ? ethers.parseUnits(this.config.gasPrice, 'gwei') : undefined
      });

      const receipt = await tx.wait();
      
      return {
        success: true,
        txHash: receipt.hash
      };
    } catch (error) {
      return {
        success: false,
        error: `Failed to rescue funds from escrow: ${error}`
      };
    }
  }

  /**
   * Get current block timestamp
   */
  async getCurrentTimestamp(): Promise<number> {
    const block = await this.provider.getBlock('latest');
    return block?.timestamp || Math.floor(Date.now() / 1000);
  }

  /**
   * Get wallet address
   */
  getAddress(): string {
    return this.wallet.address;
  }
} 