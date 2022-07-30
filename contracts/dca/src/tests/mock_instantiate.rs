use astroport::asset::Asset;
use astroport_dca::dca::InstantiateMsg;
use cosmwasm_std::{
    testing::{mock_dependencies, mock_env, MockApi, MockQuerier, MockStorage},
    Addr, Env, OwnedDeps,
};
use cw_multi_test::{App, Executor};

use crate::contract::instantiate;

use super::mock_creator;

/// Instantiates the dca module.
pub fn mock_instantiate(
    factory_addr: Addr,
    router_addr: Addr,
    whitelisted_fee_assets: Vec<Asset>,
) -> (OwnedDeps<MockStorage, MockApi, MockQuerier>, Env) {
    let mut deps = mock_dependencies();
    let env = mock_env();

    let creator = mock_creator();

    instantiate(
        deps.as_mut(),
        env.clone(),
        creator,
        InstantiateMsg {
            factory_addr: factory_addr.into_string(),
            router_addr: router_addr.into_string(),
            max_hops: 4,
            max_spread: "0.05".to_string(),
            whitelisted_fee_assets,
            whitelisted_tokens: vec![],
        },
    )
    .unwrap();

    (deps, env)
}

pub fn app_mock_instantiate(
    app: &mut App,
    dca_module_id: u64,
    factory_addr: Addr,
    router_addr: Addr,
    whitelisted_fee_assets: Vec<Asset>,
) -> Addr {
    let creator = mock_creator();

    app.instantiate_contract(
        dca_module_id,
        creator.sender,
        &InstantiateMsg {
            factory_addr: factory_addr.into_string(),
            router_addr: router_addr.into_string(),
            max_hops: 4,
            max_spread: "0.05".to_string(),
            whitelisted_fee_assets,
            whitelisted_tokens: vec![],
        },
        &[],
        "dca_module",
        None,
    )
    .unwrap()
}
