use astroport::asset::{addr_validate_to_lower, AssetInfo};
use astroport_dca::dca::DcaQueryInfo;
use cosmwasm_std::{Deps, Env, StdResult};

use crate::{get_token_allowance::get_token_allowance, state::USER_DCA};

/// ## Description
/// Returns a users DCA orders currently set.
///
/// The result is returned in a [`Vec<DcaQueryInfo`] object of the users current DCA orders with the
/// `amount` of each order set to the native token amount that can be spent, or the token allowance.
///
/// ## Arguments
/// * `deps` - A [`Deps`] that contains the dependencies.
///
/// * `env` - The [`Env`] of the blockchain.
///
/// * `user` - The users lowercase address as a [`String`].
pub fn get_user_dca_orders(deps: Deps, env: Env, user: String) -> StdResult<Vec<DcaQueryInfo>> {
    let user_address = addr_validate_to_lower(deps.api, &user)?;

    USER_DCA
        .load(deps.storage, &user_address)?
        .into_iter()
        .map(|order| {
            Ok(DcaQueryInfo {
                order: order.clone(),
                token_allowance: match &order.initial_asset.info {
                    AssetInfo::NativeToken { .. } => order.initial_asset.amount,
                    AssetInfo::Token { contract_addr } => {
                        // since it is a cw20 token, we need to retrieve the current allowance for the dca contract
                        get_token_allowance(&deps, &env, &user_address, contract_addr)?
                    }
                },
            })
        })
        .collect::<StdResult<Vec<_>>>()
}

#[cfg(test)]
mod test {
    use astroport::asset::{Asset, AssetInfo};
    use astroport_dca::dca::{DcaInfo, DcaQueryInfo, ExecuteMsg, QueryMsg};
    use cosmwasm_std::{coins, Addr, Uint128};
    use cw20::Cw20Coin;
    use cw_multi_test::{App, Executor};

    use crate::tests::{
        app_mock_instantiate, mock_app_with_balance, mock_creator, store_cw20_token_code,
        store_dca_module_code,
    };

    #[test]
    fn does_get_user_orders() {
        let mut app = mock_app_with_balance(vec![(mock_creator().sender, coins(100_000, "uluna"))]);

        let dca_module_id = store_dca_module_code(&mut app);
        let cw20_token_id = store_cw20_token_code(&mut app);

        let dca_addr = app_mock_instantiate(
            &mut app,
            dca_module_id,
            Addr::unchecked("factory"),
            Addr::unchecked("router"),
            vec![],
        );

        let cw20_addr = app
            .instantiate_contract(
                cw20_token_id,
                Addr::unchecked("admin"),
                &cw20_base::msg::InstantiateMsg {
                    decimals: 6,
                    initial_balances: vec![Cw20Coin {
                        address: mock_creator().sender.into_string(),
                        amount: Uint128::new(100_000),
                    }],
                    marketing: None,
                    mint: None,
                    name: "cw20 mock token".to_string(),
                    symbol: "cwT".to_string(),
                },
                &[],
                "cw20 mock token",
                None,
            )
            .unwrap();

        // add two orders
        let add_order = |app: &mut App, asset: Asset| {
            app.execute_contract(
                mock_creator().sender,
                dca_addr.clone(),
                &ExecuteMsg::CreateDcaOrder {
                    initial_asset: asset.clone(),
                    target_asset: astroport::asset::AssetInfo::NativeToken {
                        denom: "ukrw".to_string(),
                    },
                    interval: 1_000,
                    dca_amount: Uint128::new(10_000),
                    first_purchase: None,
                },
                &match asset.info {
                    AssetInfo::NativeToken { denom } => coins(20_000, denom),
                    _ => vec![],
                },
            )
            .unwrap();
        };

        add_order(
            &mut app,
            Asset {
                amount: Uint128::new(20_000),
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
            },
        );

        // increase allowance on the cw20 token
        app.execute_contract(
            mock_creator().sender,
            cw20_addr.clone(),
            &cw20::Cw20ExecuteMsg::IncreaseAllowance {
                spender: dca_addr.clone().to_string(),
                amount: Uint128::new(20_000),
                expires: None,
            },
            &[],
        )
        .unwrap();

        add_order(
            &mut app,
            Asset {
                amount: Uint128::new(20_000),
                info: AssetInfo::Token {
                    contract_addr: cw20_addr.clone(),
                },
            },
        );

        // now decrease the allowance to 10k
        app.execute_contract(
            mock_creator().sender,
            cw20_addr.clone(),
            &cw20::Cw20ExecuteMsg::DecreaseAllowance {
                spender: dca_addr.clone().into_string(),
                amount: Uint128::new(10_000),
                expires: None,
            },
            &[],
        )
        .unwrap();

        let res: Vec<DcaQueryInfo> = app
            .wrap()
            .query_wasm_smart(
                dca_addr,
                &QueryMsg::UserDcaOrders {
                    user: mock_creator().sender.into_string(),
                },
            )
            .unwrap();
        assert_eq!(
            res,
            vec![
                DcaQueryInfo {
                    order: DcaInfo {
                        id: 1,
                        dca_amount: Uint128::new(10_000),
                        initial_asset: Asset {
                            info: AssetInfo::NativeToken {
                                denom: "uluna".to_string()
                            },
                            amount: Uint128::new(20_000)
                        },
                        interval: 1_000,
                        last_purchase: 0,
                        target_asset: AssetInfo::NativeToken {
                            denom: "ukrw".to_string()
                        }
                    },
                    token_allowance: Uint128::new(20_000)
                },
                DcaQueryInfo {
                    order: DcaInfo {
                        id: 2,
                        initial_asset: Asset {
                            amount: Uint128::new(20_000),
                            info: AssetInfo::Token {
                                contract_addr: cw20_addr
                            }
                        },
                        target_asset: AssetInfo::NativeToken {
                            denom: "ukrw".to_string()
                        },
                        interval: 1_000,
                        last_purchase: 0,
                        dca_amount: Uint128::new(10_000)
                    },
                    token_allowance: Uint128::new(10_000)
                }
            ]
        );
    }
}
