use astroport::{
    asset::{Asset, AssetInfo},
    querier::query_factory_config,
};
use cosmwasm_std::{attr, Decimal, DepsMut, MessageInfo, Response, StdError};

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
/// * `per_hop_fee` - An optional [`Vec<Asset>`] which represents the new fee paid to bots per hop
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
    whitelisted_fee_assets: Option<Vec<Asset>>,
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

        if let Some(new_whitelisted_fee_assets) = whitelisted_fee_assets {
            config.whitelisted_fee_assets = new_whitelisted_fee_assets;
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

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use astroport::asset::{Asset, AssetInfo};
    use astroport_dca::dca::ExecuteMsg;
    use cosmwasm_std::{Addr, Decimal, Uint128};
    use cw_multi_test::{App, Executor};

    use crate::{
        error::ContractError,
        state::{Config, CONFIG},
        tests::{
            app_mock_instantiate, mock_app, mock_creator, read_item, store_dca_module_code,
            store_factory_code,
        },
    };

    fn instantiate() -> (App, Addr) {
        let mut app = mock_app();

        let dca_module_id = store_dca_module_code(&mut app);
        let factory_id = store_factory_code(&mut app);

        let factory_addr = app
            .instantiate_contract(
                factory_id,
                mock_creator().sender,
                &astroport::factory::InstantiateMsg {
                    owner: "factory_owner".to_string(),
                    token_code_id: 99,
                    whitelist_code_id: 100,
                    pair_configs: vec![],
                    fee_address: None,
                    generator_address: None,
                },
                &[],
                "factory",
                None,
            )
            .unwrap();

        let dca_addr = app_mock_instantiate(
            &mut app,
            dca_module_id,
            factory_addr,
            Addr::unchecked("router"),
            vec![],
        );

        (app, dca_addr)
    }

    #[test]
    fn does_update() {
        let (mut app, dca_addr) = instantiate();

        let config = read_item(&app, &dca_addr, CONFIG);

        let mut new_fee_assets = vec![Asset {
            amount: Uint128::new(25_000),
            info: AssetInfo::Token {
                contract_addr: Addr::unchecked("bad_token"),
            },
        }];
        new_fee_assets.extend_from_slice(&config.whitelisted_fee_assets);

        let mut new_tokens = vec![AssetInfo::Token {
            contract_addr: Addr::unchecked("bad_token"),
        }];
        new_tokens.extend_from_slice(&config.whitelisted_tokens);

        let new_config = Config {
            max_hops: config.max_hops + 1,
            max_spread: Decimal::from_str("0.1").unwrap() + config.max_spread,
            factory_addr: Addr::unchecked("contract0"),
            router_addr: Addr::unchecked("router"),
            whitelisted_fee_assets: new_fee_assets,
            whitelisted_tokens: new_tokens,
        };

        app.execute_contract(
            Addr::unchecked("factory_owner"),
            dca_addr.clone(),
            &ExecuteMsg::UpdateConfig {
                max_hops: Some(new_config.max_hops),
                whitelisted_tokens: Some(new_config.whitelisted_tokens.clone()),
                whitelisted_fee_assets: Some(new_config.whitelisted_fee_assets.clone()),
                max_spread: Some(new_config.max_spread),
            },
            &[],
        )
        .unwrap();

        let config = read_item(&app, &dca_addr, CONFIG);
        assert_eq!(config, new_config);
    }

    #[test]
    fn does_allow_empty_update() {
        let (mut app, dca_addr) = instantiate();

        let config = read_item(&app, &dca_addr, CONFIG);

        app.execute_contract(
            Addr::unchecked("factory_owner"),
            dca_addr.clone(),
            &ExecuteMsg::UpdateConfig {
                max_hops: None,
                whitelisted_tokens: None,
                whitelisted_fee_assets: None,
                max_spread: None,
            },
            &[],
        )
        .unwrap();

        let new_config = read_item(&app, &dca_addr, CONFIG);

        assert_eq!(config, new_config);
    }

    #[test]
    fn does_check_if_authorized() {
        let (mut app, dca_addr) = instantiate();

        let res = app
            .execute_contract(
                mock_creator().sender,
                dca_addr,
                &ExecuteMsg::UpdateConfig {
                    max_hops: Some(1),
                    whitelisted_tokens: Some(vec![AssetInfo::NativeToken {
                        denom: "ugbp".to_string(),
                    }]),
                    whitelisted_fee_assets: Some(vec![Asset {
                        amount: Uint128::new(5_000),
                        info: AssetInfo::Token {
                            contract_addr: Addr::unchecked("bad_addr"),
                        },
                    }]),
                    max_spread: Some(Decimal::from_str("0.075").unwrap()),
                },
                &[],
            )
            .unwrap_err();

        assert_eq!(
            res.downcast::<ContractError>().unwrap(),
            ContractError::Unauthorized {}
        );
    }
}
