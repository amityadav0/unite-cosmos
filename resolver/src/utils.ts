import { ethers } from 'ethers';
import { SecretInfo } from './types';

/**
 * Generate a random secret
 */
export function generateSecret(): string {
  const bytes = ethers.randomBytes(32);
  return ethers.hexlify(bytes);
}

/**
 * Create hashlock from secret
 */
export function createHashlock(secret: string): string {
  return ethers.keccak256(secret);
}

/**
 * Generate secret and hashlock pair
 */
export function generateSecretAndHashlock(): SecretInfo {
  const secret = generateSecret();
  const hashlock = createHashlock(secret);
  
  return {
    secret,
    hashlock,
    index: 0
  };
}

/**
 * Generate multiple secrets for partial fills
 */
export function generateSecretsForPartialFills(numParts: number): SecretInfo[] {
  const secrets: SecretInfo[] = [];
  
  for (let i = 0; i <= numParts; i++) {
    const secret = generateSecret();
    const hashlock = createHashlock(secret);
    
    secrets.push({
      secret,
      hashlock,
      index: i
    });
  }
  
  return secrets;
}

/**
 * Create Merkle tree from secrets
 */
export function createMerkleTree(secrets: SecretInfo[]): string {
  const leaves = secrets.map(secret => 
    ethers.keccak256(ethers.AbiCoder.defaultAbiCoder().encode(
      ['uint256', 'bytes32'],
      [secret.index, secret.hashlock]
    ))
  );
  
  // Simple binary merkle tree implementation
  return buildMerkleTree(leaves);
}

/**
 * Build merkle tree from leaves
 */
function buildMerkleTree(leaves: string[]): string {
  if (leaves.length === 0) {
    return ethers.ZeroHash;
  }
  
  if (leaves.length === 1) {
    return leaves[0];
  }
  
  const newLeaves: string[] = [];
  
  for (let i = 0; i < leaves.length; i += 2) {
    const left = leaves[i];
    const right = i + 1 < leaves.length ? leaves[i + 1] : left;
    
    const combined = ethers.concat([left, right]);
    newLeaves.push(ethers.keccak256(combined));
  }
  
  return buildMerkleTree(newLeaves);
}

/**
 * Validate Ethereum address
 */
export function isValidEthereumAddress(address: string): boolean {
  try {
    ethers.getAddress(address);
    return true;
  } catch {
    return false;
  }
}

/**
 * Validate Cosmos address
 */
export function isValidCosmosAddress(address: string, prefix: string): boolean {
  // Basic validation for Cosmos addresses
  const pattern = new RegExp(`^${prefix}1[a-zA-Z0-9]{38}$`);
  return pattern.test(address);
}

/**
 * Convert amount to wei (for Ethereum)
 */
export function toWei(amount: string, decimals: number = 18): string {
  return ethers.parseUnits(amount, decimals).toString();
}

/**
 * Convert amount from wei (for Ethereum)
 */
export function fromWei(amount: string, decimals: number = 18): string {
  return ethers.formatUnits(amount, decimals);
}

/**
 * Convert amount to micro units (for Cosmos)
 */
export function toMicroUnits(amount: string, decimals: number = 6): string {
  return (parseFloat(amount) * Math.pow(10, decimals)).toString();
}

/**
 * Convert amount from micro units (for Cosmos)
 */
export function fromMicroUnits(amount: string, decimals: number = 6): string {
  return (parseFloat(amount) / Math.pow(10, decimals)).toString();
}

/**
 * Generate unique escrow ID
 */
export function generateEscrowId(): string {
  const timestamp = Date.now();
  const random = Math.floor(Math.random() * 1000000);
  return `${timestamp}-${random}`;
}

/**
 * Calculate timelock values
 */
export function calculateTimelocks(
  baseTime: number,
  withdrawalDelay: number = 3600, // 1 hour
  cancellationDelay: number = 7200, // 2 hours
  rescueDelay: number = 86400 // 24 hours
): { withdrawal: number; cancellation: number; rescue: number } {
  return {
    withdrawal: baseTime + withdrawalDelay,
    cancellation: baseTime + cancellationDelay,
    rescue: baseTime + rescueDelay
  };
}

/**
 * Check if timelock has expired
 */
export function isTimelockExpired(timelock: number, currentTime: number): boolean {
  return currentTime >= timelock;
}

/**
 * Format error message
 */
export function formatError(error: any): string {
  if (typeof error === 'string') {
    return error;
  }
  
  if (error instanceof Error) {
    return error.message;
  }
  
  if (error && typeof error === 'object') {
    return error.message || JSON.stringify(error);
  }
  
  return 'Unknown error occurred';
}

/**
 * Sleep utility
 */
export function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}

/**
 * Retry function with exponential backoff
 */
export async function retry<T>(
  fn: () => Promise<T>,
  maxRetries: number = 3,
  baseDelay: number = 1000
): Promise<T> {
  let lastError: any;
  
  for (let i = 0; i <= maxRetries; i++) {
    try {
      return await fn();
    } catch (error) {
      lastError = error;
      
      if (i === maxRetries) {
        throw error;
      }
      
      const delay = baseDelay * Math.pow(2, i);
      await sleep(delay);
    }
  }
  
  throw lastError;
} 