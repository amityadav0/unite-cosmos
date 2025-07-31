import {
  CosmWasmClient,
  SigningCosmWasmClient,
  MsgExecuteContractEncodeObject
} from '@cosmjs/cosmwasm-stargate';
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing';
import { CosmosConfig, WithdrawResult, CancelResult } from './types';

export class CosmosClient {
  private client: SigningCosmWasmClient;
  private config: CosmosConfig;
  private wallet: DirectSecp256k1HdWallet;

  constructor(config: CosmosConfig) {
    this.config = config;
  }

  /**
   * Initialize the Cosmos client
   */
  async initialize(): Promise<void> {
    try {
      this.wallet = await DirectSecp256k1HdWallet.fromMnemonic(
        this.config.mnemonic,
        {
          prefix: this.config.prefix
        }
      );

      const accounts = await this.wallet.getAccounts();
      if (accounts.length === 0) {
        throw new Error('No accounts found in wallet');
      }

      this.client = await SigningCosmWasmClient.connectWithSigner(
        this.config.rpcUrl,
        this.wallet
      );
    } catch (error) {
      throw new Error(`Failed to initialize Cosmos client: ${error}`);
    }
  }

  /**
   * Create source escrow on Cosmos
   */
  async createSrcEscrow(
    hashlock: string,
    maker: string,
    taker: string,
    token: string,
    amount: string,
    timelock: number
  ): Promise<string> {
    try {
      const accounts = await this.wallet.getAccounts();
      const sender = accounts[0].address;

      const msg = {
        create_src_escrow: {
          hashlock,
          maker,
          taker,
          token,
          amount,
          timelock: timelock.toString()
        }
      };

      const result = await this.client.execute(
        sender,
        this.config.escrowContractAddress,
        msg,
        'auto'
      );

      // Extract escrow address from events
      const escrowAddress = this.extractEscrowAddressFromEvents(result.events);
      if (!escrowAddress) {
        throw new Error('Escrow address not found in transaction events');
      }

      return escrowAddress;
    } catch (error) {
      throw new Error(`Failed to create source escrow: ${error}`);
    }
  }

  /**
   * Create destination escrow on Cosmos
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
      const accounts = await this.wallet.getAccounts();
      const sender = accounts[0].address;

      const msg = {
        create_dst_escrow: {
          hashlock,
          maker,
          taker,
          token,
          amount,
          timelock: timelock.toString()
        }
      };

      const result = await this.client.execute(
        sender,
        this.config.escrowContractAddress,
        msg,
        'auto'
      );

      // Extract escrow address from events
      const escrowAddress = this.extractEscrowAddressFromEvents(result.events);
      if (!escrowAddress) {
        throw new Error('Escrow address not found in transaction events');
      }

      return escrowAddress;
    } catch (error) {
      throw new Error(`Failed to create destination escrow: ${error}`);
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
      const accounts = await this.wallet.getAccounts();
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

      const result = await this.client.execute(
        sender,
        this.config.escrowContractAddress,
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
        error: `Failed to withdraw from escrow: ${error}`
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
      const accounts = await this.wallet.getAccounts();
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

      const result = await this.client.execute(
        sender,
        this.config.escrowContractAddress,
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
        error: `Failed to cancel escrow: ${error}`
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
      const accounts = await this.wallet.getAccounts();
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

      const result = await this.client.execute(
        sender,
        this.config.escrowContractAddress,
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
        error: `Failed to public withdraw from escrow: ${error}`
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
      const accounts = await this.wallet.getAccounts();
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

      const result = await this.client.execute(
        sender,
        this.config.escrowContractAddress,
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
        error: `Failed to public cancel escrow: ${error}`
      };
    }
  }

  /**
   * Rescue funds from escrow
   */
  async rescueFundsFromEscrow(
    escrowId: string,
    isSource: boolean = true
  ): Promise<WithdrawResult> {
    try {
      const accounts = await this.wallet.getAccounts();
      const sender = accounts[0].address;

      const msg = {
        rescue: {
          escrow_id: escrowId
        }
      };

      const result = await this.client.execute(
        sender,
        this.config.escrowContractAddress,
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
        error: `Failed to rescue funds from escrow: ${error}`
      };
    }
  }

  /**
   * Get escrow state
   */
  async getEscrowState(escrowId: string): Promise<any> {
    try {
      const queryMsg = {
        get_escrow: {
          escrow_id: escrowId
        }
      };

      const result = await this.client.queryContractSmart(
        this.config.escrowContractAddress,
        queryMsg
      );

      return result;
    } catch (error) {
      throw new Error(`Failed to get escrow state: ${error}`);
    }
  }

  /**
   * Get current block timestamp
   */
  async getCurrentTimestamp(): Promise<number> {
    try {
      const block = await this.client.getBlock();
      return Math.floor(new Date(block.header.time).getTime() / 1000);
    } catch (error) {
      return Math.floor(Date.now() / 1000);
    }
  }

  /**
   * Get wallet address
   */
  async getAddress(): Promise<string> {
    const accounts = await this.wallet.getAccounts();
    return accounts[0].address;
  }

  /**
   * Extract escrow address from transaction events
   */
  private extractEscrowAddressFromEvents(events: any[]): string | null {
    for (const event of events) {
      if (event.type === 'wasm' || event.type === 'execute') {
        for (const attr of event.attributes) {
          if (attr.key === 'escrow_address' || attr.key === 'contract_address') {
            return attr.value;
          }
        }
      }
    }
    return null;
  }
} 