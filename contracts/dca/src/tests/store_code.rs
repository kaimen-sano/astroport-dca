use cw_multi_test::{App, ContractWrapper};

use crate::contract::{execute, instantiate, migrate, query};

/// Stores the DCA module contract to the app
pub fn store_dca_module_code(app: &mut App) -> u64 {
    let contract =
        Box::new(ContractWrapper::new(execute, instantiate, query).with_migrate(migrate));

    app.store_code(contract)
}

/// Stores the base cw20 contract to the app
pub fn store_cw20_token_code(app: &mut App) -> u64 {
    let contract = Box::new(ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    ));

    app.store_code(contract)
}

/// Stores the Astroport factory to the app
pub fn store_factory_code(app: &mut App) -> u64 {
    let contract = Box::new(
        ContractWrapper::new(
            astroport_factory::contract::execute,
            astroport_factory::contract::instantiate,
            astroport_factory::contract::query,
        )
        .with_reply(astroport_factory::contract::reply),
    );

    app.store_code(contract)
}

/// Stores the Astroport router to the app
pub fn store_router_code(app: &mut App) -> u64 {
    let contract = Box::new(ContractWrapper::new(
        astroport_router::contract::execute,
        astroport_router::contract::instantiate,
        astroport_router::contract::query,
    ));

    app.store_code(contract)
}

/// Stores the Astroport pair to the app
pub fn store_astroport_pair_code(app: &mut App) -> u64 {
    let contract = Box::new(
        ContractWrapper::new(
            astroport_pair::contract::execute,
            astroport_pair::contract::instantiate,
            astroport_pair::contract::query,
        )
        .with_reply(astroport_pair::contract::reply),
    );

    app.store_code(contract)
}

/// Stores the Astroport token to the app
pub fn store_astroport_token_code(app: &mut App) -> u64 {
    let contract = Box::new(ContractWrapper::new(
        astroport_token::contract::execute,
        astroport_token::contract::instantiate,
        astroport_token::contract::query,
    ));

    app.store_code(contract)
}
