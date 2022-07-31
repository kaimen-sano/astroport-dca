use astroport::asset::AssetInfo;
use astroport_dca::dca::DcaInfo;
use cosmwasm_std::{attr, BankMsg, Coin, DepsMut, MessageInfo, Response};

use crate::{error::ContractError, state::USER_DCA};

/// ## Description
/// Cancels a users DCA purchase so that it will no longer be fulfilled.
///
/// Returns the `initial_asset` back to the user if it was a native token.
///
/// Returns a [`ContractError`] as a failure, otherwise returns a [`Response`] with the specified
/// attributes if the operation was successful.
/// ## Arguments
/// * `deps` - A [`DepsMut`] that contains the dependencies.
///
/// * `info` - A [`MessageInfo`] from the sender who wants to cancel their order.
///
/// * `id` The [`u64`] ID which the user wants to cancel the DCA order for.
pub fn cancel_dca_order(
    deps: DepsMut,
    info: MessageInfo,
    id: u64,
) -> Result<Response, ContractError> {
    let mut funds = Vec::new();

    // remove order from user dca's, and add any native token funds for `initial_asset` into the `funds`.
    USER_DCA.update(
        deps.storage,
        &info.sender,
        |orders| -> Result<Vec<DcaInfo>, ContractError> {
            let mut orders = orders.ok_or(ContractError::NonexistentDca {})?;

            let order_position = orders
                .iter()
                .position(|order| order.id == id)
                .ok_or(ContractError::NonexistentDca {})?;

            let removed_order = &orders[order_position];
            if let AssetInfo::NativeToken { denom } = &removed_order.initial_asset.info {
                funds.push(BankMsg::Send {
                    to_address: info.sender.to_string(),
                    amount: vec![Coin {
                        amount: removed_order.initial_asset.amount,
                        denom: denom.clone(),
                    }],
                })
            }

            orders.remove(order_position);

            Ok(orders)
        },
    )?;

    Ok(Response::new().add_messages(funds).add_attributes(vec![
        attr("action", "cancel_dca_order"),
        attr("id", id.to_string()),
    ]))
}

#[cfg(test)]
mod tests {
    use astroport::asset::{Asset, AssetInfo};
    use astroport_dca::dca::ExecuteMsg;
    use cosmwasm_std::{
        attr, coins,
        testing::{mock_dependencies, mock_env, mock_info},
        Addr, BankMsg, DepsMut, MessageInfo, Response, Uint128,
    };
    use cw_multi_test::Executor;

    use crate::{
        contract::execute,
        error::ContractError,
        state::USER_DCA,
        tests::{
            app_mock_instantiate, mock_app, mock_creator, read_map, store_cw20_token_code,
            store_dca_module_code,
        },
    };

    fn create_order(
        deps: DepsMut,
        info: MessageInfo,
        asset_info: AssetInfo,
        target_asset: AssetInfo,
        first_purchase: Option<u64>,
    ) {
        let asset = Asset {
            amount: Uint128::new(1_000_000),
            info: asset_info,
        };

        execute(
            deps,
            mock_env(),
            info,
            ExecuteMsg::CreateDcaOrder {
                initial_asset: asset,
                target_asset,
                first_purchase,
                interval: 60,
                dca_amount: Uint128::new(500_000),
            },
        )
        .unwrap();
    }

    #[test]
    fn does_cancel_order_native() {
        let mut deps = mock_dependencies();

        let token = AssetInfo::NativeToken {
            denom: "uluna".to_string(),
        };
        let target_asset = AssetInfo::NativeToken {
            denom: "ukrw".to_string(),
        };

        create_order(
            deps.as_mut(),
            mock_info("creator", &coins(1_000_000, "uluna")),
            token,
            target_asset,
            None,
        );

        // cancel dca order
        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_creator(),
            ExecuteMsg::CancelDcaOrder { id: 1 },
        )
        .unwrap();

        assert_eq!(
            res,
            Response::new()
                .add_attributes(vec![attr("action", "cancel_dca_order"), attr("id", "1")])
                .add_message(BankMsg::Send {
                    to_address: mock_creator().sender.into_string(),
                    amount: coins(1_000_000, "uluna")
                })
        );

        // check it was removed from storage
        let orders = USER_DCA
            .load(&deps.storage, &mock_creator().sender)
            .unwrap();
        assert!(orders.is_empty())
    }

    #[test]
    fn does_cancel_order_token() {
        let mut app = mock_app();

        let cw20_token_id = store_cw20_token_code(&mut app);
        let dca_module_id = store_dca_module_code(&mut app);

        let cw20_addr = app
            .instantiate_contract(
                cw20_token_id,
                mock_creator().sender,
                &cw20_base::msg::InstantiateMsg {
                    decimals: 6,
                    initial_balances: vec![],
                    marketing: None,
                    mint: None,
                    name: "cw20 token".to_string(),
                    symbol: "cwT".to_string(),
                },
                &[],
                "mock cw20 token",
                None,
            )
            .unwrap();

        let dca_addr = app_mock_instantiate(
            &mut app,
            dca_module_id,
            Addr::unchecked("factory"),
            Addr::unchecked("router"),
            vec![Asset {
                amount: Uint128::new(15_000),
                info: AssetInfo::Token {
                    contract_addr: cw20_addr.clone(),
                },
            }],
        );

        // increment allowance
        app.execute_contract(
            mock_creator().sender,
            cw20_addr.clone(),
            &cw20_base::msg::ExecuteMsg::IncreaseAllowance {
                spender: dca_addr.clone().into_string(),
                amount: Uint128::new(1_000_000),
                expires: None,
            },
            &[],
        )
        .unwrap();

        // create order
        app.execute_contract(
            mock_creator().sender,
            dca_addr.clone(),
            &ExecuteMsg::CreateDcaOrder {
                initial_asset: Asset {
                    amount: Uint128::new(1_000_000),
                    info: AssetInfo::Token {
                        contract_addr: cw20_addr,
                    },
                },
                target_asset: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
                interval: 500,
                dca_amount: Uint128::new(500_000),
                first_purchase: None,
            },
            &[],
        )
        .unwrap();

        // cancel order
        app.execute_contract(
            mock_creator().sender,
            dca_addr.clone(),
            &ExecuteMsg::CancelDcaOrder { id: 1 },
            &[],
        )
        .unwrap();

        // check it was removed from storage
        let orders = read_map(&app, dca_addr, &mock_creator().sender, USER_DCA);
        assert!(orders.is_empty());
    }

    #[test]
    fn does_error_on_invalid_id() {
        let mut deps = mock_dependencies();

        // errors if user has never made a order before
        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_creator(),
            ExecuteMsg::CancelDcaOrder { id: 2 },
        )
        .unwrap_err();
        assert_eq!(res, ContractError::NonexistentDca {});

        // errors if wrong id is passed
        create_order(
            deps.as_mut(),
            mock_info("creator", &coins(1_000_000, "uluna")),
            AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
            AssetInfo::NativeToken {
                denom: "ukrw".to_string(),
            },
            None,
        );

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_creator(),
            ExecuteMsg::CancelDcaOrder { id: 2 },
        )
        .unwrap_err();
        assert_eq!(res, ContractError::NonexistentDca {});
    }
}
