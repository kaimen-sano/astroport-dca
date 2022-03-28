use cosmwasm_std::{Deps, StdResult};

use crate::state::{Config, CONFIG};

/// ## Description
/// Returns the configuration set for the contract.
///
/// The result is returned in a [`Config`] object.
///
/// ## Params
/// * **deps** is an object of type [`Deps`].
pub fn get_config(deps: Deps) -> StdResult<Config> {
    CONFIG.load(deps.storage)
}
