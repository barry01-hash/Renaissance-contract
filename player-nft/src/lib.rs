#![no_std]

use soroban_sdk::{contract, contractimpl, Env, Symbol};

/// Initial contract boundary for player NFT functionality.
#[contract]
pub struct PlayerNftContract;

#[contractimpl]
impl PlayerNftContract {
    pub fn contract_name(env: Env) -> Symbol {
        Symbol::new(&env, "player_nft")
    }
}
