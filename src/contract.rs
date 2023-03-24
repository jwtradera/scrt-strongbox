use base64::engine::{general_purpose, Engine};
use cosmwasm_std::{
    entry_point, to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult,
};
use secret_toolkit_crypto::sha_256;

use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, StrongboxResponse};
use crate::state::{
    config, config_read, read_viewing_key, revoke_viewing_key, write_viewing_key, State,
    ENTROPY_LEN, INITIAL_SEED_LEN,
};
use crate::viewing_key::{ViewingKey, VIEWING_KEY_SIZE};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let initial_seed = msg.serenity_seed;
    // Validate length
    if initial_seed.len() != INITIAL_SEED_LEN {
        return Err(StdError::generic_err("You need to provide valid seed"));
    }

    let sender_address = deps.api.addr_canonicalize(info.sender.as_str())?;

    let state = State {
        strongbox: String::from(""),
        owner: sender_address,
        serenity_seed: sha_256(&general_purpose::STANDARD.encode(&initial_seed).as_bytes())
            .to_vec(),
        entropy_hashes: vec![],
    };

    config(deps.storage).save(&state)?;

    deps.api
        .debug(format!("Contract was initialized by {}", info.sender).as_str());
    Ok(Response::default())
}

#[entry_point]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::UpdateStrongbox { strongbox } => try_update_strongbox(deps, info, strongbox),
        ExecuteMsg::CreateViewingKey {
            entropy, viewer, ..
        } => try_create_viewing_key(deps, env, info, entropy, viewer),

        ExecuteMsg::TransferOwnership { new_owner } => {
            try_transfer_ownership(deps, info, new_owner)
        }
        ExecuteMsg::RevokeViewingKey { viewer } => try_revoke_viewing_key(deps, info, viewer),
    }
}

pub fn try_update_strongbox(
    deps: DepsMut,
    info: MessageInfo,
    strongbox: String,
) -> StdResult<Response> {
    let signer = deps.api.addr_canonicalize(info.sender.as_str())?;

    config(deps.storage).update(|mut state| {
        if signer != state.owner {
            return Err(StdError::generic_err("You are not allowed"));
        }
        state.strongbox = strongbox;
        Ok(state)
    })?;

    deps.api.debug("Strongbox updated successfully");
    Ok(Response::default())
}

pub fn try_create_viewing_key(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    entropy: String,
    viewer: Addr,
) -> StdResult<Response> {
    // Validate length
    if entropy.len() != ENTROPY_LEN {
        return Err(StdError::generic_err("You need to provide valid entropy"));
    }

    // Validate owner
    let config_state: State = config_read(deps.storage).load()?;
    let sender = deps.api.addr_canonicalize(info.sender.as_str())?;
    if sender != config_state.owner {
        return Err(StdError::generic_err("You are not allowed"));
    }

    // Validate duplicate entropy
    let entropy_hash = to_binary(&sha_256(&entropy.as_bytes()))?;
    let duplicated = config_state
        .entropy_hashes
        .iter()
        .find(|&x| x.eq(&entropy_hash));

    // Store entropy hash
    config(deps.storage).update(|mut state| {
        // Check entropy is duplicated
        if duplicated.is_some() {
            return Err(StdError::generic_err("You need to use another entropy"));
        }

        state.entropy_hashes.push(entropy_hash);
        Ok(state)
    })?;

    // Generate viewing key
    let prng_seed = config_state.serenity_seed;

    let key = ViewingKey::new(&env, &sender, &prng_seed, (&entropy).as_ref());
    write_viewing_key(
        deps.storage,
        &deps.api.addr_canonicalize(viewer.as_str())?,
        &key,
    );

    let response = Response::default().set_data(to_binary(&key)?);
    Ok(response)
}

pub fn try_transfer_ownership(
    deps: DepsMut,
    info: MessageInfo,
    new_owner: Addr,
) -> StdResult<Response> {
    let signer = deps.api.addr_canonicalize(info.sender.as_str())?;

    let new_owner_addr = deps.api.addr_canonicalize(new_owner.as_str())?;

    config(deps.storage).update(|mut state| {
        if signer != state.owner {
            return Err(StdError::generic_err("You are not allowed"));
        }

        state.owner = new_owner_addr;
        Ok(state)
    })?;

    deps.api.debug("Owner updated successfully");
    Ok(Response::default())
}

pub fn try_revoke_viewing_key(
    deps: DepsMut,
    info: MessageInfo,
    viewer: Addr,
) -> StdResult<Response> {
    // Validate owner
    let config_state: State = config_read(deps.storage).load()?;
    let sender = deps.api.addr_canonicalize(info.sender.as_str())?;
    if sender != config_state.owner {
        return Err(StdError::generic_err("You are not allowed"));
    }

    // Check viewing key exists
    let viewer_addr = deps.api.addr_canonicalize(viewer.as_str())?;
    let viewer_key = read_viewing_key(deps.storage, &viewer_addr);
    if viewer_key.is_none() {
        return Err(StdError::generic_err("Viewing key not exists"));
    }

    revoke_viewing_key(deps.storage, &viewer_addr);

    deps.api.debug("Viewing key revoked successfully");
    Ok(Response::default())
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    let (addresses, key) = msg.get_validation_params();

    for address in addresses {
        let canonical_addr = deps.api.addr_canonicalize(address.as_str())?;

        let expected_key = read_viewing_key(deps.storage, &canonical_addr);

        if expected_key.is_none() {
            // Checking the key will take significant time. We don't want to exit immediately if it isn't set
            // in a way which will allow to time the command and determine if a viewing key doesn't exist
            key.check_viewing_key(&[0u8; VIEWING_KEY_SIZE]);
        } else if key.check_viewing_key(expected_key.unwrap().as_slice()) {
            return match msg {
                QueryMsg::GetStrongbox { .. } => to_binary(&query_strongbox(deps)?),
            };
        }
    }

    Err(StdError::generic_err("Your viewing key does not matched"))
}

fn query_strongbox(deps: Deps) -> StdResult<StrongboxResponse> {
    let mut _strongbox = String::from("");
    let state = config_read(deps.storage).load()?;
    _strongbox = state.strongbox;

    return Ok(StrongboxResponse {
        strongbox: _strongbox,
    });
}

#[cfg(test)]
mod tests {

    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{from_binary, Coin, StdError, Uint128};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();
        let info = mock_info(
            "creator",
            &[Coin {
                denom: "earth".to_string(),
                amount: Uint128::new(1000),
            }],
        );
        let msg = InstantiateMsg {
            serenity_seed: String::from("init strongbox"),
        };

        // init action will be failed due to seed length
        let res = instantiate(deps.as_mut(), mock_env(), info, msg);
        let error_msg = match res {
            Err(StdError::GenericErr { msg }) => msg,
            _ => panic!("You need to provide valid seed"),
        };
        assert_eq!(error_msg, "You need to provide valid seed");

        let info = mock_info(
            "creator",
            &[Coin {
                denom: "earth".to_string(),
                amount: Uint128::new(1000),
            }],
        );
        let msg = InstantiateMsg {
            serenity_seed: String::from("r5ypLSFsvpFYFfbfv05USo7wMlFjvoGh"),
        };
        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());
    }

    #[test]
    fn update_strongbox() {
        let mut deps = mock_dependencies();
        let info = mock_info(
            "creator",
            &[Coin {
                denom: "earth".to_string(),
                amount: Uint128::new(1000),
            }],
        );
        let init_msg = InstantiateMsg {
            serenity_seed: String::from("r5ypLSFsvpFYFfbfv05USo7wMlFjvoGh"),
        };
        instantiate(deps.as_mut(), mock_env(), info, init_msg).unwrap();

        // not anyone can update
        let anyone_info = mock_info(
            "visitor1",
            &[Coin {
                denom: "earth".to_string(),
                amount: Uint128::new(1000),
            }],
        );
        let update_msg = ExecuteMsg::UpdateStrongbox {
            strongbox: String::from("Test strongbox"),
        };
        let res = execute(deps.as_mut(), mock_env(), anyone_info, update_msg);
        let error_msg = match res {
            Err(StdError::GenericErr { msg }) => msg,
            _ => panic!("You are not allowed"),
        };
        assert_eq!(error_msg, "You are not allowed");

        // owner can update
        let owner_info = mock_info(
            "creator",
            &[Coin {
                denom: "earth".to_string(),
                amount: Uint128::new(1000),
            }],
        );
        let msg = ExecuteMsg::UpdateStrongbox {
            strongbox: String::from("Test strongbox"),
        };
        let res = execute(deps.as_mut(), mock_env(), owner_info, msg).unwrap();
        assert_eq!(0, res.messages.len());
    }

    #[test]
    fn create_viewing_key() {
        let mut deps = mock_dependencies();
        let owner_info = mock_info(
            "creator",
            &[Coin {
                denom: "earth".to_string(),
                amount: Uint128::new(1000),
            }],
        );
        let init_msg = InstantiateMsg {
            serenity_seed: String::from("r5ypLSFsvpFYFfbfv05USo7wMlFjvoGh"),
        };
        instantiate(deps.as_mut(), mock_env(), owner_info, init_msg).unwrap();

        // entropy length should be fixed
        let anyone_info = mock_info(
            "visitor1",
            &[Coin {
                denom: "earth".to_string(),
                amount: Uint128::new(1000),
            }],
        );
        let create_vk_msg = ExecuteMsg::CreateViewingKey {
            viewer: Addr::unchecked(String::from("user1")),
            entropy: "supbro".to_string(),
            padding: None,
        };

        let res = execute(deps.as_mut(), mock_env(), anyone_info, create_vk_msg);
        let error_msg = match res {
            Err(StdError::GenericErr { msg }) => msg,
            _ => panic!("You need to provide valid entropy"),
        };
        assert_eq!(error_msg, "You need to provide valid entropy");

        // only owner can create viewing key
        let anyone_info = mock_info(
            "visitor2",
            &[Coin {
                denom: "earth".to_string(),
                amount: Uint128::new(1000),
            }],
        );
        let create_vk_msg = ExecuteMsg::CreateViewingKey {
            viewer: Addr::unchecked(String::from("user1")),
            entropy: "2418D8fZhQs8jIzuhiZ8".to_string(),
            padding: None,
        };
        let res = execute(deps.as_mut(), mock_env(), anyone_info, create_vk_msg);
        let error_msg = match res {
            Err(StdError::GenericErr { msg }) => msg,
            _ => panic!("You are not allowed"),
        };
        assert_eq!(error_msg, "You are not allowed");

        // owner can create viewing key
        let owner_info = mock_info(
            "creator",
            &[Coin {
                denom: "earth".to_string(),
                amount: Uint128::new(1000),
            }],
        );
        let create_vk_msg = ExecuteMsg::CreateViewingKey {
            viewer: Addr::unchecked(String::from("user2")),
            entropy: "2418D8fZhQs8jIzuhiZ8".to_string(),
            padding: None,
        };
        let res = execute(deps.as_mut(), mock_env(), owner_info, create_vk_msg).unwrap();
        let vk: ViewingKey = match from_binary(&res.data.unwrap()).unwrap() {
            Some(data) => data,
            _ => panic!("Unexpected result from handle"),
        };

        assert!(vk.as_bytes().len() > 0, "Viewing key not valid");

        // owner can't use same seed for create viewing key
        let owner_info = mock_info(
            "creator",
            &[Coin {
                denom: "earth".to_string(),
                amount: Uint128::new(1000),
            }],
        );
        let create_vk_msg = ExecuteMsg::CreateViewingKey {
            viewer: Addr::unchecked(String::from("user2")),
            entropy: "2418D8fZhQs8jIzuhiZ8".to_string(),
            padding: None,
        };
        let res = execute(deps.as_mut(), mock_env(), owner_info, create_vk_msg);
        let error_msg = match res {
            Err(StdError::GenericErr { msg }) => msg,
            _ => panic!("You need to use another entropy"),
        };
        assert_eq!(error_msg, "You need to use another entropy");
    }

    #[test]
    fn query_strongbox() {
        let mut deps = mock_dependencies();
        let owner_info = mock_info(
            "creator",
            &[Coin {
                denom: "earth".to_string(),
                amount: Uint128::new(1000),
            }],
        );
        let init_msg = InstantiateMsg {
            serenity_seed: String::from("r5ypLSFsvpFYFfbfv05USo7wMlFjvoGh"),
        };
        instantiate(deps.as_mut(), mock_env(), owner_info, init_msg).unwrap();

        let owner_info = mock_info(
            "creator",
            &[Coin {
                denom: "earth".to_string(),
                amount: Uint128::new(1000),
            }],
        );
        let update_msg = ExecuteMsg::UpdateStrongbox {
            strongbox: String::from("Test strongbox"),
        };
        execute(deps.as_mut(), mock_env(), owner_info, update_msg).unwrap();

        let owner_info = mock_info(
            "creator",
            &[Coin {
                denom: "earth".to_string(),
                amount: Uint128::new(1000),
            }],
        );
        let create_vk_msg = ExecuteMsg::CreateViewingKey {
            viewer: Addr::unchecked(String::from("user1")),
            entropy: "2418D8fZhQs8jIzuhiZ8".to_string(),
            padding: None,
        };
        let res = execute(deps.as_mut(), mock_env(), owner_info, create_vk_msg).unwrap();
        let vk: ViewingKey = from_binary(&res.data.unwrap()).unwrap();

        // other user can't use viewing key
        let query_msg = QueryMsg::GetStrongbox {
            behalf: Addr::unchecked(String::from("user2")),
            key: vk.to_string(),
        };
        let res = query(deps.as_ref(), mock_env(), query_msg);
        let error_msg = match res {
            Err(StdError::GenericErr { msg }) => msg,
            _ => panic!("Your viewing key does not matched"),
        };
        assert_eq!(error_msg, "Your viewing key does not matched");

        // correct user can use viewing key for query strongbox
        let query_msg = QueryMsg::GetStrongbox {
            behalf: Addr::unchecked(String::from("user1")),
            key: vk.to_string(),
        };
        let res = query(deps.as_ref(), mock_env(), query_msg).unwrap();
        let res: StrongboxResponse = from_binary(&res).unwrap();
        assert_eq!(res.strongbox, "Test strongbox");
    }

    #[test]
    fn transfer_ownership() {
        let mut deps = mock_dependencies();
        let owner_info = mock_info(
            "creator1",
            &[Coin {
                denom: "earth".to_string(),
                amount: Uint128::new(1000),
            }],
        );
        let init_msg = InstantiateMsg {
            serenity_seed: String::from("r5ypLSFsvpFYFfbfv05USo7wMlFjvoGh"),
        };
        instantiate(deps.as_mut(), mock_env(), owner_info, init_msg).unwrap();

        let owner_info = mock_info(
            "creator1",
            &[Coin {
                denom: "earth".to_string(),
                amount: Uint128::new(1000),
            }],
        );
        let update_msg = ExecuteMsg::TransferOwnership {
            new_owner: Addr::unchecked("creator2"),
        };
        execute(deps.as_mut(), mock_env(), owner_info, update_msg).unwrap();

        // Old owner can't update strongbox
        let old_owner_info = mock_info(
            "creator1",
            &[Coin {
                denom: "earth".to_string(),
                amount: Uint128::new(1000),
            }],
        );
        let update_msg = ExecuteMsg::UpdateStrongbox {
            strongbox: String::from("Test strongbox"),
        };
        let res = execute(deps.as_mut(), mock_env(), old_owner_info, update_msg);
        match res {
            Err(StdError::GenericErr { msg }) => msg,
            _ => panic!("You are not allowed"),
        };

        // New owner can update strongbox
        let new_owner_info = mock_info(
            "creator2",
            &[Coin {
                denom: "earth".to_string(),
                amount: Uint128::new(1000),
            }],
        );
        let update_msg = ExecuteMsg::UpdateStrongbox {
            strongbox: String::from("Test strongbox"),
        };
        execute(deps.as_mut(), mock_env(), new_owner_info, update_msg).unwrap();
    }

    #[test]
    fn revoke_viewing_key() {
        let mut deps = mock_dependencies();
        let owner_info = mock_info(
            "creator",
            &[Coin {
                denom: "earth".to_string(),
                amount: Uint128::new(1000),
            }],
        );
        let init_msg = InstantiateMsg {
            serenity_seed: String::from("r5ypLSFsvpFYFfbfv05USo7wMlFjvoGh"),
        };
        instantiate(deps.as_mut(), mock_env(), owner_info, init_msg).unwrap();

        let owner_info = mock_info(
            "creator",
            &[Coin {
                denom: "earth".to_string(),
                amount: Uint128::new(1000),
            }],
        );
        let update_msg = ExecuteMsg::UpdateStrongbox {
            strongbox: String::from("Test strongbox"),
        };
        execute(deps.as_mut(), mock_env(), owner_info, update_msg).unwrap();

        let owner_info = mock_info(
            "creator",
            &[Coin {
                denom: "earth".to_string(),
                amount: Uint128::new(1000),
            }],
        );
        let create_vk_msg = ExecuteMsg::CreateViewingKey {
            viewer: Addr::unchecked(String::from("user1")),
            entropy: "2418D8fZhQs8jIzuhiZ8".to_string(),
            padding: None,
        };
        let res = execute(deps.as_mut(), mock_env(), owner_info, create_vk_msg).unwrap();
        let vk: ViewingKey = from_binary(&res.data.unwrap()).unwrap();

        // owner can revoke viewing key
        let owner_info = mock_info(
            "creator",
            &[Coin {
                denom: "earth".to_string(),
                amount: Uint128::new(1000),
            }],
        );
        let revoke_msg = ExecuteMsg::RevokeViewingKey {
            viewer: Addr::unchecked(String::from("user1")),
        };
        execute(deps.as_mut(), mock_env(), owner_info, revoke_msg).unwrap();

        // user can't view strongbox with revoked key
        let query_msg = QueryMsg::GetStrongbox {
            behalf: Addr::unchecked(String::from("user1")),
            key: vk.to_string(),
        };
        let res = query(deps.as_ref(), mock_env(), query_msg);
        let error_msg = match res {
            Err(StdError::GenericErr { msg }) => msg,
            _ => panic!("Your viewing key does not matched"),
        };
        assert_eq!(error_msg, "Your viewing key does not matched");
    }
}
