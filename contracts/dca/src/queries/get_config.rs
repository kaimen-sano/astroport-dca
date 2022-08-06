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

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use astroport::asset::{Asset, AssetInfo};
    use astroport_dca::dca::QueryMsg;
    use cosmwasm_std::{
        from_binary,
        testing::{mock_dependencies, mock_env},
        Addr, Decimal, Uint128,
    };

    use crate::{
        contract::query,
        state::{Config, CONFIG},
    };

    #[test]
    fn does_get_config() {
        let mut deps = mock_dependencies();

        let saved_config = Config {
            factory_addr: Addr::unchecked("factory"),
            max_hops: 4,
            max_spread: Decimal::from_str("0.05").unwrap(),
            router_addr: Addr::unchecked("router"),
            whitelisted_fee_assets: vec![Asset {
                amount: Uint128::new(20_000),
                info: astroport::asset::AssetInfo::NativeToken {
                    denom: "ujpy".to_string(),
                },
            }],
            whitelisted_tokens: vec![AssetInfo::NativeToken {
                denom: "ukrw".to_string(),
            }],
        };

        CONFIG.save(&mut deps.storage, &saved_config).unwrap();

        let res: Config =
            from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap();
        assert_eq!(res, saved_config);
    }
}
