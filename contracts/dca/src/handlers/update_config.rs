use astroport::{asset::AssetInfo, querier::query_factory_config};
use cosmwasm_std::{attr, Decimal, DepsMut, MessageInfo, Response, StdError, Uint128};

use crate::{error::ContractError, state::CONFIG};

/// ## Description
/// Updates the contract configuration with the specified optional parameters.
///
/// If any new configuration value is excluded, the current configuration value will remain
/// unchanged.
///
/// Returns a [`ContractError`] as a failure, otherwise returns a [`Response`] with the specified
/// attributes if the operation was successful.
/// ## Arguments
/// * `deps` - A [`DepsMut`] that contains the dependencies.
///
/// * `info` - A [`MessageInfo`] from the factory contract owner who wants to modify the
/// configuration of the contract.
///
/// * `max_hops` - An optional value which represents the new maximum amount of hops per swap if the
/// user does not specify a value.
///
/// * `per_hop_fee` - An optional [`Uint128`] which represents the new uusd fee paid to bots per hop
/// executed in a DCA purchase.
///
/// * `whitelisted_tokens` - An optional [`Vec<AssetInfo>`] which represents the new whitelisted
/// tokens that can be used in a hop route for DCA purchases.
///
/// * `max_spread` - An optional [`Decimal`] which represents the new maximum spread for each DCA
/// purchase if the user does not specify a value.
pub fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    max_hops: Option<u32>,
    per_hop_fee: Option<Uint128>,
    whitelisted_tokens: Option<Vec<AssetInfo>>,
    max_spread: Option<Decimal>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let factory_config = query_factory_config(&deps.querier, config.factory_addr)?;

    if info.sender != factory_config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // update config
    CONFIG.update::<_, StdError>(deps.storage, |mut config| {
        if let Some(new_max_hops) = max_hops {
            config.max_hops = new_max_hops;
        }

        if let Some(new_per_hop_fee) = per_hop_fee {
            config.per_hop_fee = new_per_hop_fee;
        }

        if let Some(new_whitelisted_tokens) = whitelisted_tokens {
            config.whitelisted_tokens = new_whitelisted_tokens;
        }

        if let Some(new_max_spread) = max_spread {
            config.max_spread = new_max_spread;
        }

        Ok(config)
    })?;

    Ok(Response::default().add_attributes(vec![attr("action", "update_config")]))
}
