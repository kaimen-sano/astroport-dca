use astroport::asset::addr_validate_to_lower;
use cosmwasm_std::{Deps, StdResult};

use crate::state::{UserConfig, USER_CONFIG};

/// ## Description
/// Returns the configuration set for a user to override the default contract configuration.
///
/// The result is returned in a [`UserConfig`] object.
///
/// ## Params
/// * **deps** is an object of type [`Deps`].
///
/// * **user** is the users lowercase address of type [`String`].
pub fn get_user_config(deps: Deps, user: String) -> StdResult<UserConfig> {
    let user_address = addr_validate_to_lower(deps.api, &user)?;

    USER_CONFIG.load(deps.storage, &user_address)
}
