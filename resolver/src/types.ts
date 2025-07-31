export interface EscrowConfig {
  sourceChain: ChainType;
  destinationChain: ChainType;
  sourceToken: string;
  destinationToken: string;
  amount: string;
  maker: string;
  taker: string;
  timelocks: Timelocks;
  hashlock: string;
  secret?: string;
}

export interface Timelocks {
  withdrawal: number;
  cancellation: number;
  rescue: number;
}

export enum ChainType {
  ETHEREUM = 'ethereum',
  COSMOS = 'cosmos'
}

export interface ChainConfig {
  rpcUrl: string;
  chainId: string;
  gasPrice?: string;
  gasLimit?: number;
}

export interface EthereumConfig extends ChainConfig {
  privateKey: string;
  escrowFactoryAddress: string;
  escrowSrcAddress: string;
  escrowDstAddress: string;
}

export interface CosmosConfig extends ChainConfig {
  mnemonic: string;
  escrowContractAddress: string;
  prefix: string;
  denom: string;
}

export interface ResolverConfig {
  ethereum: EthereumConfig;
  cosmos: CosmosConfig;
}

export interface EscrowState {
  escrowId: string;
  sourceChain: ChainType;
  destinationChain: ChainType;
  sourceEscrowAddress: string;
  destinationEscrowAddress: string;
  status: EscrowStatus;
  createdAt: number;
  expiresAt: number;
}

export enum EscrowStatus {
  PENDING = 'pending',
  ACTIVE = 'active',
  COMPLETED = 'completed',
  CANCELLED = 'cancelled',
  EXPIRED = 'expired'
}

export interface EscrowResult {
  success: boolean;
  escrowId?: string;
  sourceEscrowAddress?: string;
  destinationEscrowAddress?: string;
  error?: string;
  txHash?: string;
}

export interface WithdrawResult {
  success: boolean;
  txHash?: string;
  error?: string;
}

export interface CancelResult {
  success: boolean;
  txHash?: string;
  error?: string;
}

export interface SecretInfo {
  secret: string;
  hashlock: string;
  index: number;
} 