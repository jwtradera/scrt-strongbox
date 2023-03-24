use schemars::JsonSchema;
use secret_toolkit_crypto::sha_256;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Binary, CanonicalAddr, Storage};
use cosmwasm_storage::{
    singleton, singleton_read, PrefixedStorage, ReadonlyPrefixedStorage, ReadonlySingleton,
    Singleton,
};

use crate::viewing_key::ViewingKey;

pub static INITIAL_SEED_LEN: usize = 32;
pub static ENTROPY_LEN: usize = 20;

pub static CONFIG_KEY: &[u8] = b"strongbox_config";
pub static PREFIX_VIEWING_KEY: &[u8] = b"strongbox_view_key";

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
pub struct State {
    pub owner: CanonicalAddr,
    pub strongbox: String,
    pub serenity_seed: Vec<u8>,
    pub entropy_hashes: Vec<Binary>,
}

pub fn config(storage: &mut dyn Storage) -> Singleton<State> {
    singleton(storage, CONFIG_KEY)
}

pub fn config_read(storage: &dyn Storage) -> ReadonlySingleton<State> {
    singleton_read(storage, CONFIG_KEY)
}

pub fn read_viewing_key(store: &dyn Storage, owner: &CanonicalAddr) -> Option<Vec<u8>> {
    let user_key_store = ReadonlyPrefixedStorage::new(store, PREFIX_VIEWING_KEY);
    user_key_store.get(owner.as_slice())
}

pub fn write_viewing_key(store: &mut dyn Storage, owner: &CanonicalAddr, key: &ViewingKey) {
    let mut user_key_store = PrefixedStorage::new(store, PREFIX_VIEWING_KEY);
    user_key_store.set(owner.as_slice(), &sha_256(key.as_bytes()));
}

pub fn revoke_viewing_key(store: &mut dyn Storage, owner: &CanonicalAddr) {
    let mut user_key_store = PrefixedStorage::new(store, PREFIX_VIEWING_KEY);
    user_key_store.remove(owner.as_slice());
}
