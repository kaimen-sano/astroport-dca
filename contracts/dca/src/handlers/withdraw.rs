use astroport::asset::UUSD_DENOM;
use cosmwasm_std::{attr, coins, BankMsg, DepsMut, MessageInfo, Response, Uint128};

use crate::{error::ContractError, state::USER_CONFIG};

/// ## Description
/// Withdraws a users bot tip from the contract.
/// Returns a [`ContractError`] as a failure, otherwise returns a [`Response`] with the specified
/// attributes if the operation was successful
/// ## Params
/// * **deps** is an object of type [`DepsMut`].
///
/// * **info** is an object of type [`MessageInfo`].
///
/// * **amount** is a [`Uint128`] representing the amount of uusd to send back to the user.
pub fn withdraw(
    deps: DepsMut,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let mut config = USER_CONFIG
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();

    config.tip_balance = config.tip_balance.checked_sub(amount)?;

    // optimization: if the user is withdrawing all their tip, they are probably never going to
    // interact with the contract again. In this case, we can delete their config to save space
    // otherwise, we save their new configuration
    match config.tip_balance.is_zero() {
        true => {
            USER_CONFIG.remove(deps.storage, &info.sender);
            Ok(())
        }
        false => USER_CONFIG.save(deps.storage, &info.sender, &config),
    }?;

    Ok(Response::new()
        .add_attributes(vec![
            attr("action", "withdraw"),
            attr("tip_removed", amount),
        ])
        .add_message(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: coins(amount.u128(), UUSD_DENOM),
        }))
}
