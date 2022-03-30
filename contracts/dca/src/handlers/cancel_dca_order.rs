use astroport::{asset::AssetInfo, dca::DcaInfo};
use cosmwasm_std::{attr, BankMsg, Coin, DepsMut, MessageInfo, Response};

use crate::{error::ContractError, state::USER_DCA};

/// ## Description
/// Cancels a users DCA purchase so that it will no longer be fulfilled.
///
/// Returns the `initial_asset` back to the user.
///
/// Returns a [`ContractError`] as a failure, otherwise returns a [`Response`] with the specified
/// attributes if the operation was successful
/// ## Params
/// * **deps** is an object of type [`DepsMut`].
///
/// * **info** is an object of type [`MessageInfo`] which contains a uusd tip to add.
///
/// * `initial_asset` The [`AssetInfo`] which the user wants to cancel the DCA order for.
pub fn cancel_dca_order(
    deps: DepsMut,
    info: MessageInfo,
    initial_asset: AssetInfo,
) -> Result<Response, ContractError> {
    let mut funds = Vec::new();

    // remove order from user dca's, and add any native token funds for `initial_asset` into the `funds`.
    USER_DCA.update(
        deps.storage,
        &info.sender,
        |orders| -> Result<Vec<DcaInfo>, ContractError> {
            let mut orders = orders.ok_or(ContractError::NonexistentSwap {})?;

            let order_position = orders
                .iter()
                .position(|order| order.initial_asset.info == initial_asset)
                .ok_or(ContractError::NonexistentSwap {})?;

            let removed_order = &orders[order_position];
            if let AssetInfo::NativeToken { denom } = &removed_order.initial_asset.info {
                funds.push(BankMsg::Send {
                    to_address: info.sender.to_string(),
                    amount: vec![Coin {
                        amount: removed_order.initial_asset.amount,
                        denom: denom.clone(),
                    }],
                })
            }

            orders.remove(order_position);

            Ok(orders)
        },
    )?;

    Ok(Response::new()
        .add_messages(funds)
        .add_attributes(vec![attr("action", "cancel_dca_order")]))
}
