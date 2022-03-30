use cosmwasm_std::{Deps, StdResult};

use crate::state::{Config, CONFIG};

/// ## Description
/// Returns the contract configuration set by the factory address owner or contract instantiator.
///
/// The result is returned in a [`Config`] object.
///
/// ## Arguments
/// * `deps` - A [`Deps`] that contains the dependencies.
pub fn get_config(deps: Deps) -> StdResult<Config> {
    CONFIG.load(deps.storage)
}
