import { EthereumClient } from './ethereum-client';
import { CosmosClient } from './cosmos-client';
import { 
  ResolverConfig, 
  EscrowConfig, 
  EscrowResult, 
  WithdrawResult, 
  CancelResult,
  ChainType,
  EscrowState
} from './types';
import { 
  generateSecretAndHashlock, 
  generateEscrowId, 
  calculateTimelocks,
  formatError,
  retry
} from './utils';

export class CrossChainResolver {
  private ethereumClient: EthereumClient;
  private cosmosClient: CosmosClient;
  private config: ResolverConfig;
  private escrowStates: Map<string, EscrowState> = new Map();

  constructor(config: ResolverConfig) {
    this.config = config;
    this.ethereumClient = new EthereumClient(config.ethereum);
    this.cosmosClient = new CosmosClient(config.cosmos);
  }

  /**
   * Initialize the resolver
   */
  async initialize(): Promise<void> {
    try {
      await this.cosmosClient.initialize();
      console.log('Resolver initialized successfully');
    } catch (error) {
      throw new Error(`Failed to initialize resolver: ${formatError(error)}`);
    }
  }

  /**
   * Create cross-chain escrow
   */
  async createCrossChainEscrow(escrowConfig: EscrowConfig): Promise<EscrowResult> {
    try {
      // Validate configuration
      this.validateEscrowConfig(escrowConfig);

      // Generate secret and hashlock if not provided
      const secretInfo = escrowConfig.secret 
        ? { secret: escrowConfig.secret, hashlock: escrowConfig.hashlock, index: 0 }
        : generateSecretAndHashlock();

      // Calculate timelocks
      const currentTime = Math.floor(Date.now() / 1000);
      const timelocks = calculateTimelocks(currentTime);

      // Create escrow ID
      const escrowId = generateEscrowId();

      let sourceEscrowAddress: string;
      let destinationEscrowAddress: string;

      // Create escrows based on source chain
      if (escrowConfig.sourceChain === ChainType.ETHEREUM) {
        // Create source escrow on Ethereum
        sourceEscrowAddress = await this.createEthereumSourceEscrow(
          secretInfo.hashlock,
          escrowConfig.maker,
          escrowConfig.taker,
          escrowConfig.sourceToken,
          escrowConfig.amount,
          timelocks.withdrawal
        );

        // Create destination escrow on Cosmos
        destinationEscrowAddress = await this.createCosmosDestinationEscrow(
          secretInfo.hashlock,
          escrowConfig.maker,
          escrowConfig.taker,
          escrowConfig.destinationToken,
          escrowConfig.amount,
          timelocks.withdrawal
        );
      } else {
        // Create source escrow on Cosmos
        sourceEscrowAddress = await this.createCosmosSourceEscrow(
          secretInfo.hashlock,
          escrowConfig.maker,
          escrowConfig.taker,
          escrowConfig.sourceToken,
          escrowConfig.amount,
          timelocks.withdrawal
        );

        // Create destination escrow on Ethereum
        destinationEscrowAddress = await this.createEthereumDestinationEscrow(
          secretInfo.hashlock,
          escrowConfig.maker,
          escrowConfig.taker,
          escrowConfig.destinationToken,
          escrowConfig.amount,
          timelocks.withdrawal
        );
      }

      // Store escrow state
      const escrowState: EscrowState = {
        escrowId,
        sourceChain: escrowConfig.sourceChain,
        destinationChain: escrowConfig.destinationChain,
        sourceEscrowAddress,
        destinationEscrowAddress,
        status: 'active' as any,
        createdAt: currentTime,
        expiresAt: currentTime + timelocks.withdrawal
      };

      this.escrowStates.set(escrowId, escrowState);

      return {
        success: true,
        escrowId,
        sourceEscrowAddress,
        destinationEscrowAddress,
        txHash: `${sourceEscrowAddress}-${destinationEscrowAddress}`
      };
    } catch (error) {
      return {
        success: false,
        error: formatError(error)
      };
    }
  }

  /**
   * Withdraw from escrow using secret
   */
  async withdrawFromEscrow(
    escrowId: string,
    secret: string,
    isSource: boolean = true
  ): Promise<WithdrawResult> {
    try {
      const escrowState = this.escrowStates.get(escrowId);
      if (!escrowState) {
        return {
          success: false,
          error: 'Escrow not found'
        };
      }

      if (isSource) {
        if (escrowState.sourceChain === ChainType.ETHEREUM) {
          return await this.ethereumClient.withdrawFromEscrow(
            escrowState.sourceEscrowAddress,
            secret,
            true
          );
        } else {
          return await this.cosmosClient.withdrawFromEscrow(
            escrowId,
            secret,
            true
          );
        }
      } else {
        if (escrowState.destinationChain === ChainType.ETHEREUM) {
          return await this.ethereumClient.withdrawFromEscrow(
            escrowState.destinationEscrowAddress,
            secret,
            false
          );
        } else {
          return await this.cosmosClient.withdrawFromEscrow(
            escrowId,
            secret,
            false
          );
        }
      }
    } catch (error) {
      return {
        success: false,
        error: formatError(error)
      };
    }
  }

  /**
   * Cancel escrow
   */
  async cancelEscrow(
    escrowId: string,
    isSource: boolean = true
  ): Promise<CancelResult> {
    try {
      const escrowState = this.escrowStates.get(escrowId);
      if (!escrowState) {
        return {
          success: false,
          error: 'Escrow not found'
        };
      }

      if (isSource) {
        if (escrowState.sourceChain === ChainType.ETHEREUM) {
          return await this.ethereumClient.cancelEscrow(
            escrowState.sourceEscrowAddress,
            true
          );
        } else {
          return await this.cosmosClient.cancelEscrow(
            escrowId,
            true
          );
        }
      } else {
        if (escrowState.destinationChain === ChainType.ETHEREUM) {
          return await this.ethereumClient.cancelEscrow(
            escrowState.destinationEscrowAddress,
            false
          );
        } else {
          return await this.cosmosClient.cancelEscrow(
            escrowId,
            false
          );
        }
      }
    } catch (error) {
      return {
        success: false,
        error: formatError(error)
      };
    }
  }

  /**
   * Public withdraw from escrow
   */
  async publicWithdrawFromEscrow(
    escrowId: string,
    isSource: boolean = true
  ): Promise<WithdrawResult> {
    try {
      const escrowState = this.escrowStates.get(escrowId);
      if (!escrowState) {
        return {
          success: false,
          error: 'Escrow not found'
        };
      }

      if (isSource) {
        if (escrowState.sourceChain === ChainType.ETHEREUM) {
          return await this.ethereumClient.publicWithdrawFromEscrow(
            escrowState.sourceEscrowAddress,
            true
          );
        } else {
          return await this.cosmosClient.publicWithdrawFromEscrow(
            escrowId,
            true
          );
        }
      } else {
        if (escrowState.destinationChain === ChainType.ETHEREUM) {
          return await this.ethereumClient.publicWithdrawFromEscrow(
            escrowState.destinationEscrowAddress,
            false
          );
        } else {
          return await this.cosmosClient.publicWithdrawFromEscrow(
            escrowId,
            false
          );
        }
      }
    } catch (error) {
      return {
        success: false,
        error: formatError(error)
      };
    }
  }

  /**
   * Public cancel escrow
   */
  async publicCancelEscrow(
    escrowId: string,
    isSource: boolean = true
  ): Promise<CancelResult> {
    try {
      const escrowState = this.escrowStates.get(escrowId);
      if (!escrowState) {
        return {
          success: false,
          error: 'Escrow not found'
        };
      }

      if (isSource) {
        if (escrowState.sourceChain === ChainType.ETHEREUM) {
          return await this.ethereumClient.publicCancelEscrow(
            escrowState.sourceEscrowAddress,
            true
          );
        } else {
          return await this.cosmosClient.publicCancelEscrow(
            escrowId,
            true
          );
        }
      } else {
        if (escrowState.destinationChain === ChainType.ETHEREUM) {
          return await this.ethereumClient.publicCancelEscrow(
            escrowState.destinationEscrowAddress,
            false
          );
        } else {
          return await this.cosmosClient.publicCancelEscrow(
            escrowId,
            false
          );
        }
      }
    } catch (error) {
      return {
        success: false,
        error: formatError(error)
      };
    }
  }

  /**
   * Get escrow state
   */
  getEscrowState(escrowId: string): EscrowState | undefined {
    return this.escrowStates.get(escrowId);
  }

  /**
   * Get all escrow states
   */
  getAllEscrowStates(): EscrowState[] {
    return Array.from(this.escrowStates.values());
  }

  /**
   * Create Ethereum source escrow
   */
  private async createEthereumSourceEscrow(
    hashlock: string,
    maker: string,
    taker: string,
    token: string,
    amount: string,
    timelock: number
  ): Promise<string> {
    return await retry(async () => {
      return await this.ethereumClient.getEscrowSrcAddress(
        hashlock,
        maker,
        taker,
        token,
        amount,
        timelock
      );
    });
  }

  /**
   * Create Ethereum destination escrow
   */
  private async createEthereumDestinationEscrow(
    hashlock: string,
    maker: string,
    taker: string,
    token: string,
    amount: string,
    timelock: number
  ): Promise<string> {
    return await retry(async () => {
      return await this.ethereumClient.createDstEscrow(
        hashlock,
        maker,
        taker,
        token,
        amount,
        timelock
      );
    });
  }

  /**
   * Create Cosmos source escrow
   */
  private async createCosmosSourceEscrow(
    hashlock: string,
    maker: string,
    taker: string,
    token: string,
    amount: string,
    timelock: number
  ): Promise<string> {
    return await retry(async () => {
      return await this.cosmosClient.createSrcEscrow(
        hashlock,
        maker,
        taker,
        token,
        amount,
        timelock
      );
    });
  }

  /**
   * Create Cosmos destination escrow
   */
  private async createCosmosDestinationEscrow(
    hashlock: string,
    maker: string,
    taker: string,
    token: string,
    amount: string,
    timelock: number
  ): Promise<string> {
    return await retry(async () => {
      return await this.cosmosClient.createDstEscrow(
        hashlock,
        maker,
        taker,
        token,
        amount,
        timelock
      );
    });
  }

  /**
   * Validate escrow configuration
   */
  private validateEscrowConfig(config: EscrowConfig): void {
    if (config.sourceChain === config.destinationChain) {
      throw new Error('Source and destination chains must be different');
    }

    if (!config.maker || !config.taker) {
      throw new Error('Maker and taker addresses are required');
    }

    if (!config.amount || parseFloat(config.amount) <= 0) {
      throw new Error('Amount must be greater than 0');
    }

    if (!config.sourceToken || !config.destinationToken) {
      throw new Error('Source and destination tokens are required');
    }

    if (!config.hashlock) {
      throw new Error('Hashlock is required');
    }
  }
} 