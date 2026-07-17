#![no_std]

//! `renaissance-vault` token vault contract for betting stakes.
//!
//! A Soroban vault that holds XLM or custom Renaissance tokens (e.g., RENA) as betting stakes.
//! Integrates with Stellar Asset Contract (SAC) or native XLM to lock funds during active bets
//! and distribute payouts to winners. Implements reentrancy protection and strict access controls.

use renaissance_core::{get_event_topic_by_string, PlatformError};
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, token, Address, Env, Symbol};

// ── Balance structures ─────────────────────────────────────────────────────────

/// Tracks user balances across all assets
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserBalance {
    pub available: i128, // Funds available to withdraw or use for bets
    pub locked: i128,    // Funds currently locked in active bets
}

/// Tracks total vault balances for accounting
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultBalance {
    pub total_deposited: i128,
    pub total_locked: i128,
}

// ── Storage keys ───────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    /// Admin address that can manage authorized contracts
    Admin,
    /// Paused state flag for emergency stops
    Paused,
    /// Authorized betting contract address (only this can call lock_for_bet/payout)
    BettingContract,
    /// Flag to prevent reentrancy attacks
    ReentrancyGuard,
    /// User balance: (user address, asset address) -> UserBalance
    UserBalance(Address, Address),
    /// Vault total balance for an asset: asset address -> VaultBalance
    VaultBalance(Address),
    /// Locked funds for a specific match: (match_id, user, asset) -> amount
    LockedBet(u64, Address, Address),
    /// Pending withdrawal for an asset
    PendingWithdrawal(Address),
    /// Pending admin transfer
    PendingAdmin,
}

// ── Custom errors ───────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum VaultError {
    /// Caller is not the authorized betting contract
    UnauthorizedBettingContract = 200,
    /// Insufficient balance to perform the operation
    InsufficientBalance = 201,
    /// Reentrancy attempt detected
    ReentrancyDetected = 202,
    /// Invalid amount (must be positive)
    InvalidAmount = 203,
    /// Bet lock not found
    BetLockNotFound = 204,
    /// Timelock not expired yet
    TimelockNotExpired = 205,
    /// Token is tracked and cannot be recovered
    TokenIsTracked = 206,
    /// Mismatch in pending data
    MismatchPendingData = 207,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingWithdrawal {
    pub to: Address,
    pub amount: i128,
    pub pending_until: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingAdminData {
    pub new_admin: Address,
    pub pending_until: u64,
}

// ── Contract Implementation ─────────────────────────────────────────────────────

#[contract]
pub struct RenaissanceVaultContract;

#[contractimpl]
impl RenaissanceVaultContract {
    /// Reentrancy guard implementation - follows checks-effects-interactions pattern
    fn enter_reentrancy_guard(env: &Env) -> Result<(), VaultError> {
        if env.storage().instance().has(&DataKey::ReentrancyGuard) {
            return Err(VaultError::ReentrancyDetected);
        }
        env.storage().instance().set(&DataKey::ReentrancyGuard, &true);
        Ok(())
    }

    fn exit_reentrancy_guard(env: &Env) {
        env.storage().instance().remove(&DataKey::ReentrancyGuard);
    }

    /// Helper to ensure caller is the authorized betting contract
    fn ensure_betting_contract(env: &Env) -> Result<Address, PlatformError> {
        let betting_contract: Address = env
            .storage()
            .instance()
            .get(&DataKey::BettingContract)
            .ok_or(PlatformError::Unauthorized)?;
            
        betting_contract.require_auth();
        Ok(betting_contract)
    }

    /// Helper to ensure contract is not paused
    fn ensure_not_paused(env: &Env) -> Result<(), PlatformError> {
        if env.storage().instance().get(&DataKey::Paused).unwrap_or(false) {
            return Err(PlatformError::Paused);
        }
        Ok(())
    }

    /// One-time initialization of the vault contract
    /// Sets up admin and authorizes the initial betting contract
    pub fn initialize(env: Env, admin: Address, betting_contract: Address) -> Result<(), PlatformError> {
        admin.require_auth();

        if env.storage().instance().has(&DataKey::Admin) {
            return Err(PlatformError::InternalError);
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::BettingContract, &betting_contract);
        env.storage().instance().set(&DataKey::Paused, &false);

        // Extend TTL for instance storage (30 days)
        const MAX_TTL: u32 = 518400;
        env.storage()
            .instance()
            .extend_ttl(MAX_TTL, MAX_TTL);

        Ok(())
    }

    /// Update the authorized betting contract (admin only)
    pub fn set_betting_contract(env: Env, new_betting_contract: Address) -> Result<(), PlatformError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(PlatformError::Unauthorized)?;
        admin.require_auth();

        env.storage()
            .instance()
            .set(&DataKey::BettingContract, &new_betting_contract);

        Ok(())
    }

    /// Deposit tokens into the vault - user stakes their tokens
    /// Works with both native XLM (via SAC) and any custom asset
    pub fn deposit(env: Env, user: Address, amount: i128, asset: Address) -> Result<(), VaultError> {
        if amount <= 0 {
            return Err(VaultError::InvalidAmount);
        }

        Self::ensure_not_paused(&env).map_err(|_| VaultError::InvalidAmount)?;
        user.require_auth();

        // Use reentrancy guard to prevent recursive calls
        Self::enter_reentrancy_guard(&env)?;

        // Transfer tokens from user to vault
        let client = token::Client::new(&env, &asset);
        client.transfer(&user, &env.current_contract_address(), &amount);

        // Update user's balance
        let user_balance_key = DataKey::UserBalance(user.clone(), asset.clone());
        let mut user_balance: UserBalance = env
            .storage()
            .persistent()
            .get(&user_balance_key)
            .unwrap_or_else(|| UserBalance {
                available: 0,
                locked: 0,
            });
        user_balance.available += amount;
        env.storage().persistent().set(&user_balance_key, &user_balance);

        // Update vault's total balance
        let vault_balance_key = DataKey::VaultBalance(asset.clone());
        let mut vault_balance: VaultBalance = env
            .storage()
            .persistent()
            .get(&vault_balance_key)
            .unwrap_or_else(|| VaultBalance {
                total_deposited: 0,
                total_locked: 0,
            });
        vault_balance.total_deposited += amount;
        env.storage().persistent().set(&vault_balance_key, &vault_balance);

        // Exit reentrancy guard before external interactions are complete
        Self::exit_reentrancy_guard(&env);

        // Emit Deposited event
        env.events()
            .publish((get_event_topic_by_string(&env, "Deposited"),), (user, amount, asset));

        Ok(())
    }

    /// Withdraw unused tokens from the vault
    /// Only available funds (not locked in active bets) can be withdrawn
    pub fn withdraw(env: Env, user: Address, amount: i128, asset: Address) -> Result<(), VaultError> {
        if amount <= 0 {
            return Err(VaultError::InvalidAmount);
        }

        Self::ensure_not_paused(&env).map_err(|_| VaultError::InvalidAmount)?;
        user.require_auth();

        Self::enter_reentrancy_guard(&env)?;

        // Check user has sufficient available balance
        let user_balance_key = DataKey::UserBalance(user.clone(), asset.clone());
        let mut user_balance: UserBalance = env
            .storage()
            .persistent()
            .get(&user_balance_key)
            .ok_or(VaultError::InsufficientBalance)?;

        if user_balance.available < amount {
            return Err(VaultError::InsufficientBalance);
        }

        // Update balances
        user_balance.available -= amount;
        env.storage().persistent().set(&user_balance_key, &user_balance);

        let vault_balance_key = DataKey::VaultBalance(asset.clone());
        let mut vault_balance: VaultBalance = env
            .storage()
            .persistent()
            .get(&vault_balance_key)
            .ok_or(VaultError::InsufficientBalance)?;
        vault_balance.total_deposited -= amount;
        env.storage().persistent().set(&vault_balance_key, &vault_balance);

        // Transfer tokens back to user (after all state changes - checks-effects-interactions)
        let client = token::Client::new(&env, &asset);
        client.transfer(&env.current_contract_address(), &user, &amount);

        Self::exit_reentrancy_guard(&env);

        // Emit Withdrawn event
        env.events()
            .publish((get_event_topic_by_string(&env, "Withdrawn"),), (user, amount, asset));

        Ok(())
    }

    /// Lock funds for an active bet - only callable by the betting contract
    /// Moves funds from available to locked state
    pub fn lock_for_bet(env: Env, user: Address, amount: i128, asset: Address, match_id: u64) -> Result<(), VaultError> {
        if amount <= 0 {
            return Err(VaultError::InvalidAmount);
        }

        Self::ensure_not_paused(&env).map_err(|_| VaultError::InvalidAmount)?;
        // Ensure only the betting contract can call this
        Self::ensure_betting_contract(&env).map_err(|_| VaultError::UnauthorizedBettingContract)?;

        Self::enter_reentrancy_guard(&env)?;

        // Check sufficient available balance
        let user_balance_key = DataKey::UserBalance(user.clone(), asset.clone());
        let mut user_balance: UserBalance = env
            .storage()
            .persistent()
            .get(&user_balance_key)
            .ok_or(VaultError::InsufficientBalance)?;

        if user_balance.available < amount {
            return Err(VaultError::InsufficientBalance);
        }

        // Create lock record for this bet
        let lock_key = DataKey::LockedBet(match_id, user.clone(), asset.clone());
        if env.storage().persistent().has(&lock_key) {
            return Err(VaultError::InvalidAmount); // Bet already locked
        }
        env.storage().persistent().set(&lock_key, &amount);

        // Update user's balances
        user_balance.available -= amount;
        user_balance.locked += amount;
        env.storage().persistent().set(&user_balance_key, &user_balance);

        // Update vault's totals
        let vault_balance_key = DataKey::VaultBalance(asset.clone());
        let mut vault_balance: VaultBalance = env
            .storage()
            .persistent()
            .get(&vault_balance_key)
            .ok_or(VaultError::InsufficientBalance)?;
        vault_balance.total_locked += amount;
        env.storage().persistent().set(&vault_balance_key, &vault_balance);

        Self::exit_reentrancy_guard(&env);

        // Emit Locked event
        env.events()
            .publish((get_event_topic_by_string(&env, "Locked"),), (user, amount, asset, match_id));

        Ok(())
    }

    /// Payout winnings to a winner - only callable by the betting contract
    /// Releases locked funds and transfers them to the winner
    pub fn payout(env: Env, winner: Address, amount: i128, asset: Address, match_id: u64) -> Result<(), VaultError> {
        if amount <= 0 {
            return Err(VaultError::InvalidAmount);
        }

        Self::ensure_not_paused(&env).map_err(|_| VaultError::InvalidAmount)?;
        Self::ensure_betting_contract(&env).map_err(|_| VaultError::UnauthorizedBettingContract)?;

        Self::enter_reentrancy_guard(&env)?;

        // Get and remove the lock record
        let lock_key = DataKey::LockedBet(match_id, winner.clone(), asset.clone());
        let locked_amount: i128 = env
            .storage()
            .persistent()
            .get(&lock_key)
            .ok_or(VaultError::BetLockNotFound)?;
        env.storage().persistent().remove(&lock_key);

        // Update user's balances
        let user_balance_key = DataKey::UserBalance(winner.clone(), asset.clone());
        let mut user_balance: UserBalance = env
            .storage()
            .persistent()
            .get(&user_balance_key)
            .ok_or(VaultError::InsufficientBalance)?;
        user_balance.locked -= locked_amount;
        user_balance.available += amount; // Add winnings to available balance
        env.storage().persistent().set(&user_balance_key, &user_balance);

        // Update vault's totals
        let vault_balance_key = DataKey::VaultBalance(asset.clone());
        let mut vault_balance: VaultBalance = env
            .storage()
            .persistent()
            .get(&vault_balance_key)
            .ok_or(VaultError::InsufficientBalance)?;
        vault_balance.total_locked -= locked_amount;
        env.storage().persistent().set(&vault_balance_key, &vault_balance);

        // Transfer the payout (after all state updates)
        let client = token::Client::new(&env, &asset);
        client.transfer(&env.current_contract_address(), &winner, &amount);

        Self::exit_reentrancy_guard(&env);

        // Emit PaidOut event
        env.events()
            .publish((get_event_topic_by_string(&env, "PaidOut"),), (winner, amount, asset, match_id));

        Ok(())
    }

    /// Get the total vault balance for a specific asset
    pub fn get_vault_balance(env: Env, asset: Address) -> i128 {
        let vault_balance: VaultBalance = env
            .storage()
            .persistent()
            .get(&DataKey::VaultBalance(asset))
            .unwrap_or_else(|| VaultBalance {
                total_deposited: 0,
                total_locked: 0,
            });
        vault_balance.total_deposited
    }

    /// Get a specific user's balance for an asset
    pub fn get_user_balance(env: Env, user: Address, asset: Address) -> UserBalance {
        env
            .storage()
            .persistent()
            .get(&DataKey::UserBalance(user, asset))
            .unwrap_or_else(|| UserBalance {
                available: 0,
                locked: 0,
            })
    }

    /// Check if funds are locked for a specific bet
    pub fn is_locked_for_bet(env: Env, match_id: u64, user: Address, asset: Address) -> bool {
        env.storage().persistent().has(&DataKey::LockedBet(match_id, user, asset))
    }

    /// Emergency withdraw of an asset (timelocked)
    pub fn emergency_withdraw(env: Env, asset: Address, to: Address, amount: i128) -> Result<(), VaultError> {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();

        let key = DataKey::PendingWithdrawal(asset.clone());
        if let Some(pending) = env.storage().instance().get::<_, PendingWithdrawal>(&key) {
            if pending.to != to || pending.amount != amount {
                return Err(VaultError::MismatchPendingData);
            }
            if env.ledger().timestamp() < pending.pending_until {
                return Err(VaultError::TimelockNotExpired);
            }

            let client = token::Client::new(&env, &asset);
            client.transfer(&env.current_contract_address(), &to, &amount);
            env.storage().instance().remove(&key);

            let reason_hash = soroban_sdk::BytesN::from_array(&env, &[0; 32]);
            env.events().publish(
                (get_event_topic_by_string(&env, "EmergencyAction"),),
                (Symbol::new(&env, "emergency_withdraw"), asset, to, amount, reason_hash),
            );
            Ok(())
        } else {
            env.storage().instance().set(&key, &PendingWithdrawal {
                to: to.clone(),
                amount,
                pending_until: env.ledger().timestamp() + 24 * 60 * 60, // 24 hours
            });
            Ok(())
        }
    }

    /// Cancel a pending emergency withdrawal
    pub fn cancel_emergency_withdraw(env: Env, asset: Address) -> Result<(), VaultError> {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage().instance().remove(&DataKey::PendingWithdrawal(asset));
        Ok(())
    }

    /// Recover accidentally sent tokens (only for non-tracked tokens)
    pub fn recover_token(env: Env, asset: Address, to: Address, amount: i128) -> Result<(), VaultError> {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();

        let vault_balance_key = DataKey::VaultBalance(asset.clone());
        if let Some(vault_balance) = env.storage().persistent().get::<_, VaultBalance>(&vault_balance_key) {
            if vault_balance.total_deposited > 0 || vault_balance.total_locked > 0 {
                return Err(VaultError::TokenIsTracked);
            }
        }

        let client = token::Client::new(&env, &asset);
        client.transfer(&env.current_contract_address(), &to, &amount);

        let reason_hash = soroban_sdk::BytesN::from_array(&env, &[0; 32]);
        env.events().publish(
            (get_event_topic_by_string(&env, "EmergencyAction"),),
            (Symbol::new(&env, "recover_token"), asset, to, amount, reason_hash),
        );
        Ok(())
    }

    /// Transfer admin rights (timelocked)
    pub fn set_admin(env: Env, new_admin: Address) -> Result<(), VaultError> {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();

        let key = DataKey::PendingAdmin;
        if let Some(pending) = env.storage().instance().get::<_, PendingAdminData>(&key) {
            if pending.new_admin != new_admin {
                return Err(VaultError::MismatchPendingData);
            }
            if env.ledger().timestamp() < pending.pending_until {
                return Err(VaultError::TimelockNotExpired);
            }

            env.storage().instance().set(&DataKey::Admin, &new_admin);
            env.storage().instance().remove(&key);

            let reason_hash = soroban_sdk::BytesN::from_array(&env, &[0; 32]);
            env.events().publish(
                (get_event_topic_by_string(&env, "EmergencyAction"),),
                (Symbol::new(&env, "set_admin"), new_admin, reason_hash),
            );
            Ok(())
        } else {
            env.storage().instance().set(&key, &PendingAdminData {
                new_admin: new_admin.clone(),
                pending_until: env.ledger().timestamp() + 48 * 60 * 60, // 48 hours
            });
            Ok(())
        }
    }

    /// Cancel a pending admin transfer
    pub fn cancel_set_admin(env: Env) -> Result<(), VaultError> {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage().instance().remove(&DataKey::PendingAdmin);
        Ok(())
    }
}

#[cfg(test)]
mod test;