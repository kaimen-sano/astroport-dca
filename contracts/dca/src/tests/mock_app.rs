use cosmwasm_std::{Addr, Coin};
use cw_multi_test::{App, AppBuilder};

/// Creates a mock application for integration tests
pub fn mock_app() -> App {
    App::default()
}

pub fn mock_app_with_balance(balances: Vec<(Addr, Vec<Coin>)>) -> App {
    AppBuilder::new().build(|router, _, storage| {
        balances.into_iter().for_each(|(account, coins)| {
            router.bank.init_balance(storage, &account, coins).unwrap();
        });
    })
}
