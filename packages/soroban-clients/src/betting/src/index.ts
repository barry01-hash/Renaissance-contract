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





/**
 * A single user's bet on a single match.
 */
export interface Bet {
  amount: i128;
  /**
 * Set once winnings have been paid out or the bet has been refunded.
 */
claimed: boolean;
  match_id: u64;
  outcome: Outcome;
  /**
 * Cached payout — 0 until claimed/refunded.
 */
payout: i128;
  token: string;
  user: string;
}


/**
 * A registered football match waiting to be settled.
 */
export interface Match {
  /**
 * Unix timestamp (seconds). After this point, `refund_bet` becomes
 * available if the oracle has not yet settled.
 */
deadline: u64;
  match_id: u64;
  /**
 * Authorized oracle that can settle this match.
 */
oracle: string;
  /**
 * Final outcome — populated by `settle_bet`.
 */
outcome: Option<u32>;
  /**
 * Set to `true` by the oracle in `settle_bet`.
 */
settled: boolean;
  /**
 * Token contract used to place and pay bets (XLM or custom SAC).
 */
token: string;
}

/**
 * Storage namespace. Admin lives in instance storage (singleton),
 * match descriptors and stats live in instance storage keyed by match id,
 * and bets live in persistent storage keyed by (match id, user).
 */
export type DataKey = {tag: "Admin", values: void} | {tag: "Paused", values: void} | {tag: "PlatformAdmin", values: void} | {tag: "SecurityAdmin", values: void} | {tag: "TreasuryAdmin", values: void} | {tag: "UpgradeApprovals", values: void} | {tag: "WasmHash", values: void} | {tag: "Match", values: readonly [u64]} | {tag: "Stats", values: readonly [u64]} | {tag: "Bet", values: readonly [u64, string]};

/**
 * Possible football match outcomes for win / draw / loss betting.
 * `#[repr(u32)]` is required so `Outcome as u32` produces stable discriminants
 * in pure-Rust code paths (the SDK will additionally tag values across the
 * host boundary, but ordinary Rust arithmetic on outcome indices relies on
 * `repr(u32)`).
 */
export type Outcome = {tag: "HomeWin", values: void} | {tag: "Draw", values: void} | {tag: "AwayWin", values: void};


/**
 * Aggregated match statistics used by `claim_payout` to compute payouts
 * without iterating every `Bet` row.
 */
export interface MatchStats {
  /**
 * All bettors on this match — purely informational; payouts
 * are resolved lazily per-bettor in `claim_payout`.
 */
bettors: Array<string>;
  match_id: u64;
  /**
 * Per-outcome pool totals indexed by `Outcome::index()`:
 * 0 = HomeWin, 1 = Draw, 2 = AwayWin.
 */
pools: Array<i128>;
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
   * Construct and simulate a pause transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  pause: (options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_bet transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Look up a user's bet on a match. Returns `None` if the bet does
   * not exist.
   */
  get_bet: ({user, match_id}: {user: string, match_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Option<Bet>>>

  /**
   * Construct and simulate a unpause transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  unpause: (options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  upgrade: ({caller, new_wasm_hash}: {caller: string, new_wasm_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_match transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Look up a match descriptor. Returns `None` if the match id is
   * not registered.
   */
  get_match: ({match_id}: {match_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Option<Match>>>

  /**
   * Construct and simulate a is_paused transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  is_paused: (options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a place_bet transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Stake `amount` tokens on `outcome` for `match_id`.
   * 
   * Validates that:
   * - `amount > 0`
   * - The match exists, is unsettled, and the betting window is still open
   * - `user` holds at least `amount` of the match's settlement token
   * 
   * Then transfers `amount` from `user` into the contract and records the
   * bet + per-outcome pool statistics. Emits `BetPlaced`.
   */
  place_bet: ({user, match_id, outcome, amount}: {user: string, match_id: u64, outcome: Outcome, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a initialize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * One-time bootstrap of the contract admin. Subsequent calls fail with
   * `PlatformError::Unauthorized` to guarantee a stable admin identity.
   */
  initialize: ({admin}: {admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a refund_bet transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Refund the user's full bet amount if the match has passed its deadline
   * without being settled. Idempotent per `(user, match_id)`.
   * Emits `BetRefunded`.
   */
  refund_bet: ({user, match_id}: {user: string, match_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a settle_bet transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Oracle records the canonical `outcome` of `match_id`. Settlement is
   * intentionally **constant-time**: only the canonical outcome flag and
   * per-pool totals are committed. Winners pull their pro-rata share via
   * `claim_payout` to keep this entry-point within Soroban's CPU budget,
   * regardless of how many bettors exist.
   */
  settle_bet: ({oracle, match_id, outcome}: {oracle: string, match_id: u64, outcome: Outcome}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a set_oracle transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Replace the oracle of an unsettled match. Admin only.
   */
  set_oracle: ({match_id, new_oracle}: {match_id: u64, new_oracle: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a claim_payout transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Pull the winner's pro-rata share of the losers' pool for a settled
   * match.
   * 
   * Payout (parimutuel, integer truncation toward zero):
   * 
   * ```text
   * winning_pool = pools[outcome]
   * losing_pool  = total_pool - winning_pool
   * share        = bet.amount * losing_pool / winning_pool
   * payout       = bet.amount + share
   * ```
   * 
   * Emits `BetClaimed` with the paid amount.
   */
  claim_payout: ({user, match_id}: {user: string, match_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a register_match transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Define a new match id with its authorized oracle, settlement token,
   * and betting deadline. Admin only.
   */
  register_match: ({match_id, oracle, token, deadline}: {match_id: u64, oracle: string, token: string, deadline: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

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
        "AAAAAQAAACZBIHNpbmdsZSB1c2VyJ3MgYmV0IG9uIGEgc2luZ2xlIG1hdGNoLgAAAAAAAAAAAANCZXQAAAAABwAAAAAAAAAGYW1vdW50AAAAAAALAAAAQlNldCBvbmNlIHdpbm5pbmdzIGhhdmUgYmVlbiBwYWlkIG91dCBvciB0aGUgYmV0IGhhcyBiZWVuIHJlZnVuZGVkLgAAAAAAB2NsYWltZWQAAAAAAQAAAAAAAAAIbWF0Y2hfaWQAAAAGAAAAAAAAAAdvdXRjb21lAAAAB9AAAAAHT3V0Y29tZQAAAAArQ2FjaGVkIHBheW91dCDigJQgMCB1bnRpbCBjbGFpbWVkL3JlZnVuZGVkLgAAAAAGcGF5b3V0AAAAAAALAAAAAAAAAAV0b2tlbgAAAAAAABMAAAAAAAAABHVzZXIAAAAT",
        "AAAAAAAAAEpMb29rIHVwIGEgdXNlcidzIGJldCBvbiBhIG1hdGNoLiBSZXR1cm5zIGBOb25lYCBpZiB0aGUgYmV0IGRvZXMKbm90IGV4aXN0LgAAAAAAB2dldF9iZXQAAAAAAgAAAAAAAAAEdXNlcgAAABMAAAAAAAAACG1hdGNoX2lkAAAABgAAAAEAAAPoAAAH0AAAAANCZXQA",
        "AAAAAAAAAAAAAAAHdW5wYXVzZQAAAAAAAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANUGxhdGZvcm1FcnJvcgAAAA==",
        "AAAAAAAAAAAAAAAHdXBncmFkZQAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAADW5ld193YXNtX2hhc2gAAAAAAAPuAAAAIAAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADVBsYXRmb3JtRXJyb3IAAAA=",
        "AAAAAQAAADJBIHJlZ2lzdGVyZWQgZm9vdGJhbGwgbWF0Y2ggd2FpdGluZyB0byBiZSBzZXR0bGVkLgAAAAAAAAAAAAVNYXRjaAAAAAAAAAYAAABtVW5peCB0aW1lc3RhbXAgKHNlY29uZHMpLiBBZnRlciB0aGlzIHBvaW50LCBgcmVmdW5kX2JldGAgYmVjb21lcwphdmFpbGFibGUgaWYgdGhlIG9yYWNsZSBoYXMgbm90IHlldCBzZXR0bGVkLgAAAAAAAAhkZWFkbGluZQAAAAYAAAAAAAAACG1hdGNoX2lkAAAABgAAAC1BdXRob3JpemVkIG9yYWNsZSB0aGF0IGNhbiBzZXR0bGUgdGhpcyBtYXRjaC4AAAAAAAAGb3JhY2xlAAAAAAATAAAALEZpbmFsIG91dGNvbWUg4oCUIHBvcHVsYXRlZCBieSBgc2V0dGxlX2JldGAuAAAAB291dGNvbWUAAAAD6AAAAAQAAAAsU2V0IHRvIGB0cnVlYCBieSB0aGUgb3JhY2xlIGluIGBzZXR0bGVfYmV0YC4AAAAHc2V0dGxlZAAAAAABAAAAPlRva2VuIGNvbnRyYWN0IHVzZWQgdG8gcGxhY2UgYW5kIHBheSBiZXRzIChYTE0gb3IgY3VzdG9tIFNBQykuAAAAAAAFdG9rZW4AAAAAAAAT",
        "AAAAAAAAAE1Mb29rIHVwIGEgbWF0Y2ggZGVzY3JpcHRvci4gUmV0dXJucyBgTm9uZWAgaWYgdGhlIG1hdGNoIGlkIGlzCm5vdCByZWdpc3RlcmVkLgAAAAAAAAlnZXRfbWF0Y2gAAAAAAAABAAAAAAAAAAhtYXRjaF9pZAAAAAYAAAABAAAD6AAAB9AAAAAFTWF0Y2gAAAA=",
        "AAAAAAAAAAAAAAAJaXNfcGF1c2VkAAAAAAAAAAAAAAEAAAAB",
        "AAAAAAAAAVdTdGFrZSBgYW1vdW50YCB0b2tlbnMgb24gYG91dGNvbWVgIGZvciBgbWF0Y2hfaWRgLgoKVmFsaWRhdGVzIHRoYXQ6Ci0gYGFtb3VudCA+IDBgCi0gVGhlIG1hdGNoIGV4aXN0cywgaXMgdW5zZXR0bGVkLCBhbmQgdGhlIGJldHRpbmcgd2luZG93IGlzIHN0aWxsIG9wZW4KLSBgdXNlcmAgaG9sZHMgYXQgbGVhc3QgYGFtb3VudGAgb2YgdGhlIG1hdGNoJ3Mgc2V0dGxlbWVudCB0b2tlbgoKVGhlbiB0cmFuc2ZlcnMgYGFtb3VudGAgZnJvbSBgdXNlcmAgaW50byB0aGUgY29udHJhY3QgYW5kIHJlY29yZHMgdGhlCmJldCArIHBlci1vdXRjb21lIHBvb2wgc3RhdGlzdGljcy4gRW1pdHMgYEJldFBsYWNlZGAuAAAAAAlwbGFjZV9iZXQAAAAAAAAEAAAAAAAAAAR1c2VyAAAAEwAAAAAAAAAIbWF0Y2hfaWQAAAAGAAAAAAAAAAdvdXRjb21lAAAAB9AAAAAHT3V0Y29tZQAAAAAAAAAABmFtb3VudAAAAAAACwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADVBsYXRmb3JtRXJyb3IAAAA=",
        "AAAAAgAAAMZTdG9yYWdlIG5hbWVzcGFjZS4gQWRtaW4gbGl2ZXMgaW4gaW5zdGFuY2Ugc3RvcmFnZSAoc2luZ2xldG9uKSwKbWF0Y2ggZGVzY3JpcHRvcnMgYW5kIHN0YXRzIGxpdmUgaW4gaW5zdGFuY2Ugc3RvcmFnZSBrZXllZCBieSBtYXRjaCBpZCwKYW5kIGJldHMgbGl2ZSBpbiBwZXJzaXN0ZW50IHN0b3JhZ2Uga2V5ZWQgYnkgKG1hdGNoIGlkLCB1c2VyKS4AAAAAAAAAAAAHRGF0YUtleQAAAAAKAAAAAAAAAAAAAAAFQWRtaW4AAAAAAAAAAAAAAAAAAAZQYXVzZWQAAAAAAAAAAAAAAAAADVBsYXRmb3JtQWRtaW4AAAAAAAAAAAAAAAAAAA1TZWN1cml0eUFkbWluAAAAAAAAAAAAAAAAAAANVHJlYXN1cnlBZG1pbgAAAAAAAAAAAAAAAAAAEFVwZ3JhZGVBcHByb3ZhbHMAAAAAAAAAAAAAAAhXYXNtSGFzaAAAAAEAAAAAAAAABU1hdGNoAAAAAAAAAQAAAAYAAAABAAAAAAAAAAVTdGF0cwAAAAAAAAEAAAAGAAAAAQAAAAAAAAADQmV0AAAAAAIAAAAGAAAAEw==",
        "AAAAAgAAASxQb3NzaWJsZSBmb290YmFsbCBtYXRjaCBvdXRjb21lcyBmb3Igd2luIC8gZHJhdyAvIGxvc3MgYmV0dGluZy4KYCNbcmVwcih1MzIpXWAgaXMgcmVxdWlyZWQgc28gYE91dGNvbWUgYXMgdTMyYCBwcm9kdWNlcyBzdGFibGUgZGlzY3JpbWluYW50cwppbiBwdXJlLVJ1c3QgY29kZSBwYXRocyAodGhlIFNESyB3aWxsIGFkZGl0aW9uYWxseSB0YWcgdmFsdWVzIGFjcm9zcyB0aGUKaG9zdCBib3VuZGFyeSwgYnV0IG9yZGluYXJ5IFJ1c3QgYXJpdGhtZXRpYyBvbiBvdXRjb21lIGluZGljZXMgcmVsaWVzIG9uCmByZXByKHUzMilgKS4AAAAAAAAAB091dGNvbWUAAAAAAwAAAAAAAAAAAAAAB0hvbWVXaW4AAAAAAAAAAAAAAAAERHJhdwAAAAAAAAAAAAAAB0F3YXlXaW4A",
        "AAAAAAAAAIhPbmUtdGltZSBib290c3RyYXAgb2YgdGhlIGNvbnRyYWN0IGFkbWluLiBTdWJzZXF1ZW50IGNhbGxzIGZhaWwgd2l0aApgUGxhdGZvcm1FcnJvcjo6VW5hdXRob3JpemVkYCB0byBndWFyYW50ZWUgYSBzdGFibGUgYWRtaW4gaWRlbnRpdHkuAAAACmluaXRpYWxpemUAAAAAAAEAAAAAAAAABWFkbWluAAAAAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADVBsYXRmb3JtRXJyb3IAAAA=",
        "AAAAAAAAAJVSZWZ1bmQgdGhlIHVzZXIncyBmdWxsIGJldCBhbW91bnQgaWYgdGhlIG1hdGNoIGhhcyBwYXNzZWQgaXRzIGRlYWRsaW5lCndpdGhvdXQgYmVpbmcgc2V0dGxlZC4gSWRlbXBvdGVudCBwZXIgYCh1c2VyLCBtYXRjaF9pZClgLgpFbWl0cyBgQmV0UmVmdW5kZWRgLgAAAAAAAApyZWZ1bmRfYmV0AAAAAAACAAAAAAAAAAR1c2VyAAAAEwAAAAAAAAAIbWF0Y2hfaWQAAAAGAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANUGxhdGZvcm1FcnJvcgAAAA==",
        "AAAAAAAAAThPcmFjbGUgcmVjb3JkcyB0aGUgY2Fub25pY2FsIGBvdXRjb21lYCBvZiBgbWF0Y2hfaWRgLiBTZXR0bGVtZW50IGlzCmludGVudGlvbmFsbHkgKipjb25zdGFudC10aW1lKio6IG9ubHkgdGhlIGNhbm9uaWNhbCBvdXRjb21lIGZsYWcgYW5kCnBlci1wb29sIHRvdGFscyBhcmUgY29tbWl0dGVkLiBXaW5uZXJzIHB1bGwgdGhlaXIgcHJvLXJhdGEgc2hhcmUgdmlhCmBjbGFpbV9wYXlvdXRgIHRvIGtlZXAgdGhpcyBlbnRyeS1wb2ludCB3aXRoaW4gU29yb2JhbidzIENQVSBidWRnZXQsCnJlZ2FyZGxlc3Mgb2YgaG93IG1hbnkgYmV0dG9ycyBleGlzdC4AAAAKc2V0dGxlX2JldAAAAAAAAwAAAAAAAAAGb3JhY2xlAAAAAAATAAAAAAAAAAhtYXRjaF9pZAAAAAYAAAAAAAAAB291dGNvbWUAAAAH0AAAAAdPdXRjb21lAAAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADVBsYXRmb3JtRXJyb3IAAAA=",
        "AAAAAAAAADVSZXBsYWNlIHRoZSBvcmFjbGUgb2YgYW4gdW5zZXR0bGVkIG1hdGNoLiBBZG1pbiBvbmx5LgAAAAAAAApzZXRfb3JhY2xlAAAAAAACAAAAAAAAAAhtYXRjaF9pZAAAAAYAAAAAAAAACm5ld19vcmFjbGUAAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA1QbGF0Zm9ybUVycm9yAAAA",
        "AAAAAAAAAVZQdWxsIHRoZSB3aW5uZXIncyBwcm8tcmF0YSBzaGFyZSBvZiB0aGUgbG9zZXJzJyBwb29sIGZvciBhIHNldHRsZWQKbWF0Y2guCgpQYXlvdXQgKHBhcmltdXR1ZWwsIGludGVnZXIgdHJ1bmNhdGlvbiB0b3dhcmQgemVybyk6CgpgYGB0ZXh0Cndpbm5pbmdfcG9vbCA9IHBvb2xzW291dGNvbWVdCmxvc2luZ19wb29sICA9IHRvdGFsX3Bvb2wgLSB3aW5uaW5nX3Bvb2wKc2hhcmUgICAgICAgID0gYmV0LmFtb3VudCAqIGxvc2luZ19wb29sIC8gd2lubmluZ19wb29sCnBheW91dCAgICAgICA9IGJldC5hbW91bnQgKyBzaGFyZQpgYGAKCkVtaXRzIGBCZXRDbGFpbWVkYCB3aXRoIHRoZSBwYWlkIGFtb3VudC4AAAAAAAxjbGFpbV9wYXlvdXQAAAACAAAAAAAAAAR1c2VyAAAAEwAAAAAAAAAIbWF0Y2hfaWQAAAAGAAAAAQAAA+kAAAALAAAH0AAAAA1QbGF0Zm9ybUVycm9yAAAA",
        "AAAAAQAAAGhBZ2dyZWdhdGVkIG1hdGNoIHN0YXRpc3RpY3MgdXNlZCBieSBgY2xhaW1fcGF5b3V0YCB0byBjb21wdXRlIHBheW91dHMKd2l0aG91dCBpdGVyYXRpbmcgZXZlcnkgYEJldGAgcm93LgAAAAAAAAAKTWF0Y2hTdGF0cwAAAAAAAwAAAG1BbGwgYmV0dG9ycyBvbiB0aGlzIG1hdGNoIOKAlCBwdXJlbHkgaW5mb3JtYXRpb25hbDsgcGF5b3V0cwphcmUgcmVzb2x2ZWQgbGF6aWx5IHBlci1iZXR0b3IgaW4gYGNsYWltX3BheW91dGAuAAAAAAAAB2JldHRvcnMAAAAD6gAAABMAAAAAAAAACG1hdGNoX2lkAAAABgAAAFpQZXItb3V0Y29tZSBwb29sIHRvdGFscyBpbmRleGVkIGJ5IGBPdXRjb21lOjppbmRleCgpYDoKMCA9IEhvbWVXaW4sIDEgPSBEcmF3LCAyID0gQXdheVdpbi4AAAAAAAVwb29scwAAAAAAA+oAAAAL",
        "AAAAAAAAAGVEZWZpbmUgYSBuZXcgbWF0Y2ggaWQgd2l0aCBpdHMgYXV0aG9yaXplZCBvcmFjbGUsIHNldHRsZW1lbnQgdG9rZW4sCmFuZCBiZXR0aW5nIGRlYWRsaW5lLiBBZG1pbiBvbmx5LgAAAAAAAA5yZWdpc3Rlcl9tYXRjaAAAAAAABAAAAAAAAAAIbWF0Y2hfaWQAAAAGAAAAAAAAAAZvcmFjbGUAAAAAABMAAAAAAAAABXRva2VuAAAAAAAAEwAAAAAAAAAIZGVhZGxpbmUAAAAGAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANUGxhdGZvcm1FcnJvcgAAAA==",
        "AAAAAQAAAEhTdGFuZGFyZGl6ZWQgdHJhY2tpbmcgZGF0YSBjb25maWd1cmF0aW9uIGZvciBpbnRlcmFjdGl2ZSBiZXR0aW5nIG1hdGNoZXMAAAAAAAAADU1hdGNoTWV0YWRhdGEAAAAAAAAFAAAAAAAAAAthc3NldF90b2tlbgAAAAATAAAAAAAAAAhtYXRjaF9pZAAAAAYAAAAAAAAACnBsYXllcl9vbmUAAAAAABMAAAAAAAAACnBsYXllcl90d28AAAAAABMAAAAAAAAACnRvdGFsX3Bvb2wAAAAAAAs=",
        "AAAABAAAAFBDZW50cmFsaXplZCBwbGF0Zm9ybSBlcnJvciBjb2RlcyBtYXBwZWQgY2xlYW5seSBhY3Jvc3MgYWxsIGNoaWxkIGNvbnRyYWN0IHNjb3BlcwAAAAAAAAANUGxhdGZvcm1FcnJvcgAAAAAAAAcAAAAAAAAADUludGVybmFsRXJyb3IAAAAAAAABAAAAAAAAAAxVbmF1dGhvcml6ZWQAAAACAAAAAAAAAA1JbnZhbGlkQW1vdW50AAAAAAAAAwAAAAAAAAAPRXhwaXJlZERlYWRsaW5lAAAAAAQAAAAAAAAACE92ZXJmbG93AAAABQAAALFDYWxsZXIgaG9sZHMgZmV3ZXIgdG9rZW5zIHRoYW4gdGhlIHJlcXVlc3RlZCBkZWJpdC4KRW1pdHRlZCBieSBgcmVuYWlzc2FuY2UtYmV0dGluZ2AgYHBsYWNlX2JldGAgdG8gZW5mb3JjZSB0aGUKYWNjZXB0YW5jZSBjcml0ZXJpb24gdGhhdCBiZXRzIG9ubHkgc2V0dGxlIGFnYWluc3QgcmVhbCBiYWxhbmNlcy4AAAAAAAATSW5zdWZmaWNpZW50QmFsYW5jZQAAAAAGAAAAAAAAAAZQYXVzZWQAAAAAAAc=" ]),
      options
    )
  }
  public readonly fromJSON = {
    pause: this.txFromJSON<Result<void>>,
        get_bet: this.txFromJSON<Option<Bet>>,
        unpause: this.txFromJSON<Result<void>>,
        upgrade: this.txFromJSON<Result<void>>,
        get_match: this.txFromJSON<Option<Match>>,
        is_paused: this.txFromJSON<boolean>,
        place_bet: this.txFromJSON<Result<void>>,
        initialize: this.txFromJSON<Result<void>>,
        refund_bet: this.txFromJSON<Result<void>>,
        settle_bet: this.txFromJSON<Result<void>>,
        set_oracle: this.txFromJSON<Result<void>>,
        claim_payout: this.txFromJSON<Result<i128>>,
        register_match: this.txFromJSON<Result<void>>
  }
}