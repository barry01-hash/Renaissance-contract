#![no_std]

use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Symbol};

const COUNTER: Symbol = symbol_short!("COUNTER");

#[contract]
pub struct CounterContract;

#[contractimpl]
impl CounterContract {
    pub fn increment(env: Env, address: Address) -> i32 {
        address.require_auth();
        let count = Self::get_count(env.clone()).saturating_add(1);
        env.storage().persistent().set(&COUNTER, &count);
        count
    }

    pub fn decrement(env: Env, address: Address) -> i32 {
        address.require_auth();
        let count = Self::get_count(env.clone()).saturating_sub(1);
        env.storage().persistent().set(&COUNTER, &count);
        count
    }

    pub fn get_count(env: Env) -> i32 {
        env.storage().persistent().get(&COUNTER).unwrap_or(0)
    }
}
