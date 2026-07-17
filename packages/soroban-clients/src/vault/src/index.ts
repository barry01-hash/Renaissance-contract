import { Buffer } from "buffer";
import { Address } from "@stellar/stellar-sdk";
import {
  AssembledTransaction,
  Client as ContractClient,
  ClientOptions as ContractClientOptions,
  MethodOptions,
  Result,
  Spec as ContractSpec,
} from "@stellar/stellar-sdk/contract";
import type {
  u32,
  i32,
  u64,
  i64,
  u128,
  i128,
  u256,
  i256,
  Option,
  Timepoint,
  Duration,
} from "@stellar/stellar-sdk/contract";
export * from "@stellar/stellar-sdk";
export * as contract from "@stellar/stellar-sdk/contract";
export * as rpc from "@stellar/stellar-sdk/rpc";

if (typeof window !== "undefined") {
  //@ts-ignore Buffer exists
  window.Buffer = window.Buffer || Buffer;
}




export type DataKey = {tag: "Admin", values: void} | {tag: "Paused", values: void} | {tag: "BettingContract", values: void} | {tag: "ReentrancyGuard", values: void} | {tag: "UserBalance", values: readonly [string, string]} | {tag: "VaultBalance", values: readonly [string]} | {tag: "LockedBet", values: readonly [u64, string, string]} | {tag: "PendingWithdrawal", values: readonly [string]} | {tag: "PendingAdmin", values: void};

export const VaultError = {
  /**
   * Caller is not the authorized betting contract
   */
  200: {message:"UnauthorizedBettingContract"},
  /**
   * Insufficient balance to perform the operation
   */
  201: {message:"InsufficientBalance"},
  /**
   * Reentrancy attempt detected
   */
  202: {message:"ReentrancyDetected"},
  /**
   * Invalid amount (must be positive)
   */
  203: {message:"InvalidAmount"},
  /**
   * Bet lock not found
   */
  204: {message:"BetLockNotFound"},
  /**
   * Timelock not expired yet
   */
  205: {message:"TimelockNotExpired"},
  /**
   * Token is tracked and cannot be recovered
   */
  206: {message:"TokenIsTracked"},
  /**
   * Mismatch in pending data
   */
  207: {message:"MismatchPendingData"}
}


/**
 * Tracks user balances across all assets
 */
export interface UserBalance {
  available: i128;
  locked: i128;
}


/**
 * Tracks total vault balances for accounting
 */
export interface VaultBalance {
  total_deposited: i128;
  total_locked: i128;
}


export interface PendingAdminData {
  new_admin: string;
  pending_until: u64;
}


export interface PendingWithdrawal {
  amount: i128;
  pending_until: u64;
  to: string;
}


/**
 * Standardized tracking data configuration for interactive betting matches
 */
export interface MatchMetadata {
  asset_token: string;
  match_id: u64;
  player_one: string;
  player_two: string;
  total_pool: i128;
}

/**
 * Centralized platform error codes mapped cleanly across all child contract scopes
 */
export const PlatformError = {
  1: {message:"InternalError"},
  2: {message:"Unauthorized"},
  3: {message:"InvalidAmount"},
  4: {message:"ExpiredDeadline"},
  5: {message:"Overflow"},
  /**
   * Caller holds fewer tokens than the requested debit.
   * Emitted by `renaissance-betting` `place_bet` to enforce the
   * acceptance criterion that bets only settle against real balances.
   */
  6: {message:"InsufficientBalance"},
  7: {message:"Paused"}
}

export interface Client {
  /**
   * Construct and simulate a payout transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Payout winnings to a winner - only callable by the betting contract
   * Releases locked funds and transfers them to the winner
   */
  payout: ({winner, amount, asset, match_id}: {winner: string, amount: i128, asset: string, match_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a deposit transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Deposit tokens into the vault - user stakes their tokens
   * Works with both native XLM (via SAC) and any custom asset
   */
  deposit: ({user, amount, asset}: {user: string, amount: i128, asset: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a withdraw transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Withdraw unused tokens from the vault
   * Only available funds (not locked in active bets) can be withdrawn
   */
  withdraw: ({user, amount, asset}: {user: string, amount: i128, asset: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a set_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Transfer admin rights (timelocked)
   */
  set_admin: ({new_admin}: {new_admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a initialize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * One-time initialization of the vault contract
   * Sets up admin and authorizes the initial betting contract
   */
  initialize: ({admin, betting_contract}: {admin: string, betting_contract: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a lock_for_bet transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Lock funds for an active bet - only callable by the betting contract
   * Moves funds from available to locked state
   */
  lock_for_bet: ({user, amount, asset, match_id}: {user: string, amount: i128, asset: string, match_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a recover_token transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Recover accidentally sent tokens (only for non-tracked tokens)
   */
  recover_token: ({asset, to, amount}: {asset: string, to: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a cancel_set_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Cancel a pending admin transfer
   */
  cancel_set_admin: (options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_user_balance transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Get a specific user's balance for an asset
   */
  get_user_balance: ({user, asset}: {user: string, asset: string}, options?: MethodOptions) => Promise<AssembledTransaction<UserBalance>>

  /**
   * Construct and simulate a get_vault_balance transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Get the total vault balance for a specific asset
   */
  get_vault_balance: ({asset}: {asset: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a is_locked_for_bet transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Check if funds are locked for a specific bet
   */
  is_locked_for_bet: ({match_id, user, asset}: {match_id: u64, user: string, asset: string}, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a emergency_withdraw transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Emergency withdraw of an asset (timelocked)
   */
  emergency_withdraw: ({asset, to, amount}: {asset: string, to: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a set_betting_contract transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Update the authorized betting contract (admin only)
   */
  set_betting_contract: ({new_betting_contract}: {new_betting_contract: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a cancel_emergency_withdraw transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Cancel a pending emergency withdrawal
   */
  cancel_emergency_withdraw: ({asset}: {asset: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
    /** Options for initializing a Client as well as for calling a method, with extras specific to deploying. */
    options: MethodOptions &
      Omit<ContractClientOptions, "contractId"> & {
        /** The hash of the Wasm blob, which must already be installed on-chain. */
        wasmHash: Buffer | string;
        /** Salt used to generate the contract's ID. Passed through to {@link Operation.createCustomContract}. Default: random. */
        salt?: Buffer | Uint8Array;
        /** The format used to decode `wasmHash`, if it's provided as a string. */
        format?: "hex" | "base64";
      }
  ): Promise<AssembledTransaction<T>> {
    return ContractClient.deploy(null, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAAAAAAAHpQYXlvdXQgd2lubmluZ3MgdG8gYSB3aW5uZXIgLSBvbmx5IGNhbGxhYmxlIGJ5IHRoZSBiZXR0aW5nIGNvbnRyYWN0ClJlbGVhc2VzIGxvY2tlZCBmdW5kcyBhbmQgdHJhbnNmZXJzIHRoZW0gdG8gdGhlIHdpbm5lcgAAAAAABnBheW91dAAAAAAABAAAAAAAAAAGd2lubmVyAAAAAAATAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAABWFzc2V0AAAAAAAAEwAAAAAAAAAIbWF0Y2hfaWQAAAAGAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAKVmF1bHRFcnJvcgAA",
        "AAAAAAAAAHJEZXBvc2l0IHRva2VucyBpbnRvIHRoZSB2YXVsdCAtIHVzZXIgc3Rha2VzIHRoZWlyIHRva2VucwpXb3JrcyB3aXRoIGJvdGggbmF0aXZlIFhMTSAodmlhIFNBQykgYW5kIGFueSBjdXN0b20gYXNzZXQAAAAAAAdkZXBvc2l0AAAAAAMAAAAAAAAABHVzZXIAAAATAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAABWFzc2V0AAAAAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAClZhdWx0RXJyb3IAAA==",
        "AAAAAAAAAGdXaXRoZHJhdyB1bnVzZWQgdG9rZW5zIGZyb20gdGhlIHZhdWx0Ck9ubHkgYXZhaWxhYmxlIGZ1bmRzIChub3QgbG9ja2VkIGluIGFjdGl2ZSBiZXRzKSBjYW4gYmUgd2l0aGRyYXduAAAAAAh3aXRoZHJhdwAAAAMAAAAAAAAABHVzZXIAAAATAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAABWFzc2V0AAAAAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAClZhdWx0RXJyb3IAAA==",
        "AAAAAAAAACJUcmFuc2ZlciBhZG1pbiByaWdodHMgKHRpbWVsb2NrZWQpAAAAAAAJc2V0X2FkbWluAAAAAAAAAQAAAAAAAAAJbmV3X2FkbWluAAAAAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAClZhdWx0RXJyb3IAAA==",
        "AAAAAgAAAAAAAAAAAAAAB0RhdGFLZXkAAAAACQAAAAAAAAAyQWRtaW4gYWRkcmVzcyB0aGF0IGNhbiBtYW5hZ2UgYXV0aG9yaXplZCBjb250cmFjdHMAAAAAAAVBZG1pbgAAAAAAAAAAAAAlUGF1c2VkIHN0YXRlIGZsYWcgZm9yIGVtZXJnZW5jeSBzdG9wcwAAAAAAAAZQYXVzZWQAAAAAAAAAAABMQXV0aG9yaXplZCBiZXR0aW5nIGNvbnRyYWN0IGFkZHJlc3MgKG9ubHkgdGhpcyBjYW4gY2FsbCBsb2NrX2Zvcl9iZXQvcGF5b3V0KQAAAA9CZXR0aW5nQ29udHJhY3QAAAAAAAAAACJGbGFnIHRvIHByZXZlbnQgcmVlbnRyYW5jeSBhdHRhY2tzAAAAAAAPUmVlbnRyYW5jeUd1YXJkAAAAAAEAAAA6VXNlciBiYWxhbmNlOiAodXNlciBhZGRyZXNzLCBhc3NldCBhZGRyZXNzKSAtPiBVc2VyQmFsYW5jZQAAAAAAC1VzZXJCYWxhbmNlAAAAAAIAAAATAAAAEwAAAAEAAAA/VmF1bHQgdG90YWwgYmFsYW5jZSBmb3IgYW4gYXNzZXQ6IGFzc2V0IGFkZHJlc3MgLT4gVmF1bHRCYWxhbmNlAAAAAAxWYXVsdEJhbGFuY2UAAAABAAAAEwAAAAEAAABETG9ja2VkIGZ1bmRzIGZvciBhIHNwZWNpZmljIG1hdGNoOiAobWF0Y2hfaWQsIHVzZXIsIGFzc2V0KSAtPiBhbW91bnQAAAAJTG9ja2VkQmV0AAAAAAAAAwAAAAYAAAATAAAAEwAAAAEAAAAfUGVuZGluZyB3aXRoZHJhd2FsIGZvciBhbiBhc3NldAAAAAARUGVuZGluZ1dpdGhkcmF3YWwAAAAAAAABAAAAEwAAAAAAAAAWUGVuZGluZyBhZG1pbiB0cmFuc2ZlcgAAAAAADFBlbmRpbmdBZG1pbg==",
        "AAAAAAAAAGdPbmUtdGltZSBpbml0aWFsaXphdGlvbiBvZiB0aGUgdmF1bHQgY29udHJhY3QKU2V0cyB1cCBhZG1pbiBhbmQgYXV0aG9yaXplcyB0aGUgaW5pdGlhbCBiZXR0aW5nIGNvbnRyYWN0AAAAAAppbml0aWFsaXplAAAAAAACAAAAAAAAAAVhZG1pbgAAAAAAABMAAAAAAAAAEGJldHRpbmdfY29udHJhY3QAAAATAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANUGxhdGZvcm1FcnJvcgAAAA==",
        "AAAAAAAAAG9Mb2NrIGZ1bmRzIGZvciBhbiBhY3RpdmUgYmV0IC0gb25seSBjYWxsYWJsZSBieSB0aGUgYmV0dGluZyBjb250cmFjdApNb3ZlcyBmdW5kcyBmcm9tIGF2YWlsYWJsZSB0byBsb2NrZWQgc3RhdGUAAAAADGxvY2tfZm9yX2JldAAAAAQAAAAAAAAABHVzZXIAAAATAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAABWFzc2V0AAAAAAAAEwAAAAAAAAAIbWF0Y2hfaWQAAAAGAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAKVmF1bHRFcnJvcgAA",
        "AAAABAAAAAAAAAAAAAAAClZhdWx0RXJyb3IAAAAAAAgAAAAtQ2FsbGVyIGlzIG5vdCB0aGUgYXV0aG9yaXplZCBiZXR0aW5nIGNvbnRyYWN0AAAAAAAAG1VuYXV0aG9yaXplZEJldHRpbmdDb250cmFjdAAAAADIAAAALUluc3VmZmljaWVudCBiYWxhbmNlIHRvIHBlcmZvcm0gdGhlIG9wZXJhdGlvbgAAAAAAABNJbnN1ZmZpY2llbnRCYWxhbmNlAAAAAMkAAAAbUmVlbnRyYW5jeSBhdHRlbXB0IGRldGVjdGVkAAAAABJSZWVudHJhbmN5RGV0ZWN0ZWQAAAAAAMoAAAAhSW52YWxpZCBhbW91bnQgKG11c3QgYmUgcG9zaXRpdmUpAAAAAAAADUludmFsaWRBbW91bnQAAAAAAADLAAAAEkJldCBsb2NrIG5vdCBmb3VuZAAAAAAAD0JldExvY2tOb3RGb3VuZAAAAADMAAAAGFRpbWVsb2NrIG5vdCBleHBpcmVkIHlldAAAABJUaW1lbG9ja05vdEV4cGlyZWQAAAAAAM0AAAAoVG9rZW4gaXMgdHJhY2tlZCBhbmQgY2Fubm90IGJlIHJlY292ZXJlZAAAAA5Ub2tlbklzVHJhY2tlZAAAAAAAzgAAABhNaXNtYXRjaCBpbiBwZW5kaW5nIGRhdGEAAAATTWlzbWF0Y2hQZW5kaW5nRGF0YQAAAADP",
        "AAAAAAAAAD5SZWNvdmVyIGFjY2lkZW50YWxseSBzZW50IHRva2VucyAob25seSBmb3Igbm9uLXRyYWNrZWQgdG9rZW5zKQAAAAAADXJlY292ZXJfdG9rZW4AAAAAAAADAAAAAAAAAAVhc3NldAAAAAAAABMAAAAAAAAAAnRvAAAAAAATAAAAAAAAAAZhbW91bnQAAAAAAAsAAAABAAAD6QAAA+0AAAAAAAAH0AAAAApWYXVsdEVycm9yAAA=",
        "AAAAAQAAACZUcmFja3MgdXNlciBiYWxhbmNlcyBhY3Jvc3MgYWxsIGFzc2V0cwAAAAAAAAAAAAtVc2VyQmFsYW5jZQAAAAACAAAAAAAAAAlhdmFpbGFibGUAAAAAAAALAAAAAAAAAAZsb2NrZWQAAAAAAAs=",
        "AAAAAQAAACpUcmFja3MgdG90YWwgdmF1bHQgYmFsYW5jZXMgZm9yIGFjY291bnRpbmcAAAAAAAAAAAAMVmF1bHRCYWxhbmNlAAAAAgAAAAAAAAAPdG90YWxfZGVwb3NpdGVkAAAAAAsAAAAAAAAADHRvdGFsX2xvY2tlZAAAAAs=",
        "AAAAAAAAAB9DYW5jZWwgYSBwZW5kaW5nIGFkbWluIHRyYW5zZmVyAAAAABBjYW5jZWxfc2V0X2FkbWluAAAAAAAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAClZhdWx0RXJyb3IAAA==",
        "AAAAAAAAACpHZXQgYSBzcGVjaWZpYyB1c2VyJ3MgYmFsYW5jZSBmb3IgYW4gYXNzZXQAAAAAABBnZXRfdXNlcl9iYWxhbmNlAAAAAgAAAAAAAAAEdXNlcgAAABMAAAAAAAAABWFzc2V0AAAAAAAAEwAAAAEAAAfQAAAAC1VzZXJCYWxhbmNlAA==",
        "AAAAAAAAADBHZXQgdGhlIHRvdGFsIHZhdWx0IGJhbGFuY2UgZm9yIGEgc3BlY2lmaWMgYXNzZXQAAAARZ2V0X3ZhdWx0X2JhbGFuY2UAAAAAAAABAAAAAAAAAAVhc3NldAAAAAAAABMAAAABAAAACw==",
        "AAAAAAAAACxDaGVjayBpZiBmdW5kcyBhcmUgbG9ja2VkIGZvciBhIHNwZWNpZmljIGJldAAAABFpc19sb2NrZWRfZm9yX2JldAAAAAAAAAMAAAAAAAAACG1hdGNoX2lkAAAABgAAAAAAAAAEdXNlcgAAABMAAAAAAAAABWFzc2V0AAAAAAAAEwAAAAEAAAAB",
        "AAAAAAAAACtFbWVyZ2VuY3kgd2l0aGRyYXcgb2YgYW4gYXNzZXQgKHRpbWVsb2NrZWQpAAAAABJlbWVyZ2VuY3lfd2l0aGRyYXcAAAAAAAMAAAAAAAAABWFzc2V0AAAAAAAAEwAAAAAAAAACdG8AAAAAABMAAAAAAAAABmFtb3VudAAAAAAACwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAClZhdWx0RXJyb3IAAA==",
        "AAAAAQAAAAAAAAAAAAAAEFBlbmRpbmdBZG1pbkRhdGEAAAACAAAAAAAAAAluZXdfYWRtaW4AAAAAAAATAAAAAAAAAA1wZW5kaW5nX3VudGlsAAAAAAAABg==",
        "AAAAAQAAAAAAAAAAAAAAEVBlbmRpbmdXaXRoZHJhd2FsAAAAAAAAAwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAA1wZW5kaW5nX3VudGlsAAAAAAAABgAAAAAAAAACdG8AAAAAABM=",
        "AAAAAAAAADNVcGRhdGUgdGhlIGF1dGhvcml6ZWQgYmV0dGluZyBjb250cmFjdCAoYWRtaW4gb25seSkAAAAAFHNldF9iZXR0aW5nX2NvbnRyYWN0AAAAAQAAAAAAAAAUbmV3X2JldHRpbmdfY29udHJhY3QAAAATAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANUGxhdGZvcm1FcnJvcgAAAA==",
        "AAAAAAAAACVDYW5jZWwgYSBwZW5kaW5nIGVtZXJnZW5jeSB3aXRoZHJhd2FsAAAAAAAAGWNhbmNlbF9lbWVyZ2VuY3lfd2l0aGRyYXcAAAAAAAABAAAAAAAAAAVhc3NldAAAAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAAApWYXVsdEVycm9yAAA=",
        "AAAAAQAAAEhTdGFuZGFyZGl6ZWQgdHJhY2tpbmcgZGF0YSBjb25maWd1cmF0aW9uIGZvciBpbnRlcmFjdGl2ZSBiZXR0aW5nIG1hdGNoZXMAAAAAAAAADU1hdGNoTWV0YWRhdGEAAAAAAAAFAAAAAAAAAAthc3NldF90b2tlbgAAAAATAAAAAAAAAAhtYXRjaF9pZAAAAAYAAAAAAAAACnBsYXllcl9vbmUAAAAAABMAAAAAAAAACnBsYXllcl90d28AAAAAABMAAAAAAAAACnRvdGFsX3Bvb2wAAAAAAAs=",
        "AAAABAAAAFBDZW50cmFsaXplZCBwbGF0Zm9ybSBlcnJvciBjb2RlcyBtYXBwZWQgY2xlYW5seSBhY3Jvc3MgYWxsIGNoaWxkIGNvbnRyYWN0IHNjb3BlcwAAAAAAAAANUGxhdGZvcm1FcnJvcgAAAAAAAAcAAAAAAAAADUludGVybmFsRXJyb3IAAAAAAAABAAAAAAAAAAxVbmF1dGhvcml6ZWQAAAACAAAAAAAAAA1JbnZhbGlkQW1vdW50AAAAAAAAAwAAAAAAAAAPRXhwaXJlZERlYWRsaW5lAAAAAAQAAAAAAAAACE92ZXJmbG93AAAABQAAALFDYWxsZXIgaG9sZHMgZmV3ZXIgdG9rZW5zIHRoYW4gdGhlIHJlcXVlc3RlZCBkZWJpdC4KRW1pdHRlZCBieSBgcmVuYWlzc2FuY2UtYmV0dGluZ2AgYHBsYWNlX2JldGAgdG8gZW5mb3JjZSB0aGUKYWNjZXB0YW5jZSBjcml0ZXJpb24gdGhhdCBiZXRzIG9ubHkgc2V0dGxlIGFnYWluc3QgcmVhbCBiYWxhbmNlcy4AAAAAAAATSW5zdWZmaWNpZW50QmFsYW5jZQAAAAAGAAAAAAAAAAZQYXVzZWQAAAAAAAc=" ]),
      options
    )
  }
  public readonly fromJSON = {
    payout: this.txFromJSON<Result<void>>,
        deposit: this.txFromJSON<Result<void>>,
        withdraw: this.txFromJSON<Result<void>>,
        set_admin: this.txFromJSON<Result<void>>,
        initialize: this.txFromJSON<Result<void>>,
        lock_for_bet: this.txFromJSON<Result<void>>,
        recover_token: this.txFromJSON<Result<void>>,
        cancel_set_admin: this.txFromJSON<Result<void>>,
        get_user_balance: this.txFromJSON<UserBalance>,
        get_vault_balance: this.txFromJSON<i128>,
        is_locked_for_bet: this.txFromJSON<boolean>,
        emergency_withdraw: this.txFromJSON<Result<void>>,
        set_betting_contract: this.txFromJSON<Result<void>>,
        cancel_emergency_withdraw: this.txFromJSON<Result<void>>
  }
}