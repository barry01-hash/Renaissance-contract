#![no_std]

//! `renaissance-oracle` football match results oracle contract.
//!
//! A dedicated oracle contract that accepts verified football match results
//! from authorized data providers and makes them available on-chain for the
//! betting and rewards contracts. Prevents single-point oracle failure through
//! multi-sig confirmation (2-of-N required to finalize results).

use renaissance_core::{get_event_topic_by_string, PlatformError};
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Vec};

// ── Match Result Structure ────────────────────────────────────────────────────

/// Stores the final verified football match result.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchResult {
    pub match_id: u64,
    pub home_score: u32,
    pub away_score: u32,
    pub started_at: u64,
    pub finished_at: u64,
    pub finalized: bool,
}

/// Stores the intermediate state of a match result that's pending confirmation.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingResult {
    pub match_id: u64,
    pub home_score: u32,
    pub away_score: u32,
    pub started_at: u64,
    pub finished_at: u64,
    pub submitter: Address,
    pub confirmations: Vec<Address>, // Tracks which oracles have confirmed
}

// ── Storage keys ─────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    /// Admin address that can manage oracle list
    Admin,
    /// Paused state flag
    Paused,
    /// Set of authorized oracle addresses
    Oracles,
    /// Pending match result waiting for confirmations
    PendingResult(u64),
    /// Finalized match result
    FinalizedResult(u64),
}

// ── Custom errors ─────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum OracleError {
    /// Caller is not an authorized oracle
    OracleUnauthorized = 100,
    /// Match result has already been submitted
    ResultAlreadyExists = 101,
    /// Cannot submit result outside the match window
    InvalidMatchWindow = 102,
    /// Match result not found
    ResultNotFound = 103,
    /// Cannot confirm your own submission
    CannotConfirmOwnSubmission = 104,
    /// Already confirmed this result
    AlreadyConfirmed = 105,
    /// Match is already finalized
    MatchAlreadyFinalized = 106,
    /// Insufficient confirmations to finalize
    InsufficientConfirmations = 107,
}

// ── Contract Implementation ───────────────────────────────────────────────────

#[contract]
pub struct FootballOracleContract;

#[contractimpl]
impl FootballOracleContract {
    /// Helper to check that caller is an authorized oracle
    fn ensure_oracle(env: &Env) -> Result<Vec<Address>, PlatformError> {
        let oracles: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Oracles)
            .ok_or(PlatformError::Unauthorized)?;
            
        let caller = env.current_contract_address();
        if !oracles.contains(&caller) {
            return Err(PlatformError::Unauthorized);
        }
        Ok(oracles)
    }

    /// Helper to check that contract is not paused
    fn ensure_not_paused(env: &Env) -> Result<(), PlatformError> {
        if env.storage().instance().get(&DataKey::Paused).unwrap_or(false) {
            return Err(PlatformError::Paused);
        }
        Ok(())
    }

    /// One-time initialization of the oracle contract.
    /// Sets up the admin and initial list of authorized oracles.
    pub fn initialize(env: Env, admin: Address, initial_oracles: Vec<Address>) -> Result<(), PlatformError> {
        admin.require_auth();

        if env.storage().instance().has(&DataKey::Admin) {
            return Err(PlatformError::InternalError);
        }

        // Minimum 2 oracles required for 2-of-N multi-sig
        if initial_oracles.len() < 2 {
            return Err(PlatformError::InternalError);
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Oracles, &initial_oracles);
        env.storage().instance().set(&DataKey::Paused, &false);

        // Extend TTL for instance storage
        const MAX_TTL: u32 = 518400; // 30 days in ledgers (~5s per ledger)
        env.storage()
            .instance()
            .extend_ttl(MAX_TTL, MAX_TTL);

        Ok(())
    }

    /// Add a new authorized oracle (admin only)
    pub fn add_oracle(env: Env, new_oracle: Address) -> Result<(), PlatformError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(PlatformError::Unauthorized)?;
        admin.require_auth();

        let mut oracles: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Oracles)
            .ok_or(PlatformError::InternalError)?;

        if !oracles.contains(&new_oracle) {
            oracles.push_back(new_oracle);
            env.storage().instance().set(&DataKey::Oracles, &oracles);
        }

        Ok(())
    }

    /// Submit a match result for confirmation (oracle only)
    /// Validates that submission is within the valid match window
    pub fn submit_result(
        env: Env,
        oracle: Address,
        match_id: u64,
        home_score: u32,
        away_score: u32,
        started_at: u64,
        finished_at: u64,
    ) -> Result<(), OracleError> {
        Self::ensure_not_paused(&env).map_err(|_| OracleError::OracleUnauthorized)?;
        
        let oracles = Self::ensure_oracle(&env).map_err(|_| OracleError::OracleUnauthorized)?;
        oracle.require_auth();
        let caller = oracle;
        
        // Validate caller is in the oracle list
        if !oracles.contains(&caller) {
            return Err(OracleError::OracleUnauthorized);
        }

        // Check if match is already finalized
        if env.storage().persistent().has(&DataKey::FinalizedResult(match_id)) {
            return Err(OracleError::MatchAlreadyFinalized);
        }

        // Check if result is already pending
        if env.storage().persistent().has(&DataKey::PendingResult(match_id)) {
            return Err(OracleError::ResultAlreadyExists);
        }

        // Validate match window timestamps
        let now = env.ledger().timestamp();
        if started_at >= finished_at || finished_at > now {
            return Err(OracleError::InvalidMatchWindow);
        }

        // Create pending result
        let pending = PendingResult {
            match_id,
            home_score,
            away_score,
            started_at,
            finished_at,
            submitter: caller.clone(),
            confirmations: Vec::new(&env),
        };

        env.storage()
            .persistent()
            .set(&DataKey::PendingResult(match_id), &pending);

        // Emit ResultSubmitted event
        env.events()
            .publish((get_event_topic_by_string(&env, "ResultSubmitted"),), (match_id, caller));

        Ok(())
    }

    /// Confirm a pending match result (oracle only).
    /// When the second confirmation is received, the result is finalized.
    pub fn confirm_result(env: Env, oracle: Address, match_id: u64) -> Result<(), OracleError> {
        Self::ensure_not_paused(&env).map_err(|_| OracleError::OracleUnauthorized)?;
        
        let oracles = Self::ensure_oracle(&env).map_err(|_| OracleError::OracleUnauthorized)?;
        oracle.require_auth();
        let caller = oracle;
        
        // Validate caller is in the oracle list
        if !oracles.contains(&caller) {
            return Err(OracleError::OracleUnauthorized);
        }

        // Check if match is already finalized
        if env.storage().persistent().has(&DataKey::FinalizedResult(match_id)) {
            return Err(OracleError::MatchAlreadyFinalized);
        }

        // Get pending result
        let mut pending: PendingResult = env
            .storage()
            .persistent()
            .get(&DataKey::PendingResult(match_id))
            .ok_or(OracleError::ResultNotFound)?;

        // Cannot confirm your own submission
        if pending.submitter == caller {
            return Err(OracleError::CannotConfirmOwnSubmission);
        }

        // Cannot confirm twice
        if pending.confirmations.contains(&caller) {
            return Err(OracleError::AlreadyConfirmed);
        }

        // Add confirmation
        pending.confirmations.push_back(caller.clone());
        env.storage()
            .persistent()
            .set(&DataKey::PendingResult(match_id), &pending);

        // Emit ResultConfirmed event
        env.events()
            .publish((get_event_topic_by_string(&env, "ResultConfirmed"),), (match_id, caller));

        // Check if we have enough confirmations (2-of-N, so at least 1 confirmation since submitter is one)
        if pending.confirmations.len() >= 1 {
            // Finalize the result
            let finalized = MatchResult {
                match_id,
                home_score: pending.home_score,
                away_score: pending.away_score,
                started_at: pending.started_at,
                finished_at: pending.finished_at,
                finalized: true,
            };

            env.storage()
                .persistent()
                .set(&DataKey::FinalizedResult(match_id), &finalized);

            // Remove from pending storage
            env.storage()
                .persistent()
                .remove(&DataKey::PendingResult(match_id));

            // Emit ResultFinalized event
            env.events()
                .publish((get_event_topic_by_string(&env, "ResultFinalized"),), match_id);
        }

        Ok(())
    }

    /// Get a finalized match result
    pub fn get_result(env: Env, match_id: u64) -> Result<MatchResult, OracleError> {
        let result: MatchResult = env
            .storage()
            .persistent()
            .get(&DataKey::FinalizedResult(match_id))
            .ok_or(OracleError::ResultNotFound)?;
            
        Ok(result)
    }

    /// Check if a match result is finalized
    pub fn is_finalized(env: Env, match_id: u64) -> bool {
        env.storage().persistent().has(&DataKey::FinalizedResult(match_id))
    }

    /// Get the list of all authorized oracles
    pub fn get_oracles(env: Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::Oracles)
            .unwrap_or_else(|| Vec::new(&env))
    }
}

#[cfg(test)]
mod test;