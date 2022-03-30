use cosmwasm_std::{attr, Decimal, DepsMut, MessageInfo, Response};

use crate::{
    error::ContractError,
    state::{UserConfig, USER_CONFIG},
};

/// ## Description
/// Updates a users configuration with the specified parameters.
///
/// Returns a [`ContractError`] as a failure, otherwise returns a [`Response`] with the specified
/// attributes if the operation was successful.
/// ## Arguments
/// * `deps` - A [`DepsMut`] that contains the dependencies.
///
/// * `info` - A [`MessageInfo`] from the sender who wants to update their user configuration.
///
/// * `max_hops` - A `u8` value wrapped in an [`Option`] which represents the new maximum amount of
/// hops per DCA purchase. If `None`, the user will use the default config set by the contract.
///
/// * `max_spread` - A [`Decimal`] value wrapped in an [`Option`] which represents the new maximum
/// spread for each DCA purchase. If `None`, the user will use the config set by the contract.
pub fn update_user_config(
    deps: DepsMut,
    info: MessageInfo,
    max_hops: Option<u32>,
    max_spread: Option<Decimal>,
) -> Result<Response, ContractError> {
    let config = USER_CONFIG
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();

    let new_config = UserConfig {
        max_hops,
        max_spread,
        ..config
    };

    USER_CONFIG.save(deps.storage, &info.sender, &new_config)?;

    Ok(Response::new().add_attributes(vec![attr("action", "update_user_config")]))
}
