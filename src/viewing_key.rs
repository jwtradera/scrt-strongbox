use std::fmt;

use base64::{engine::general_purpose, Engine};
use schemars::JsonSchema;
use secret_toolkit_crypto::{sha_256, Prng, SHA256_HASH_SIZE};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;

use cosmwasm_std::{CanonicalAddr, Env};

pub const VIEWING_KEY_SIZE: usize = SHA256_HASH_SIZE;
pub const VIEWING_KEY_PREFIX: &str = "strongbox_key_";

fn ct_slice_compare(s1: &[u8], s2: &[u8]) -> bool {
    bool::from(s1.ct_eq(s2))
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
pub struct ViewingKey(pub String);

impl ViewingKey {
    pub fn check_viewing_key(&self, hashed_pw: &[u8]) -> bool {
        let mine_hashed = sha_256(&self.as_bytes());
        ct_slice_compare(&mine_hashed, hashed_pw)
    }

    pub fn new(env: &Env, sender: &CanonicalAddr, seed: &[u8], entropy: &[u8]) -> Self {
        // 16 here represents the lengths in bytes of the block height and time.
        let entropy_len = 16 + sender.len() + entropy.len();
        let mut rng_entropy = Vec::with_capacity(entropy_len);
        rng_entropy.extend_from_slice(&env.block.height.to_be_bytes());
        rng_entropy.extend_from_slice(&env.block.time.to_string().as_bytes());
        rng_entropy.extend_from_slice(&sender.0);
        rng_entropy.extend_from_slice(entropy);

        let mut rng = Prng::new(seed, &rng_entropy);

        let rand_slice = rng.rand_bytes();

        let key = sha_256(&rand_slice);

        Self(VIEWING_KEY_PREFIX.to_string() + &general_purpose::STANDARD.encode(&key))
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl fmt::Display for ViewingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
