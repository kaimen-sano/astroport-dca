use astroport::{
    asset::{Asset, AssetInfo},
    dca::DcaInfo,
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response, StdResult, Uint128};

use crate::{error::ContractError, get_token_allowance::get_token_allowance, state::USER_DCA};

/// ## Description
/// Creates a new DCA order for a user where the `target_asset` will be purchased with `dca_amount`
/// of token `initial_asset` every `interval`
///
/// Returns a [`ContractError`] as a failure, otherwise returns a [`Response`] with the specified
/// attributes if the operation was successful
/// ## Params
/// * **deps** is an object of type [`DepsMut`].
///
/// * **env** is an object of type [`Env`].
///
/// * **info** is an object of type [`MessageInfo`] which contains any native tokens needed to
/// create the DCA order.
///
/// * **initial_asset** is the asset that is being spent to create DCA orders.
///
/// * **target_asset** is the asset that is being purchased with `initial_asset`.
///
/// * **interval** is the time in seconds between DCA purchases.
///
/// * **dca_amount** is the amount of `initial_asset` to spend each DCA purchase.
pub fn create_dca_order(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    initial_asset: Asset,
    target_asset: AssetInfo,
    interval: u64,
    dca_amount: Uint128,
) -> Result<Response, ContractError> {
    // check that user has not previously created dca strategy with this initial_asset
    if USER_DCA.has(deps.storage, &info.sender) {
        return Err(ContractError::AlreadyDeposited {});
    }

    // check that user has sent the valid tokens to the contract
    // if native token, they should have included it in the message
    // otherwise, if cw20 token, they should have provided the correct allowance
    match &initial_asset.info {
        AssetInfo::NativeToken { .. } => initial_asset.assert_sent_native_token_balance(&info)?,
        AssetInfo::Token { contract_addr } => {
            let allowance = get_token_allowance(&deps.as_ref(), &env, &info.sender, contract_addr)?;
            if allowance != initial_asset.amount {
                return Err(ContractError::InvalidTokenDeposit {});
            }
        }
    }

    // store dca order
    USER_DCA.update(
        deps.storage,
        &info.sender,
        |orders| -> StdResult<Vec<DcaInfo>> {
            let mut orders = orders.unwrap_or_default();

            orders.push(DcaInfo {
                initial_asset: initial_asset.clone(),
                target_asset: target_asset.clone(),
                interval,
                last_purchase: 0,
                dca_amount,
            });

            Ok(orders)
        },
    )?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "create_dca_order"),
        attr("initial_asset", initial_asset.to_string()),
        attr("target_asset", target_asset.to_string()),
        attr("interval", interval.to_string()),
        attr("dca_amount", dca_amount),
    ]))
}
