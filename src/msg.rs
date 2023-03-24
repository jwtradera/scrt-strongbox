use cosmwasm_std::Addr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::viewing_key::ViewingKey;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub serenity_seed: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    UpdateStrongbox {
        strongbox: String,
    },
    CreateViewingKey {
        viewer: Addr,
        entropy: String,
        padding: Option<String>,
    },

    TransferOwnership {
        new_owner: Addr,
    },
    RevokeViewingKey {
        viewer: Addr,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // GetStrongbox returns the current strongbox
    GetStrongbox { behalf: Addr, key: String },
}

impl QueryMsg {
    pub fn get_validation_params(&self) -> (Vec<&Addr>, ViewingKey) {
        match self {
            Self::GetStrongbox { behalf, key, .. } => (vec![behalf], ViewingKey(key.clone())),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StrongboxResponse {
    pub strongbox: String,
}
