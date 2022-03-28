use astroport::{
    asset::{addr_validate_to_lower, Asset, AssetInfo},
    dca::DcaInfo,
};
use cosmwasm_std::{Deps, Env, StdResult};

use crate::{get_token_allowance::get_token_allowance, state::USER_DCA};

/// ## Description
/// Returns a users DCA orders currently set.
///
/// The result is returned in a [`Vec<DcaInfo`] object of the users current DCA orders.
///
/// ## Params
/// * **deps** is an object of type [`Deps`].
///
/// * **env** is an object of type [`Env`].
///
/// * **user** is the users lowercase address of type [`String`].
pub fn get_user_dca_orders(deps: Deps, env: Env, user: String) -> StdResult<Vec<DcaInfo>> {
    let user_address = addr_validate_to_lower(deps.api, &user)?;

    USER_DCA
        .load(deps.storage, &user_address)?
        .into_iter()
        .map(|order| {
            Ok(DcaInfo {
                initial_asset: match &order.initial_asset.info {
                    AssetInfo::NativeToken { .. } => order.initial_asset,
                    AssetInfo::Token { contract_addr } => {
                        // since it is a cw20 token, we need to retrieve the current allowance for the dca contract
                        let allowance =
                            get_token_allowance(&deps, &env, &user_address, contract_addr)?;

                        Asset {
                            amount: allowance,
                            ..order.initial_asset
                        }
                    }
                },
                ..order
            })
        })
        .collect::<StdResult<Vec<_>>>()
}
