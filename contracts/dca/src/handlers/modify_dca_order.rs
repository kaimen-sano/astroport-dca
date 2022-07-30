use astroport::asset::{Asset, AssetInfo};
use cosmwasm_std::{attr, coins, BankMsg, DepsMut, Env, MessageInfo, Response, Uint128};

use crate::{error::ContractError, get_token_allowance::get_token_allowance, state::USER_DCA};

/// Stores a modified dca order new parameters
pub struct ModifyDcaOrderParameters {
    /// The users [`u64`] ID of the order.
    pub id: u64,
    /// The new [`Asset`] that is being spent to create DCA orders.
    pub new_initial_asset: Asset,
    /// The [`AssetInfo`] that is being purchased with `new_initial_asset`.
    pub new_target_asset: AssetInfo,
    /// The time in seconds between DCA purchases.
    pub new_interval: u64,
    /// a [`Uint128`] amount of `new_initial_asset` to spend each DCA purchase.
    pub new_dca_amount: Uint128,
    /// An optional parameter that determines if the order's next purchase should be set to
    /// `new_first_purchase`.
    pub new_first_purchase: Option<u64>,
}

/// ## Description
/// Modifies an existing DCA order for a user such that the new parameters will apply to the
/// existing order.
///
/// If the user increases the size of their order, they must allocate the correct amount of new
/// assets to the contract.
///
/// If the user decreases the size of their order, they will be refunded with the difference.
///
/// Returns a [`ContractError`] as a failure, otherwise returns a [`Response`] with the specified
/// attributes if the operation was successful.
/// ## Arguments
/// * `deps` - A [`DepsMut`] that contains the dependencies.
///
/// * `env` - The [`Env`] of the blockchain.
///
/// * `info` - A [`MessageInfo`] from the sender who wants to modify their order, containing the
/// [`AssetInfo::NativeToken`] if the DCA order is being increased in size.
///
/// * `order_details` - The [`ModifyDcaOrderParameters`] details about the old and new DCA order
/// parameters.
pub fn modify_dca_order(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    order_details: ModifyDcaOrderParameters,
) -> Result<Response, ContractError> {
    let ModifyDcaOrderParameters {
        id,
        new_initial_asset,
        new_target_asset,
        new_interval,
        new_dca_amount,
        new_first_purchase,
    } = order_details;

    let mut orders = USER_DCA
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();

    // check that old_initial_asset.info exists
    let order = orders
        .iter_mut()
        .find(|order| order.id == id)
        .ok_or(ContractError::NonexistentDca {})?;

    let should_refund = order.initial_asset.amount > new_initial_asset.amount;
    let asset_difference = Asset {
        info: new_initial_asset.info.clone(),
        amount: match should_refund {
            true => order
                .initial_asset
                .amount
                .checked_sub(new_initial_asset.amount)?,
            false => new_initial_asset
                .amount
                .checked_sub(order.initial_asset.amount)?,
        },
    };

    let mut messages = Vec::new();

    if order.initial_asset.info == new_initial_asset.info {
        if !should_refund {
            // if the user needs to have deposited more, check that we have the correct funds/allowance sent
            // this is the case only when the old_initial_asset and new_initial_asset are the same

            // if native token, they should have included it in the message
            // otherwise, if cw20 token, they should have provided the correct allowance
            match &order.initial_asset.info {
                AssetInfo::NativeToken { .. } => {
                    asset_difference.assert_sent_native_token_balance(&info)?
                }
                AssetInfo::Token { contract_addr } => {
                    let allowance =
                        get_token_allowance(&deps.as_ref(), &env, &info.sender, contract_addr)?;
                    if allowance != new_initial_asset.amount {
                        return Err(ContractError::InvalidTokenDeposit {});
                    }
                }
            }
        } else {
            // we need to refund the user with the difference if it is a native token
            if let AssetInfo::NativeToken { denom } = &new_initial_asset.info {
                messages.push(BankMsg::Send {
                    to_address: info.sender.to_string(),
                    amount: coins(asset_difference.amount.u128(), denom),
                })
            }
        }
    } else {
        // they are different assets, so we will return the old_initial_asset if it is a native token
        if let AssetInfo::NativeToken { denom } = &new_initial_asset.info {
            messages.push(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: coins(order.initial_asset.amount.u128(), denom),
            })
        }

        // validate that user sent either native tokens or has set allowance for the new token
        match &new_initial_asset.info {
            AssetInfo::NativeToken { .. } => {
                new_initial_asset.assert_sent_native_token_balance(&info)?
            }
            AssetInfo::Token { contract_addr } => {
                let allowance =
                    get_token_allowance(&deps.as_ref(), &env, &info.sender, contract_addr)?;
                if allowance != new_initial_asset.amount {
                    return Err(ContractError::InvalidTokenDeposit {});
                }
            }
        }
    }

    // update order
    order.initial_asset = new_initial_asset.clone();
    order.target_asset = new_target_asset.clone();
    order.interval = new_interval;
    order.dca_amount = new_dca_amount;

    if let Some(new_first_purchase) = new_first_purchase {
        order.last_purchase = new_first_purchase;
    }

    USER_DCA.save(deps.storage, &info.sender, &orders)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "modify_dca_order"),
        attr("id", id.to_string()),
        attr("new_initial_asset", new_initial_asset.to_string()),
        attr("new_target_asset", new_target_asset.to_string()),
        attr("new_interval", new_interval.to_string()),
        attr("new_dca_amount", new_dca_amount),
        attr(
            "new_first_purchase",
            match new_first_purchase {
                Some(t) => t.to_string(),
                None => "none".to_string(),
            },
        ),
    ]))
}
