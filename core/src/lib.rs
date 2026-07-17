#![no_std]
use soroban_sdk::{contracterror, contracttype};

pub use soroban_sdk::{Address, Symbol};

/// Centralized platform error codes mapped cleanly across all child contract scopes
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum PlatformError {
    InternalError = 1,
    Unauthorized = 2,
    InvalidAmount = 3,
    ExpiredDeadline = 4,
    Overflow = 5,
    /// Caller holds fewer tokens than the requested debit.
    /// Emitted by `renaissance-betting` `place_bet` to enforce the
    /// acceptance criterion that bets only settle against real balances.
    InsufficientBalance = 6,
    Paused = 7,
}

/// Standardized tracking data configuration for interactive betting matches
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchMetadata {
    pub match_id: u64,
    pub player_one: Address,
    pub player_two: Address,
    pub asset_token: Address,
    pub total_pool: i128,
}

/// Helper function to generate clean, reusable system event topics
pub fn get_event_topic_by_string(env: &soroban_sdk::Env, topic: &str) -> Symbol {
    Symbol::new(env, topic)
}
