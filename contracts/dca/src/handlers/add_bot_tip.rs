use astroport::asset::UUSD_DENOM;
use cosmwasm_std::{attr, DepsMut, MessageInfo, Response, StdResult};

use crate::{
    error::ContractError,
    state::{UserConfig, USER_CONFIG},
};

/// ## Description
/// Adds a tip to the contract for a users DCA purchases.
/// Returns a [`ContractError`] as a failure, otherwise returns a [`Response`] with the specified
/// attributes if the operation was successful
/// ## Params
/// * **deps** is an object of type [`DepsMut`].
///
/// * **info** is an object of type [`MessageInfo`] which contains a uusd tip to add.
pub fn add_bot_tip(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let amount = info
        .funds
        .iter()
        .find(|coin| coin.denom == UUSD_DENOM)
        .ok_or(ContractError::InvalidZeroAmount {})?
        .amount;

    // update user tip in contract
    USER_CONFIG.update(
        deps.storage,
        &info.sender,
        |config| -> StdResult<UserConfig> {
            let mut config = config.unwrap_or_default();

            config.tip_balance = config.tip_balance.checked_add(amount)?;

            Ok(config)
        },
    )?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "add_bot_tip"),
        attr("tip_amount", amount),
    ]))
}
