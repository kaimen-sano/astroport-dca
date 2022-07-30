use cw_multi_test::{App, ContractWrapper};

use crate::contract::{execute, instantiate, migrate, query};

/// Stores the DCA module contract to the app
pub fn store_dca_module_code(app: &mut App) -> u64 {
    let contract =
        Box::new(ContractWrapper::new(execute, instantiate, query).with_migrate(migrate));

    app.store_code(contract)
}

/// Stores the base cw20 contract too the app
pub fn store_cw20_token_code(app: &mut App) -> u64 {
    let contract = Box::new(ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    ));

    app.store_code(contract)
}
