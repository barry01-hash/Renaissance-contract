#![no_std]

//! `#![no_std]` `renaissance-betting` smart contract.
//!
//! Implements a Soroban-based betting protocol for football match outcomes
//! (`HomeWin` / `Draw` / `AwayWin`) priced in a fungible token
//! (native XLM via the Stellar Asset Contract or any custom SAC token).
//!
//! Settlement is gated through a pre-defined oracle address stored when the
//! match is registered. Winners pull their pro-rata share of the losers'
//! pool via `claim_payout` (pull pattern) instead of receiving an automatic
//! push from `settle_bet`. This keeps `settle_bet` constant-time and avoids
//! the well-known Soroban "push-payment" anti-pattern where an unbounded set
//! of unauthorized transfers in a single transaction can exceed the host
//! machine's CPU / instruction budget.
//!
//! Read API:
//! - `get_bet`  : fetch a single bet by (user, match_id)
//! - `get_match`: fetch a registered match descriptor
//!
//! Write API:
//! - `initialize`     : one-time admin bootstrap
//! - `register_match` : admin defines match id + authorized oracle + token + deadline
//! - `set_oracle`     : admin rotates the oracle for an unsettled match
//! - `place_bet`      : user stakes tokens on an outcome
//! - `settle_bet`     : oracle records the canonical outcome (constant-time)
//! - `refund_bet`     : user pulls a refund after the deadline if no settlement
//! - `claim_payout`   : user pulls parimutuel winnings once the match is settled

use soroban_sdk::{
    contract, contractimpl, contracttype, token, Address, BytesN, Env, Symbol, Vec,
};
use renaissance_core::PlatformError;

// ── Outcomes ──────────────────────────────────────────────────────────────────

/// Possible football match outcomes for win / draw / loss betting.
/// `#[repr(u32)]` is required so `Outcome as u32` produces stable discriminants
/// in pure-Rust code paths (the SDK will additionally tag values across the
/// host boundary, but ordinary Rust arithmetic on outcome indices relies on
/// `repr(u32)`).
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum Outcome {
    HomeWin,
    Draw,
    AwayWin,
}

impl Outcome {
    /// Pool index for the per-outcome `Vec<i128>` tracked in `MatchStats`.
    fn index(self) -> u32 {
        match self {
            Self::HomeWin => 0,
            Self::Draw => 1,
            Self::AwayWin => 2,
        }
    }
}

// ── Storage records ───────────────────────────────────────────────────────────

/// A registered football match waiting to be settled.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Match {
    pub match_id: u64,
    /// Authorized oracle that can settle this match.
    pub oracle: Address,
    /// Token contract used to place and pay bets (XLM or custom SAC).
    pub token: Address,
    /// Unix timestamp (seconds). After this point, `refund_bet` becomes
    /// available if the oracle has not yet settled.
    pub deadline: u64,
    /// Set to `true` by the oracle in `settle_bet`.
    pub settled: bool,
    /// Final outcome — populated by `settle_bet`.
    pub outcome: Option<u32>,
}

/// Aggregated match statistics used by `claim_payout` to compute payouts
/// without iterating every `Bet` row.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchStats {
    pub match_id: u64,
    /// Per-outcome pool totals indexed by `Outcome::index()`:
    /// 0 = HomeWin, 1 = Draw, 2 = AwayWin.
    pub pools: Vec<i128>,
    /// All bettors on this match — purely informational; payouts
    /// are resolved lazily per-bettor in `claim_payout`.
    pub bettors: Vec<Address>,
}

/// A single user's bet on a single match.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bet {
    pub user: Address,
    pub match_id: u64,
    pub outcome: Outcome,
    pub amount: i128,
    pub token: Address,
    /// Set once winnings have been paid out or the bet has been refunded.
    pub claimed: bool,
    /// Cached payout — 0 until claimed/refunded.
    pub payout: i128,
}

/// Storage namespace. Admin lives in instance storage (singleton),
/// match descriptors and stats live in instance storage keyed by match id,
/// and bets live in persistent storage keyed by (match id, user).
#[contracttype]
pub enum DataKey {
    Admin,
    Paused,
    PlatformAdmin,
    SecurityAdmin,
    TreasuryAdmin,
    UpgradeApprovals,
    WasmHash,
    Match(u64),
    Stats(u64),
    Bet(u64, Address),
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct RenaissanceBettingContract;

#[contractimpl]
impl RenaissanceBettingContract {
    fn require_admin(env: &Env) -> Result<Address, PlatformError> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(PlatformError::Unauthorized)
    }

    fn ensure_not_paused(env: &Env) -> Result<(), PlatformError> {
        if env.storage().instance().get(&DataKey::Paused).unwrap_or(false) {
            return Err(PlatformError::Paused);
        }
        Ok(())
    }

    // ── Admin / setup ─────────────────────────────────────────────────────────

    /// One-time bootstrap of the contract admin. Subsequent calls fail with
    /// `PlatformError::Unauthorized` to guarantee a stable admin identity.
    pub fn initialize(env: Env, admin: Address) -> Result<(), PlatformError> {
        admin.require_auth();
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(PlatformError::Unauthorized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::PlatformAdmin, &admin);
        env.storage().instance().set(&DataKey::SecurityAdmin, &admin);
        env.storage().instance().set(&DataKey::TreasuryAdmin, &admin);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.events()
            .publish((Symbol::new(&env, "ContractInitialized"),), admin);
        Ok(())
    }

    pub fn upgrade(env: Env, caller: Address, new_wasm_hash: BytesN<32>) -> Result<(), PlatformError> {
        let platform: Address = env
            .storage()
            .instance()
            .get(&DataKey::PlatformAdmin)
            .ok_or(PlatformError::Unauthorized)?;
        let security: Address = env
            .storage()
            .instance()
            .get(&DataKey::SecurityAdmin)
            .ok_or(PlatformError::Unauthorized)?;
        let treasury: Address = env
            .storage()
            .instance()
            .get(&DataKey::TreasuryAdmin)
            .ok_or(PlatformError::Unauthorized)?;
        caller.require_auth();
        if caller != platform && caller != security && caller != treasury {
            return Err(PlatformError::Unauthorized);
        }

        let mut approvals: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::UpgradeApprovals)
            .unwrap_or(Vec::new(&env));
        if approvals.iter().any(|approved| approved == caller) {
            return Err(PlatformError::Unauthorized);
        }
        approvals.push_back(caller.clone());

        if approvals.len() >= 2 {
            env.storage().instance().set(&DataKey::WasmHash, &new_wasm_hash);
            env.deployer().update_current_contract_wasm(new_wasm_hash.clone());
            env.storage().instance().set(&DataKey::UpgradeApprovals, &Vec::<Address>::new(&env));
            env.events()
                .publish((Symbol::new(&env, "Upgraded"),), new_wasm_hash);
        } else {
            env.storage().instance().set(&DataKey::UpgradeApprovals, &approvals);
        }
        Ok(())
    }

    pub fn pause(env: Env) -> Result<(), PlatformError> {
        let admin = Self::require_admin(&env)?;
        admin.require_auth();
        if env.storage().instance().get(&DataKey::Paused).unwrap_or(false) {
            return Ok(());
        }
        env.storage().instance().set(&DataKey::Paused, &true);
        env.events().publish((Symbol::new(&env, "Paused"),), admin);
        Ok(())
    }

    pub fn unpause(env: Env) -> Result<(), PlatformError> {
        let admin = Self::require_admin(&env)?;
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &false);
        env.events().publish((Symbol::new(&env, "Unpaused"),), admin);
        Ok(())
    }

    pub fn is_paused(env: Env) -> bool {
        env.storage().instance().get(&DataKey::Paused).unwrap_or(false)
    }

    /// Define a new match id with its authorized oracle, settlement token,
    /// and betting deadline. Admin only.
    pub fn register_match(
        env: Env,
        match_id: u64,
        oracle: Address,
        token: Address,
        deadline: u64,
    ) -> Result<(), PlatformError> {
        Self::ensure_not_paused(&env)?;
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(PlatformError::Unauthorized)?;
        admin.require_auth();

        if env.storage().instance().has(&DataKey::Match(match_id)) {
            return Err(PlatformError::InternalError);
        }
        if deadline <= env.ledger().timestamp() {
            return Err(PlatformError::ExpiredDeadline);
        }

        let m = Match {
            match_id,
            oracle: oracle.clone(),
            token: token.clone(),
            deadline,
            settled: false,
            outcome: None,
        };
        env.storage().instance().set(&DataKey::Match(match_id), &m);

        let stats = MatchStats {
            match_id,
            pools: Vec::from_array(&env, [0i128, 0, 0]),
            bettors: Vec::new(&env),
        };
        env.storage()
            .instance()
            .set(&DataKey::Stats(match_id), &stats);

        env.events()
            .publish((Symbol::new(&env, "MatchRegistered"), admin), (match_id, oracle, token, deadline));
        Ok(())
    }

    /// Replace the oracle of an unsettled match. Admin only.
    pub fn set_oracle(
        env: Env,
        match_id: u64,
        new_oracle: Address,
    ) -> Result<(), PlatformError> {
        Self::ensure_not_paused(&env)?;
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(PlatformError::Unauthorized)?;
        admin.require_auth();

        let mut m: Match = env
            .storage()
            .instance()
            .get(&DataKey::Match(match_id))
            .ok_or(PlatformError::InternalError)?;
        if m.settled {
            return Err(PlatformError::InternalError);
        }
        m.oracle = new_oracle;
        env.storage().instance().set(&DataKey::Match(match_id), &m);
        Ok(())
    }

    // ── Bettor API ────────────────────────────────────────────────────────────

    /// Stake `amount` tokens on `outcome` for `match_id`.
    ///
    /// Validates that:
    /// - `amount > 0`
    /// - The match exists, is unsettled, and the betting window is still open
    /// - `user` holds at least `amount` of the match's settlement token
    ///
    /// Then transfers `amount` from `user` into the contract and records the
    /// bet + per-outcome pool statistics. Emits `BetPlaced`.
    pub fn place_bet(
        env: Env,
        user: Address,
        match_id: u64,
        outcome: Outcome,
        amount: i128,
    ) -> Result<(), PlatformError> {
        Self::ensure_not_paused(&env)?;
        user.require_auth();
        if amount <= 0 {
            return Err(PlatformError::InvalidAmount);
        }

        let m: Match = env
            .storage()
            .instance()
            .get(&DataKey::Match(match_id))
            .ok_or(PlatformError::InternalError)?;
        if m.settled {
            return Err(PlatformError::InternalError);
        }
        if env.ledger().timestamp() >= m.deadline {
            return Err(PlatformError::ExpiredDeadline);
        }

        let token_client = token::Client::new(&env, &m.token);
        let balance = token_client.balance(&user);
        if balance < amount {
            return Err(PlatformError::InsufficientBalance);
        }
        token_client.transfer(&user, &env.current_contract_address(), &amount);

        let bet = Bet {
            user: user.clone(),
            match_id,
            outcome,
            amount,
            token: m.token.clone(),
            claimed: false,
            payout: 0,
        };
        let key = DataKey::Bet(match_id, user.clone());
        if env.storage().persistent().has(&key) {
            return Err(PlatformError::InternalError);
        }
        env.storage().persistent().set(&key, &bet);

        let mut stats: MatchStats = env
            .storage()
            .instance()
            .get(&DataKey::Stats(match_id))
            .ok_or(PlatformError::InternalError)?;
        let idx = outcome.index();
        let prev = stats.pools.get(idx).unwrap_or(0);
        stats
            .pools
            .set(idx, prev.checked_add(amount).ok_or(PlatformError::Overflow)?);
        stats.bettors.push_back(user.clone());
        env.storage()
            .instance()
            .set(&DataKey::Stats(match_id), &stats);

        env.events()
            .publish((Symbol::new(&env, "BetPlaced"), user), bet);
        Ok(())
    }

    /// Oracle records the canonical `outcome` of `match_id`. Settlement is
    /// intentionally **constant-time**: only the canonical outcome flag and
    /// per-pool totals are committed. Winners pull their pro-rata share via
    /// `claim_payout` to keep this entry-point within Soroban's CPU budget,
    /// regardless of how many bettors exist.
    pub fn settle_bet(
        env: Env,
        oracle: Address,
        match_id: u64,
        outcome: Outcome,
    ) -> Result<(), PlatformError> {
        Self::ensure_not_paused(&env)?;
        oracle.require_auth();
        let mut m: Match = env
            .storage()
            .instance()
            .get(&DataKey::Match(match_id))
            .ok_or(PlatformError::InternalError)?;
        if m.oracle != oracle {
            return Err(PlatformError::Unauthorized);
        }
        if m.settled {
            return Err(PlatformError::InternalError);
        }

        m.settled = true;
        m.outcome = Some(outcome.index());
        env.storage().instance().set(&DataKey::Match(match_id), &m);

        env.events().publish(
            (Symbol::new(&env, "BetSettled"), oracle),
            (match_id, outcome.index()),
        );
        Ok(())
    }

    /// Pull the winner's pro-rata share of the losers' pool for a settled
    /// match.
    ///
    /// Payout (parimutuel, integer truncation toward zero):
    ///
    /// ```text
    /// winning_pool = pools[outcome]
    /// losing_pool  = total_pool - winning_pool
    /// share        = bet.amount * losing_pool / winning_pool
    /// payout       = bet.amount + share
    /// ```
    ///
    /// Emits `BetClaimed` with the paid amount.
    pub fn claim_payout(
        env: Env,
        user: Address,
        match_id: u64,
    ) -> Result<i128, PlatformError> {
        Self::ensure_not_paused(&env)?;
        user.require_auth();
        let key = DataKey::Bet(match_id, user.clone());
        let mut bet: Bet = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(PlatformError::InternalError)?;
        if bet.claimed {
            return Err(PlatformError::InternalError);
        }
        let m: Match = env
            .storage()
            .instance()
            .get(&DataKey::Match(match_id))
            .ok_or(PlatformError::InternalError)?;
        if !m.settled {
            return Err(PlatformError::ExpiredDeadline);
        }
        let winning_outcome = m.outcome.ok_or(PlatformError::InternalError)?;
        if bet.outcome.index() != winning_outcome {
            return Err(PlatformError::InternalError);
        }

        let stats: MatchStats = env
            .storage()
            .instance()
            .get(&DataKey::Stats(match_id))
            .ok_or(PlatformError::InternalError)?;
        let winning_pool = stats.pools.get(winning_outcome).unwrap_or(0);
        let total_pool = stats
            .pools
            .get(0)
            .unwrap_or(0)
            .checked_add(stats.pools.get(1).unwrap_or(0))
            .ok_or(PlatformError::Overflow)?
            .checked_add(stats.pools.get(2).unwrap_or(0))
            .ok_or(PlatformError::Overflow)?;
        let loser_pool = total_pool
            .checked_sub(winning_pool)
            .ok_or(PlatformError::Overflow)?;

        // Defensive: a "winning" outcome with zero pool means no winners —
        // this branch should be unreachable for any `Bet` whose outcome
        // matches `winning_outcome`, but reject rather than divide-by-zero.
        if winning_pool <= 0 {
            return Err(PlatformError::InternalError);
        }

        let share = bet
            .amount
            .checked_mul(loser_pool)
            .ok_or(PlatformError::Overflow)?
            .checked_div(winning_pool)
            .ok_or(PlatformError::InternalError)?;
        let payout = bet
            .amount
            .checked_add(share)
            .ok_or(PlatformError::Overflow)?;

        bet.claimed = true;
        bet.payout = payout;
        env.storage().persistent().set(&key, &bet);

        let token_client = token::Client::new(&env, &m.token);
        token_client.transfer(&env.current_contract_address(), &user, &payout);

        env.events().publish(
            (Symbol::new(&env, "BetClaimed"), user),
            (match_id, payout),
        );
        Ok(payout)
    }

    /// Refund the user's full bet amount if the match has passed its deadline
    /// without being settled. Idempotent per `(user, match_id)`.
    /// Emits `BetRefunded`.
    pub fn refund_bet(
        env: Env,
        user: Address,
        match_id: u64,
    ) -> Result<(), PlatformError> {
        Self::ensure_not_paused(&env)?;
        user.require_auth();
        let key = DataKey::Bet(match_id, user.clone());
        let mut bet: Bet = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(PlatformError::InternalError)?;
        if bet.claimed {
            return Err(PlatformError::InternalError);
        }
        let m: Match = env
            .storage()
            .instance()
            .get(&DataKey::Match(match_id))
            .ok_or(PlatformError::InternalError)?;
        if m.settled {
            return Err(PlatformError::InternalError);
        }
        if env.ledger().timestamp() < m.deadline {
            return Err(PlatformError::ExpiredDeadline);
        }

        bet.claimed = true;
        bet.payout = bet.amount;
        env.storage().persistent().set(&key, &bet);

        let token_client = token::Client::new(&env, &m.token);
        token_client.transfer(&env.current_contract_address(), &user, &bet.amount);

        env.events()
            .publish((Symbol::new(&env, "BetRefunded"), user), bet);
        Ok(())
    }

    /// Look up a user's bet on a match. Returns `None` if the bet does
    /// not exist.
    pub fn get_bet(env: Env, user: Address, match_id: u64) -> Option<Bet> {
        if env.storage().instance().get(&DataKey::Paused).unwrap_or(false) {
            return None;
        }
        env.storage()
            .persistent()
            .get(&DataKey::Bet(match_id, user))
    }

    /// Look up a match descriptor. Returns `None` if the match id is
    /// not registered.
    pub fn get_match(env: Env, match_id: u64) -> Option<Match> {
        if env.storage().instance().get(&DataKey::Paused).unwrap_or(false) {
            return None;
        }
        env.storage().instance().get(&DataKey::Match(match_id))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token::StellarAssetClient,
        Env,
    };

    fn setup() -> (Env, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_timestamp(1_000_000);
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        let token = env.register_stellar_asset_contract(admin.clone());
        (env, admin, oracle, token)
    }

    fn initialize(env: &Env, admin: &Address) -> RenaissanceBettingContractClient<'_> {
        let contract_id = env.register_contract(None, RenaissanceBettingContract);
        let client = RenaissanceBettingContractClient::new(env, &contract_id);
        client.initialize(admin);
        client
    }

    #[test]
    fn test_upgrade_requires_two_governance_approvals_and_preserves_state() {
        let (env, admin, oracle, token) = setup();
        let contract_id = env.register_contract(None, RenaissanceBettingContract);
        let client = RenaissanceBettingContractClient::new(&env, &contract_id);
        client.initialize(&admin);

        client.register_match(&20u64, &oracle, &token, &deadline_in(&env, 3_600));
        let user = Address::generate(&env);
        mint(&env, &token, &user, 1_000);
        client.place_bet(&user, &20u64, &Outcome::HomeWin, &100i128);

        let approver_a = Address::generate(&env);
        let approver_b = Address::generate(&env);
        env.as_contract(&contract_id, || {
            env.storage().instance().set(&DataKey::PlatformAdmin, &approver_a);
            env.storage().instance().set(&DataKey::SecurityAdmin, &approver_b);
            env.storage().instance().set(&DataKey::TreasuryAdmin, &admin);
        });

        let new_hash = BytesN::from_array(&env, &[9; 32]);
        client.upgrade(&new_hash).unwrap();
        let res = client.try_upgrade(&new_hash);
        assert!(res.is_err());

        let match_data = client.get_match(&20u64).unwrap();
        assert_eq!(match_data.match_id, 20u64);
        assert_eq!(match_data.oracle, oracle);
    }

    fn mint(env: &Env, token: &Address, to: &Address, amount: i128) {
        let sac = StellarAssetClient::new(env, token);
        sac.mint(to, &amount);
    }

    fn deadline_in(env: &Env, seconds: u64) -> u64 {
        env.ledger().timestamp() + seconds
    }

    // ── initialize ────────────────────────────────────────────────────────────

    #[test]
    fn test_initialize_bootstrap_admin() {
        let (env, admin, _oracle, _token) = setup();
        let _client = initialize(&env, &admin);
    }

    #[test]
    fn test_initialize_rejects_double_init() {
        let (env, admin, _oracle, _token) = setup();
        let client = initialize(&env, &admin);
        let res = client.try_initialize(&admin);
        assert!(res.is_err());
    }

    // ── register_match ────────────────────────────────────────────────────────

    #[test]
    fn test_register_match_happy_path() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&1u64, &oracle, &token, &deadline_in(&env, 3_600));
        let m = client.get_match(&1).unwrap();
        assert_eq!(m.match_id, 1);
        assert_eq!(m.oracle, oracle);
        assert_eq!(m.deadline, deadline_in(&env, 3_600));
        assert!(!m.settled);
        assert!(m.outcome.is_none());
    }

    #[test]
    fn test_register_match_rejects_past_deadline() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        let res = client.try_register_match(
            &2u64,
            &oracle,
            &token,
            &500_000u64, // before current timestamp of 1_000_000
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_register_match_rejects_duplicate_match_id() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&3u64, &oracle, &token, &deadline_in(&env, 100));
        let res = client.try_register_match(&3u64, &oracle, &token, &deadline_in(&env, 200));
        assert!(res.is_err());
    }

    #[test]
    fn test_set_oracle_updates_before_settle() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&4u64, &oracle, &token, &deadline_in(&env, 3_600));
        let new_oracle = Address::generate(&env);
        client.set_oracle(&4u64, &new_oracle);
        let m = client.get_match(&4).unwrap();
        assert_eq!(m.oracle, new_oracle);
    }

    // ── place_bet ─────────────────────────────────────────────────────────────

    #[test]
    fn test_place_bet_happy_path_records_bet() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&5u64, &oracle, &token, &deadline_in(&env, 3_600));

        let user = Address::generate(&env);
        mint(&env, &token, &user, 1_000);
        client.place_bet(&user, &5u64, &Outcome::HomeWin, &100i128);

        let bet = client.get_bet(&user, &5).unwrap();
        assert_eq!(bet.amount, 100);
        assert_eq!(bet.outcome, Outcome::HomeWin);
        assert!(!bet.claimed);
        assert_eq!(bet.payout, 0);
    }

    #[test]
    fn test_place_bet_moves_tokens_from_user_to_contract() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&6u64, &oracle, &token, &deadline_in(&env, 3_600));

        let user = Address::generate(&env);
        mint(&env, &token, &user, 1_000);
        client.place_bet(&user, &6u64, &Outcome::HomeWin, &400i128);

        let tc = token::Client::new(&env, &token);
        assert_eq!(tc.balance(&user), 600);

        let contract_addr = env.current_contract_address();
        assert_eq!(tc.balance(&contract_addr), 400);
    }

    #[test]
    fn test_place_bet_rejects_zero_amount() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&7u64, &oracle, &token, &deadline_in(&env, 3_600));
        let user = Address::generate(&env);
        mint(&env, &token, &user, 1_000);
        let res = client.try_place_bet(&user, &7u64, &Outcome::HomeWin, &0i128);
        match res {
            Err(Ok(e)) => assert_eq!(e, PlatformError::InvalidAmount),
            _ => panic!("expected InvalidAmount contract error"),
        }
    }

    #[test]
    fn test_place_bet_rejects_insufficient_balance() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&8u64, &oracle, &token, &deadline_in(&env, 3_600));
        let user = Address::generate(&env);
        mint(&env, &token, &user, 100);
        let res =
            client.try_place_bet(&user, &8u64, &Outcome::HomeWin, &500i128);
        match res {
            Err(Ok(e)) => assert_eq!(e, PlatformError::InsufficientBalance),
            _ => panic!("expected InsufficientBalance contract error"),
        }
    }

    #[test]
    fn test_place_bet_rejects_after_deadline() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&9u64, &oracle, &token, &deadline_in(&env, 3_600));
        let user = Address::generate(&env);
        mint(&env, &token, &user, 1_000);

        env.ledger().set_timestamp(deadline_in(&env, 3_600) + 1);
        let res =
            client.try_place_bet(&user, &9u64, &Outcome::HomeWin, &100i128);
        match res {
            Err(Ok(e)) => assert_eq!(e, PlatformError::ExpiredDeadline),
            _ => panic!("expected ExpiredDeadline contract error"),
        }
    }

    #[test]
    fn test_place_bet_rejects_already_settled_match() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&10u64, &oracle, &token, &deadline_in(&env, 3_600));
        let user_a = Address::generate(&env);
        mint(&env, &token, &user_a, 1_000);
        client.place_bet(&user_a, &10u64, &Outcome::HomeWin, &100i128);
        client.settle_bet(&oracle, &10u64, &Outcome::HomeWin);

        let user_b = Address::generate(&env);
        mint(&env, &token, &user_b, 1_000);
        let res = client.try_place_bet(&user_b, &10u64, &Outcome::Draw, &50i128);
        assert!(res.is_err());
    }

    #[test]
    fn test_place_bet_rejects_double_bet_same_user() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&11u64, &oracle, &token, &deadline_in(&env, 3_600));
        let user = Address::generate(&env);
        mint(&env, &token, &user, 1_000);
        client.place_bet(&user, &11u64, &Outcome::HomeWin, &100i128);
        let res = client.try_place_bet(&user, &11u64, &Outcome::Draw, &50i128);
        assert!(res.is_err());
    }

    // ── settle_bet ────────────────────────────────────────────────────────────

    #[test]
    fn test_settle_bet_rejects_non_oracle() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&12u64, &oracle, &token, &deadline_in(&env, 3_600));
        let impostor = Address::generate(&env);
        let res = client.try_settle_bet(&impostor, &12u64, &Outcome::HomeWin);
        match res {
            Err(Ok(e)) => assert_eq!(e, PlatformError::Unauthorized),
            _ => panic!("expected Unauthorized contract error"),
        }
    }

    #[test]
    fn test_settle_bet_records_outcome() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&13u64, &oracle, &token, &deadline_in(&env, 3_600));
        let user = Address::generate(&env);
        mint(&env, &token, &user, 1_000);
        client.place_bet(&user, &13u64, &Outcome::Draw, &200i128);
        client.settle_bet(&oracle, &13u64, &Outcome::Draw);

        let m = client.get_match(&13).unwrap();
        assert!(m.settled);
        assert_eq!(m.outcome, Some(Outcome::Draw));
    }

    #[test]
    fn test_settle_bet_rejects_double_settle() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&14u64, &oracle, &token, &deadline_in(&env, 3_600));
        let user = Address::generate(&env);
        mint(&env, &token, &user, 1_000);
        client.place_bet(&user, &14u64, &Outcome::HomeWin, &100i128);
        client.settle_bet(&oracle, &14u64, &Outcome::HomeWin);
        let res = client.try_settle_bet(&oracle, &14u64, &Outcome::Draw);
        assert!(res.is_err());
    }

    #[test]
    fn test_set_oracle_rejects_after_settle() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&15u64, &oracle, &token, &deadline_in(&env, 3_600));
        let user = Address::generate(&env);
        mint(&env, &token, &user, 1_000);
        client.place_bet(&user, &15u64, &Outcome::HomeWin, &100i128);
        client.settle_bet(&oracle, &15u64, &Outcome::HomeWin);

        let new_oracle = Address::generate(&env);
        let res = client.try_set_oracle(&15u64, &new_oracle);
        assert!(res.is_err());
    }

    // ── claim_payout ──────────────────────────────────────────────────────────

    #[test]
    fn test_claim_payout_proportional_split_two_winners() {
        // Two winners bet HomeWin 100 each; two losers bet Draw 200 total.
        // winning_pool = 200, total_pool = 400, loser_pool = 200.
        // share per winner = 100 + (100 * 200 / 200) = 200.
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&16u64, &oracle, &token, &deadline_in(&env, 3_600));

        let w1 = Address::generate(&env);
        let w2 = Address::generate(&env);
        let l1 = Address::generate(&env);
        let l2 = Address::generate(&env);
        for u in [&w1, &w2, &l1, &l2] {
            mint(&env, &token, u, 1_000);
        }

        client.place_bet(&w1, &16u64, &Outcome::HomeWin, &100i128);
        client.place_bet(&w2, &16u64, &Outcome::HomeWin, &100i128);
        client.place_bet(&l1, &16u64, &Outcome::Draw, &120i128);
        client.place_bet(&l2, &16u64, &Outcome::Draw, &80i128);

        client.settle_bet(&oracle, &16u64, &Outcome::HomeWin);

        let tc = token::Client::new(&env, &token);

        let bal_before = tc.balance(&w1);
        let payout1 = client.claim_payout(&w1, &16).unwrap();
        assert_eq!(payout1, 200);
        assert_eq!(tc.balance(&w1), bal_before + 200);

        let payout2 = client.claim_payout(&w2, &16).unwrap();
        assert_eq!(payout2, 200);
        // w2: 1_000 mint - 100 bet + 200 payout = 1_100
        assert_eq!(tc.balance(&w2), 1_100);
    }

    #[test]
    fn test_claim_payout_solo_winner_takes_loser_pool() {
        // Sole winner of 100 takes the entire loser pool of 300 + their 100.
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&17u64, &oracle, &token, &deadline_in(&env, 3_600));

        let winner = Address::generate(&env);
        let loser = Address::generate(&env);
        mint(&env, &token, &winner, 1_000);
        mint(&env, &token, &loser, 1_000);

        client.place_bet(&winner, &17u64, &Outcome::HomeWin, &100i128);
        client.place_bet(&loser, &17u64, &Outcome::Draw, &300i128);
        client.settle_bet(&oracle, &17u64, &Outcome::HomeWin);

        let payout = client.claim_payout(&winner, &17).unwrap();
        assert_eq!(payout, 400);

        // Loser cannot claim (their outcome != winning outcome).
        let res = client.try_claim_payout(&loser, &17);
        assert!(res.is_err());
    }

    #[test]
    fn test_claim_payout_rejects_double_claim() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&18u64, &oracle, &token, &deadline_in(&env, 3_600));

        let user = Address::generate(&env);
        mint(&env, &token, &user, 1_000);
        client.place_bet(&user, &18u64, &Outcome::HomeWin, &100i128);
        client.settle_bet(&oracle, &18u64, &Outcome::HomeWin);
        client.claim_payout(&user, &18).unwrap();

        let res = client.try_claim_payout(&user, &18);
        assert!(res.is_err());
    }

    #[test]
    fn test_claim_payout_rejects_unsettled_match() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&19u64, &oracle, &token, &deadline_in(&env, 3_600));

        let user = Address::generate(&env);
        mint(&env, &token, &user, 1_000);
        client.place_bet(&user, &19u64, &Outcome::HomeWin, &100i128);

        let res = client.try_claim_payout(&user, &19);
        match res {
            Err(Ok(e)) => assert_eq!(e, PlatformError::ExpiredDeadline),
            _ => panic!("expected ExpiredDeadline contract error"),
        }
    }

    #[test]
    fn test_claim_payout_rejects_unknown_bet() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&20u64, &oracle, &token, &deadline_in(&env, 3_600));
        client.settle_bet(&oracle, &20u64, &Outcome::HomeWin);

        let user = Address::generate(&env);
        let res = client.try_claim_payout(&user, &20);
        assert!(res.is_err());
    }

    // ── refund_bet ────────────────────────────────────────────────────────────

    #[test]
    fn test_refund_bet_returns_tokens_after_deadline_unsettled() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&21u64, &oracle, &token, &deadline_in(&env, 3_600));

        let user = Address::generate(&env);
        mint(&env, &token, &user, 1_000);
        client.place_bet(&user, &21u64, &Outcome::HomeWin, &300i128);

        env.ledger().set_timestamp(deadline_in(&env, 3_600) + 60);
        client.refund_bet(&user, &21);

        let tc = token::Client::new(&env, &token);
        assert_eq!(tc.balance(&user), 1_000, "refund returned full bet amount");

        let bet = client.get_bet(&user, &21).unwrap();
        assert!(bet.claimed);
        assert_eq!(bet.payout, 300);
    }

    #[test]
    fn test_refund_bet_rejects_before_deadline() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&22u64, &oracle, &token, &deadline_in(&env, 3_600));

        let user = Address::generate(&env);
        mint(&env, &token, &user, 1_000);
        client.place_bet(&user, &22u64, &Outcome::HomeWin, &100i128);

        let res = client.try_refund_bet(&user, &22);
        match res {
            Err(Ok(e)) => assert_eq!(e, PlatformError::ExpiredDeadline),
            _ => panic!("expected ExpiredDeadline contract error"),
        }
    }

    #[test]
    fn test_refund_bet_rejects_after_settlement() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&23u64, &oracle, &token, &deadline_in(&env, 3_600));

        let user = Address::generate(&env);
        mint(&env, &token, &user, 1_000);
        client.place_bet(&user, &23u64, &Outcome::HomeWin, &100i128);
        client.settle_bet(&oracle, &23u64, &Outcome::HomeWin);

        env.ledger().set_timestamp(deadline_in(&env, 3_600) + 60);
        let res = client.try_refund_bet(&user, &23);
        assert!(res.is_err());
    }

    #[test]
    fn test_refund_bet_rejects_unknown_bet() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&24u64, &oracle, &token, &deadline_in(&env, 3_600));
        env.ledger().set_timestamp(deadline_in(&env, 3_600) + 60);

        let user = Address::generate(&env);
        let res = client.try_refund_bet(&user, &24);
        assert!(res.is_err());
    }

    // ── get_* ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_get_bet_returns_none_for_unknown_user() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);
        client.register_match(&25u64, &oracle, &token, &deadline_in(&env, 3_600));
        let user = Address::generate(&env);
        assert!(client.get_bet(&user, &25).is_none());
    }

    #[test]
    fn test_get_match_returns_none_for_unknown_match() {
        let (env, admin, oracle, token) = setup();
        let client = initialize(&env, &admin);

        // Unknown match id — NoMatch -> None.
        assert!(client.get_match(&999u64).is_none());

        // Registered match id -> Some(Match).
        client.register_match(&100u64, &oracle, &token, &deadline_in(&env, 100));
        let m = client.get_match(&100).unwrap();
        assert_eq!(m.match_id, 100);
    }
}
