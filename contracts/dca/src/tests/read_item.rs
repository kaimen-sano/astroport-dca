use cosmwasm_std::Addr;
use cosmwasm_storage::ReadonlyPrefixedStorage;
use cw_multi_test::App;
use cw_storage_plus::Item;
use serde::{de::DeserializeOwned, Serialize};

const NAMESPACE_WASM: &[u8] = b"wasm";

pub fn read_item<T: Serialize + DeserializeOwned>(
    app: &App,
    contract_addr: &Addr,
    item: Item<'_, T>,
) -> T {
    app.read_module(|_, _, storage| {
        let mut name = b"contract_data/".to_vec();
        name.extend_from_slice(contract_addr.as_bytes());
        let storage = ReadonlyPrefixedStorage::multilevel(storage, &[NAMESPACE_WASM, &name]);

        item.load(&storage).unwrap()
    })
}
