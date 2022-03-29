use astroport::{asset::AssetInfo, querier::query_factory_config};
use cosmwasm_std::{attr, Decimal, DepsMut, MessageInfo, Response, StdError, Uint128};

use crate::{error::ContractError, state::CONFIG};

/// ## Description
/// Updates the contract configuration with the specified optional parameters.
/// Returns a [`ContractError`] as a failure, otherwise returns a [`Response`] with the specified
/// attributes if the operation was successful
/// ## Params
/// * **deps** is an object of type [`DepsMut`].
///
/// * **info** is an object of type [`MessageInfo`].
///
/// * **max_hops** is an optional [`u8`] which represents the new maximum amount of hops per swap. If
/// excluded, the current config value will not be changed.
///
/// * **per_hop_fee** is an optional [`Uint128`] which represents the new uusd fee paid to bots per hop
/// executed in a dca swap purchase. If excluded, the current config value will not be changed.
///
/// * **whitelisted_tokens** is an optional [`Vec<AssetInfo>`] which represents the new whitelisted tokens that
/// can be used in a swap for dca purchases. If excluded, the current config value will not be
/// changed.
///
/// * **max_spread** is an optional [`Decimal`] which represents the new maximum spread for each DCA
/// purchase. If excluded, the current config value will not be changed.
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
