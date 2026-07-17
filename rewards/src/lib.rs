#![no_std]

//! Renaissance Fan Rewards & Loyalty Points Contract
//!
//! Tracks fan engagement points earned through on-platform activities
//! (watching matches, sharing content, referrals, ...) that can be redeemed
//! for exclusive Renaissance NFT drops or partnered perks.
//!
//! Authorization model:
//! * A single **platform admin** (set during `initialize`) is the only
//!   address authorised to call `award_points`.
//! * Individual users authorise their own `redeem_points` and
//!   `transfer_points` calls.

use renaissance_core::{get_event_topic_by_string, PlatformError};
use soroban_sdk::{contract, contractimpl, contracttype, Address, BytesN, Env, Symbol, Vec};

// ── TTL ──────────────────────────────────────────────────────────────────────
// Stellar produces ~1 ledger every 5 seconds. 30 days ≈ 518_400 ledgers.
const DAY_IN_LEDGERS: u32 = 17_280; // ⌊24·60·60 / 5⌋
const MAX_BALANCE_TTL: u32 = 30 * DAY_IN_LEDGERS; // 30 days

// ── Storage keys ─────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    /// Stored in instance storage. The platform address authorised to award points.
    Admin,
    Paused,
    PlatformAdmin,
    SecurityAdmin,
    TreasuryAdmin,
    UpgradeApprovals,
    WasmHash,
    /// Stored in persistent storage. One entry per user address.
    Balance(Address),
}

// ── Contract errors ──────────────────────────────────────────────────────────
// All errors are surfaced through `renaissance_core::PlatformError` so child
// contracts share a canonical error catalogue.

// ── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct FanRewardsContract;

#[contractimpl]
impl FanRewardsContract {
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

    // ── Lifecycle ────────────────────────────────────────────────────────────

    /// One-time initialisation: designate the platform admin that will be the
    /// sole address able to award loyalty points.
    ///
    /// Returns `InternalError` if the contract has already been initialised.
    pub fn initialize(env: Env, admin: Address) -> Result<(), PlatformError> {
        admin.require_auth();

        if env.storage().instance().has(&DataKey::Admin) {
            return Err(PlatformError::InternalError);
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::PlatformAdmin, &admin);
        env.storage().instance().set(&DataKey::SecurityAdmin, &admin);
        env.storage().instance().set(&DataKey::TreasuryAdmin, &admin);
        env.storage().instance().set(&DataKey::Paused, &false);
        // Re-bump instance storage TTL so config survives long inactivity.
        env.storage()
            .instance()
            .extend_ttl(MAX_BALANCE_TTL, MAX_BALANCE_TTL);

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

    /// Return the currently configured platform admin, if any.
    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Admin)
    }

    // ── Award (platform-only) ────────────────────────────────────────────────

    /// Award fan loyalty points for an engagement activity.
    ///
    /// Authorisation: only the configured platform `admin` may call this.
    /// `amount` must be strictly positive; the resulting balance must not
    /// overflow `i128::MAX`.
    pub fn award_points(
        env: Env,
        user: Address,
        amount: i128,
        reason: Symbol,
    ) -> Result<(), PlatformError> {
        Self::ensure_not_paused(&env)?;
        // Reject if admin has not been configured yet.
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(PlatformError::Unauthorized)?;

        admin.require_auth();

        if amount <= 0 {
            return Err(PlatformError::InvalidAmount);
        }

        let key = DataKey::Balance(user.clone());
        let current: i128 = env.storage().persistent().get(&key).unwrap_or(0_i128);
        let new_balance = current
            .checked_add(amount)
            .ok_or(PlatformError::Overflow)?;

        env.storage().persistent().set(&key, &new_balance);
        env.storage()
            .persistent()
            .extend_ttl(&key, MAX_BALANCE_TTL, MAX_BALANCE_TTL);

        env.events().publish(
            (
                get_event_topic_by_string(&env, "PointsAwarded"),
                user.clone(),
            ),
            (amount, reason, new_balance),
        );

        Ok(())
    }

    // ── Redeem (user-initiated) ──────────────────────────────────────────────

    /// Redeem loyalty points for an exclusive reward (NFT drop, perk, ...).
    ///
    /// Authorisation: the `user` whose balance is being spent.
    /// `cost` must be strictly positive and ≤ the current balance.
    pub fn redeem_points(
        env: Env,
        user: Address,
        cost: i128,
        reward_id: Symbol,
    ) -> Result<(), PlatformError> {
        Self::ensure_not_paused(&env)?;
        user.require_auth();

        if cost <= 0 {
            return Err(PlatformError::InvalidAmount);
        }

        let key = DataKey::Balance(user.clone());
        let current: i128 = env.storage().persistent().get(&key).unwrap_or(0_i128);

        // Negative-balance guard: cannot redeem more than is held.
        let new_balance = current
            .checked_sub(cost)
            .ok_or(PlatformError::InvalidAmount)?;

        env.storage().persistent().set(&key, &new_balance);
        env.storage()
            .persistent()
            .extend_ttl(&key, MAX_BALANCE_TTL, MAX_BALANCE_TTL);

        env.events().publish(
            (
                get_event_topic_by_string(&env, "PointsRedeemed"),
                user.clone(),
            ),
            (cost, reward_id, new_balance),
        );

        Ok(())
    }

    // ── Transfer (peer-to-peer gifting) ──────────────────────────────────────

    /// Transfer loyalty points from `from` to `to` (optional gifting path).
    ///
    /// Authorisation: the `from` address. `amount` must be strictly positive,
    /// strictly less than or equal to `from`'s balance, and `from != to` to
    /// avoid pointless storage churn.
    pub fn transfer_points(
        env: Env,
        from: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), PlatformError> {
        Self::ensure_not_paused(&env)?;
        from.require_auth();

        if amount <= 0 {
            return Err(PlatformError::InvalidAmount);
        }
        if from == to {
            return Err(PlatformError::InvalidAmount);
        }

        // Debit source first; refuse if it would underflow into a negative balance.
        let from_key = DataKey::Balance(from.clone());
        let from_balance: i128 = env
            .storage()
            .persistent()
            .get(&from_key)
            .unwrap_or(0_i128);
        let new_from_balance = from_balance
            .checked_sub(amount)
            .ok_or(PlatformError::InvalidAmount)?;

        // Credit destination; refuse if it would overflow i128::MAX.
        let to_key = DataKey::Balance(to.clone());
        let to_balance: i128 = env.storage().persistent().get(&to_key).unwrap_or(0_i128);
        let new_to_balance = to_balance
            .checked_add(amount)
            .ok_or(PlatformError::Overflow)?;

        env.storage().persistent().set(&from_key, &new_from_balance);
        env.storage()
            .persistent()
            .extend_ttl(&from_key, MAX_BALANCE_TTL, MAX_BALANCE_TTL);
        env.storage().persistent().set(&to_key, &new_to_balance);
        env.storage()
            .persistent()
            .extend_ttl(&to_key, MAX_BALANCE_TTL, MAX_BALANCE_TTL);

        env.events().publish(
            (
                get_event_topic_by_string(&env, "PointsTransferred"),
                from.clone(),
                to.clone(),
            ),
            (amount, new_from_balance, new_to_balance),
        );

        Ok(())
    }

    // ── Views ────────────────────────────────────────────────────────────────

    /// Return the loyalty point balance held by `user`. New/unknown users get 0.
    pub fn get_balance(env: Env, user: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(user))
            .unwrap_or(0_i128)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{symbol_short, testutils::Address as _, Symbol};

    fn setup() -> (Env, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, FanRewardsContract);
        let admin = Address::generate(&env);

        let client = FanRewardsContractClient::new(&env, &contract_id);
        client.initialize(&admin).unwrap();

        (env, admin, contract_id)
    }

    #[test]
    fn test_upgrade_requires_two_governance_approvals() {
        let (env, admin, contract_id) = setup();
        let c = client(&env, &contract_id);
        let approver_a = Address::generate(&env);
        let approver_b = Address::generate(&env);
        env.as_contract(&contract_id, || {
            env.storage().instance().set(&DataKey::PlatformAdmin, &approver_a);
            env.storage().instance().set(&DataKey::SecurityAdmin, &approver_b);
            env.storage().instance().set(&DataKey::TreasuryAdmin, &admin);
        });

        let new_hash = BytesN::from_array(&env, &[7; 32]);
        c.upgrade(&new_hash).unwrap();
        let res = c.try_upgrade(&new_hash);
        assert!(res.is_err());
    }

    fn client(env: &Env, contract_id: &Address) -> FanRewardsContractClient {
        FanRewardsContractClient::new(env, contract_id)
    }

    // ── initialize ───────────────────────────────────────────────────────────

    #[test]
    fn test_initialize_records_admin() {
        let (env, admin, contract_id) = setup();
        let c = client(&env, &contract_id);
        assert_eq!(c.get_admin(), Some(admin));
    }

    #[test]
    fn test_double_initialize_is_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, FanRewardsContract);
        let c = client(&env, &contract_id);
        let admin = Address::generate(&env);

        c.initialize(&admin).unwrap();
        let other = c.try_initialize(&admin);
        assert!(other.is_err());
    }

    // ── award_points ─────────────────────────────────────────────────────────

    #[test]
    fn test_award_points_increases_balance() {
        let (env, _admin, contract_id) = setup();
        let c = client(&env, &contract_id);
        let user = Address::generate(&env);

        c.award_points(
            &user,
            &100_i128,
            &symbol_short!("watched"),
        )
        .unwrap();
        assert_eq!(c.get_balance(&user), 100_i128);
    }

    #[test]
    fn test_double_award_accumulates_balance() {
        // Acceptance criterion: double-award is safe (no double-spend, accumulation is correct).
        let (env, _admin, contract_id) = setup();
        let c = client(&env, &contract_id);
        let user = Address::generate(&env);

        c.award_points(&user, &100_i128, &symbol_short!("watched")).unwrap();
        c.award_points(&user, &250_i128, &symbol_short!("shared")).unwrap();
        c.award_points(&user, &50_i128, &symbol_short!("referral")).unwrap();

        assert_eq!(c.get_balance(&user), 400_i128);
    }

    #[test]
    fn test_award_points_rejects_non_admin() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, FanRewardsContract);

        // Deliberately do NOT call initialize() — there is no admin.
        let c = client(&env, &contract_id);
        let user = Address::generate(&env);

        let res = c.try_award_points(&user, &10_i128, &symbol_short!("watched"));
        assert!(res.is_err()); // Unauthorized
    }

    #[test]
    fn test_award_points_rejects_zero_amount() {
        let (env, _admin, contract_id) = setup();
        let c = client(&env, &contract_id);
        let user = Address::generate(&env);

        let res = c.try_award_points(&user, &0_i128, &symbol_short!("watched"));
        assert!(res.is_err()); // InvalidAmount
    }

    #[test]
    fn test_award_points_rejects_negative_amount() {
        let (env, _admin, contract_id) = setup();
        let c = client(&env, &contract_id);
        let user = Address::generate(&env);

        let res = c.try_award_points(&user, &-50_i128, &symbol_short!("watched"));
        assert!(res.is_err()); // InvalidAmount
    }

    #[test]
    fn test_award_points_protects_against_overflow() {
        let (env, _admin, contract_id) = setup();
        let c = client(&env, &contract_id);
        let user = Address::generate(&env);

        // Seed the user with a value near i128::MAX, then try to award +1.
        // We can't write persistent storage directly from the test, so we
        // award i128::MAX / 2 twice and then attempt a third award that
        // would overflow.
        let half = i128::MAX / 2 + 1; // ensures 2 * half > i128::MAX
        c.award_points(&user, &half, &symbol_short!("a")).unwrap();
        c.award_points(&user, &half, &symbol_short!("b")).unwrap();
        assert_eq!(c.get_balance(&user), half * 2);

        let res = c.try_award_points(&user, &1_i128, &symbol_short!("c"));
        assert!(res.is_err()); // Overflow
    }

    // ── redeem_points ────────────────────────────────────────────────────────

    #[test]
    fn test_redeem_points_decreases_balance() {
        let (env, _admin, contract_id) = setup();
        let c = client(&env, &contract_id);
        let user = Address::generate(&env);

        c.award_points(&user, &1_000_i128, &symbol_short!("watched")).unwrap();
        c.redeem_points(&user, &400_i128, &symbol_short!("nft_drop"))
            .unwrap();

        assert_eq!(c.get_balance(&user), 600_i128);
    }

    #[test]
    fn test_redeem_full_balance_succeeds() {
        let (env, _admin, contract_id) = setup();
        let c = client(&env, &contract_id);
        let user = Address::generate(&env);

        c.award_points(&user, &500_i128, &symbol_short!("watched")).unwrap();
        c.redeem_points(&user, &500_i128, &symbol_short!("nft_drop"))
            .unwrap();

        assert_eq!(c.get_balance(&user), 0_i128);
    }

    #[test]
    fn test_negative_balance_prevented_at_redeem() {
        // Acceptance criterion: negative balance edge case.
        let (env, _admin, contract_id) = setup();
        let c = client(&env, &contract_id);
        let user = Address::generate(&env);

        // User has never been awarded → balance is 0.
        let res = c.try_redeem_points(&user, &1_i128, &symbol_short!("nft_drop"));
        assert!(res.is_err()); // InvalidAmount (underflow)

        // Award a small balance, then try to redeem more than they hold.
        c.award_points(&user, &10_i128, &symbol_short!("watched")).unwrap();
        let res = c.try_redeem_points(&user, &11_i128, &symbol_short!("nft_drop"));
        assert!(res.is_err()); // InvalidAmount (underflow)

        // Balance should remain unchanged after the failed redeem.
        assert_eq!(c.get_balance(&user), 10_i128);
    }

    #[test]
    fn test_redeem_points_rejects_zero_cost() {
        let (env, _admin, contract_id) = setup();
        let c = client(&env, &contract_id);
        let user = Address::generate(&env);

        c.award_points(&user, &100_i128, &symbol_short!("watched")).unwrap();
        let res = c.try_redeem_points(&user, &0_i128, &symbol_short!("nft_drop"));
        assert!(res.is_err()); // InvalidAmount
    }

    #[test]
    fn test_redeem_points_rejects_negative_cost() {
        let (env, _admin, contract_id) = setup();
        let c = client(&env, &contract_id);
        let user = Address::generate(&env);

        c.award_points(&user, &100_i128, &symbol_short!("watched")).unwrap();
        let res = c.try_redeem_points(&user, &-10_i128, &symbol_short!("nft_drop"));
        assert!(res.is_err()); // InvalidAmount
    }

    // ── transfer_points ──────────────────────────────────────────────────────

    #[test]
    fn test_transfer_points_moves_balance() {
        let (env, _admin, contract_id) = setup();
        let c = client(&env, &contract_id);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);

        c.award_points(&alice, &1_000_i128, &symbol_short!("watched"))
            .unwrap();
        c.transfer_points(&alice, &bob, &350_i128).unwrap();

        assert_eq!(c.get_balance(&alice), 650_i128);
        assert_eq!(c.get_balance(&bob), 350_i128);
    }

    #[test]
    fn test_transfer_rejects_zero_and_negative_amounts() {
        let (env, _admin, contract_id) = setup();
        let c = client(&env, &contract_id);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);

        c.award_points(&alice, &100_i128, &symbol_short!("watched"))
            .unwrap();

        assert!(c.try_transfer_points(&alice, &bob, &0_i128).is_err());
        assert!(c
            .try_transfer_points(&alice, &bob, &-1_i128)
            .is_err());

        // Balances unchanged after rejection.
        assert_eq!(c.get_balance(&alice), 100_i128);
        assert_eq!(c.get_balance(&bob), 0_i128);
    }

    #[test]
    fn test_transfer_self_is_rejected() {
        let (env, _admin, contract_id) = setup();
        let c = client(&env, &contract_id);
        let user = Address::generate(&env);

        c.award_points(&user, &100_i128, &symbol_short!("watched"))
            .unwrap();
        let res = c.try_transfer_points(&user, &user, &10_i128);
        assert!(res.is_err()); // InvalidAmount — pointless self-transfer refused
        assert_eq!(c.get_balance(&user), 100_i128);
    }

    #[test]
    fn test_transfer_rejects_insufficient_balance() {
        let (env, _admin, contract_id) = setup();
        let c = client(&env, &contract_id);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);

        c.award_points(&alice, &5_i128, &symbol_short!("watched")).unwrap();
        let res = c.try_transfer_points(&alice, &bob, &6_i128);
        assert!(res.is_err()); // InvalidAmount (underflow)

        assert_eq!(c.get_balance(&alice), 5_i128);
        assert_eq!(c.get_balance(&bob), 0_i128);
    }

    // ── Events ───────────────────────────────────────────────────────────────

    #[test]
    fn test_award_emits_points_awarded_event() {
        let (env, _admin, contract_id) = setup();
        let c = client(&env, &contract_id);
        let user = Address::generate(&env);

        c.award_points(&user, &42_i128, &symbol_short!("watched"))
            .unwrap();

        let events = env.events().all();
        assert_eq!(events.len(), 1);
        let (topics, _data) = events.get(0);
        let topic0: Symbol = topics.get(0);
        assert_eq!(topic0, Symbol::new(&env, "PointsAwarded"));
    }

    #[test]
    fn test_redeem_emits_points_redeemed_event() {
        let (env, _admin, contract_id) = setup();
        let c = client(&env, &contract_id);
        let user = Address::generate(&env);

        c.award_points(&user, &100_i128, &symbol_short!("watched"))
            .unwrap();
        c.redeem_points(&user, &40_i128, &symbol_short!("nft_drop"))
            .unwrap();

        let events = env.events().all();
        // 1 emit from award + 1 emit from redeem
        assert_eq!(events.len(), 2);
        let (topics, _data) = events.get(events.len() - 1);
        let topic0: Symbol = topics.get(0);
        assert_eq!(topic0, Symbol::new(&env, "PointsRedeemed"));
    }

    #[test]
    fn test_transfer_emits_points_transferred_event() {
        let (env, _admin, contract_id) = setup();
        let c = client(&env, &contract_id);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);

        c.award_points(&alice, &200_i128, &symbol_short!("watched"))
            .unwrap();
        c.transfer_points(&alice, &bob, &50_i128).unwrap();

        let events = env.events().all();
        let (topics, _data) = events.get(events.len() - 1);
        let topic0: Symbol = topics.get(0);
        assert_eq!(topic0, Symbol::new(&env, "PointsTransferred"));
    }

    // ── Views ────────────────────────────────────────────────────────────────

    #[test]
    fn test_get_balance_returns_zero_for_unknown_user() {
        let (env, _admin, contract_id) = setup();
        let c = client(&env, &contract_id);
        let user = Address::generate(&env);
        assert_eq!(c.get_balance(&user), 0_i128);
    }
}
