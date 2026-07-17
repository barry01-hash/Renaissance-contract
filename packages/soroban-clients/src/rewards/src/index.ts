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




export type DataKey = {tag: "Admin", values: void} | {tag: "Paused", values: void} | {tag: "PlatformAdmin", values: void} | {tag: "SecurityAdmin", values: void} | {tag: "TreasuryAdmin", values: void} | {tag: "UpgradeApprovals", values: void} | {tag: "WasmHash", values: void} | {tag: "Balance", values: readonly [string]};


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
   * Construct and simulate a pause transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  pause: (options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a unpause transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  unpause: (options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  upgrade: ({caller, new_wasm_hash}: {caller: string, new_wasm_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Return the currently configured platform admin, if any.
   */
  get_admin: (options?: MethodOptions) => Promise<AssembledTransaction<Option<string>>>

  /**
   * Construct and simulate a is_paused transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  is_paused: (options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a initialize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * One-time initialisation: designate the platform admin that will be the
   * sole address able to award loyalty points.
   * 
   * Returns `InternalError` if the contract has already been initialised.
   */
  initialize: ({admin}: {admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_balance transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Return the loyalty point balance held by `user`. New/unknown users get 0.
   */
  get_balance: ({user}: {user: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a award_points transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Award fan loyalty points for an engagement activity.
   * 
   * Authorisation: only the configured platform `admin` may call this.
   * `amount` must be strictly positive; the resulting balance must not
   * overflow `i128::MAX`.
   */
  award_points: ({user, amount, reason}: {user: string, amount: i128, reason: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a redeem_points transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Redeem loyalty points for an exclusive reward (NFT drop, perk, ...).
   * 
   * Authorisation: the `user` whose balance is being spent.
   * `cost` must be strictly positive and ≤ the current balance.
   */
  redeem_points: ({user, cost, reward_id}: {user: string, cost: i128, reward_id: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a transfer_points transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Transfer loyalty points from `from` to `to` (optional gifting path).
   * 
   * Authorisation: the `from` address. `amount` must be strictly positive,
   * strictly less than or equal to `from`'s balance, and `from != to` to
   * avoid pointless storage churn.
   */
  transfer_points: ({from, to, amount}: {from: string, to: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

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
      new ContractSpec([ "AAAAAAAAAAAAAAAFcGF1c2UAAAAAAAAAAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANUGxhdGZvcm1FcnJvcgAAAA==",
        "AAAAAAAAAAAAAAAHdW5wYXVzZQAAAAAAAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANUGxhdGZvcm1FcnJvcgAAAA==",
        "AAAAAAAAAAAAAAAHdXBncmFkZQAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAADW5ld193YXNtX2hhc2gAAAAAAAPuAAAAIAAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADVBsYXRmb3JtRXJyb3IAAAA=",
        "AAAAAAAAADdSZXR1cm4gdGhlIGN1cnJlbnRseSBjb25maWd1cmVkIHBsYXRmb3JtIGFkbWluLCBpZiBhbnkuAAAAAAlnZXRfYWRtaW4AAAAAAAAAAAAAAQAAA+gAAAAT",
        "AAAAAAAAAAAAAAAJaXNfcGF1c2VkAAAAAAAAAAAAAAEAAAAB",
        "AAAAAgAAAAAAAAAAAAAAB0RhdGFLZXkAAAAACAAAAAAAAABMU3RvcmVkIGluIGluc3RhbmNlIHN0b3JhZ2UuIFRoZSBwbGF0Zm9ybSBhZGRyZXNzIGF1dGhvcmlzZWQgdG8gYXdhcmQgcG9pbnRzLgAAAAVBZG1pbgAAAAAAAAAAAAAAAAAABlBhdXNlZAAAAAAAAAAAAAAAAAANUGxhdGZvcm1BZG1pbgAAAAAAAAAAAAAAAAAADVNlY3VyaXR5QWRtaW4AAAAAAAAAAAAAAAAAAA1UcmVhc3VyeUFkbWluAAAAAAAAAAAAAAAAAAAQVXBncmFkZUFwcHJvdmFscwAAAAAAAAAAAAAACFdhc21IYXNoAAAAAQAAADlTdG9yZWQgaW4gcGVyc2lzdGVudCBzdG9yYWdlLiBPbmUgZW50cnkgcGVyIHVzZXIgYWRkcmVzcy4AAAAAAAAHQmFsYW5jZQAAAAABAAAAEw==",
        "AAAAAAAAALhPbmUtdGltZSBpbml0aWFsaXNhdGlvbjogZGVzaWduYXRlIHRoZSBwbGF0Zm9ybSBhZG1pbiB0aGF0IHdpbGwgYmUgdGhlCnNvbGUgYWRkcmVzcyBhYmxlIHRvIGF3YXJkIGxveWFsdHkgcG9pbnRzLgoKUmV0dXJucyBgSW50ZXJuYWxFcnJvcmAgaWYgdGhlIGNvbnRyYWN0IGhhcyBhbHJlYWR5IGJlZW4gaW5pdGlhbGlzZWQuAAAACmluaXRpYWxpemUAAAAAAAEAAAAAAAAABWFkbWluAAAAAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADVBsYXRmb3JtRXJyb3IAAAA=",
        "AAAAAAAAAElSZXR1cm4gdGhlIGxveWFsdHkgcG9pbnQgYmFsYW5jZSBoZWxkIGJ5IGB1c2VyYC4gTmV3L3Vua25vd24gdXNlcnMgZ2V0IDAuAAAAAAAAC2dldF9iYWxhbmNlAAAAAAEAAAAAAAAABHVzZXIAAAATAAAAAQAAAAs=",
        "AAAAAAAAANFBd2FyZCBmYW4gbG95YWx0eSBwb2ludHMgZm9yIGFuIGVuZ2FnZW1lbnQgYWN0aXZpdHkuCgpBdXRob3Jpc2F0aW9uOiBvbmx5IHRoZSBjb25maWd1cmVkIHBsYXRmb3JtIGBhZG1pbmAgbWF5IGNhbGwgdGhpcy4KYGFtb3VudGAgbXVzdCBiZSBzdHJpY3RseSBwb3NpdGl2ZTsgdGhlIHJlc3VsdGluZyBiYWxhbmNlIG11c3Qgbm90Cm92ZXJmbG93IGBpMTI4OjpNQVhgLgAAAAAAAAxhd2FyZF9wb2ludHMAAAADAAAAAAAAAAR1c2VyAAAAEwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAZyZWFzb24AAAAAABEAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA1QbGF0Zm9ybUVycm9yAAAA",
        "AAAAAAAAALtSZWRlZW0gbG95YWx0eSBwb2ludHMgZm9yIGFuIGV4Y2x1c2l2ZSByZXdhcmQgKE5GVCBkcm9wLCBwZXJrLCAuLi4pLgoKQXV0aG9yaXNhdGlvbjogdGhlIGB1c2VyYCB3aG9zZSBiYWxhbmNlIGlzIGJlaW5nIHNwZW50LgpgY29zdGAgbXVzdCBiZSBzdHJpY3RseSBwb3NpdGl2ZSBhbmQg4omkIHRoZSBjdXJyZW50IGJhbGFuY2UuAAAAAA1yZWRlZW1fcG9pbnRzAAAAAAAAAwAAAAAAAAAEdXNlcgAAABMAAAAAAAAABGNvc3QAAAALAAAAAAAAAAlyZXdhcmRfaWQAAAAAAAARAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANUGxhdGZvcm1FcnJvcgAAAA==",
        "AAAAAAAAAPBUcmFuc2ZlciBsb3lhbHR5IHBvaW50cyBmcm9tIGBmcm9tYCB0byBgdG9gIChvcHRpb25hbCBnaWZ0aW5nIHBhdGgpLgoKQXV0aG9yaXNhdGlvbjogdGhlIGBmcm9tYCBhZGRyZXNzLiBgYW1vdW50YCBtdXN0IGJlIHN0cmljdGx5IHBvc2l0aXZlLApzdHJpY3RseSBsZXNzIHRoYW4gb3IgZXF1YWwgdG8gYGZyb21gJ3MgYmFsYW5jZSwgYW5kIGBmcm9tICE9IHRvYCB0bwphdm9pZCBwb2ludGxlc3Mgc3RvcmFnZSBjaHVybi4AAAAPdHJhbnNmZXJfcG9pbnRzAAAAAAMAAAAAAAAABGZyb20AAAATAAAAAAAAAAJ0bwAAAAAAEwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANUGxhdGZvcm1FcnJvcgAAAA==",
        "AAAAAQAAAEhTdGFuZGFyZGl6ZWQgdHJhY2tpbmcgZGF0YSBjb25maWd1cmF0aW9uIGZvciBpbnRlcmFjdGl2ZSBiZXR0aW5nIG1hdGNoZXMAAAAAAAAADU1hdGNoTWV0YWRhdGEAAAAAAAAFAAAAAAAAAAthc3NldF90b2tlbgAAAAATAAAAAAAAAAhtYXRjaF9pZAAAAAYAAAAAAAAACnBsYXllcl9vbmUAAAAAABMAAAAAAAAACnBsYXllcl90d28AAAAAABMAAAAAAAAACnRvdGFsX3Bvb2wAAAAAAAs=",
        "AAAABAAAAFBDZW50cmFsaXplZCBwbGF0Zm9ybSBlcnJvciBjb2RlcyBtYXBwZWQgY2xlYW5seSBhY3Jvc3MgYWxsIGNoaWxkIGNvbnRyYWN0IHNjb3BlcwAAAAAAAAANUGxhdGZvcm1FcnJvcgAAAAAAAAcAAAAAAAAADUludGVybmFsRXJyb3IAAAAAAAABAAAAAAAAAAxVbmF1dGhvcml6ZWQAAAACAAAAAAAAAA1JbnZhbGlkQW1vdW50AAAAAAAAAwAAAAAAAAAPRXhwaXJlZERlYWRsaW5lAAAAAAQAAAAAAAAACE92ZXJmbG93AAAABQAAALFDYWxsZXIgaG9sZHMgZmV3ZXIgdG9rZW5zIHRoYW4gdGhlIHJlcXVlc3RlZCBkZWJpdC4KRW1pdHRlZCBieSBgcmVuYWlzc2FuY2UtYmV0dGluZ2AgYHBsYWNlX2JldGAgdG8gZW5mb3JjZSB0aGUKYWNjZXB0YW5jZSBjcml0ZXJpb24gdGhhdCBiZXRzIG9ubHkgc2V0dGxlIGFnYWluc3QgcmVhbCBiYWxhbmNlcy4AAAAAAAATSW5zdWZmaWNpZW50QmFsYW5jZQAAAAAGAAAAAAAAAAZQYXVzZWQAAAAAAAc=" ]),
      options
    )
  }
  public readonly fromJSON = {
    pause: this.txFromJSON<Result<void>>,
        unpause: this.txFromJSON<Result<void>>,
        upgrade: this.txFromJSON<Result<void>>,
        get_admin: this.txFromJSON<Option<string>>,
        is_paused: this.txFromJSON<boolean>,
        initialize: this.txFromJSON<Result<void>>,
        get_balance: this.txFromJSON<i128>,
        award_points: this.txFromJSON<Result<void>>,
        redeem_points: this.txFromJSON<Result<void>>,
        transfer_points: this.txFromJSON<Result<void>>
  }
}