use astroport::asset::{Asset, AssetInfo};
use cosmwasm_std::{attr, coins, BankMsg, DepsMut, Env, MessageInfo, Response, Uint128};

use crate::{error::ContractError, get_token_allowance::get_token_allowance, state::USER_DCA};

/// Stores a modified dca order new parameters
pub struct ModifyDcaOrderParameters {
    /// The users [`u64`] ID of the order.
    pub id: u64,
    /// The new [`Asset`] that is being spent to create DCA orders.
    pub new_initial_asset: Asset,
    /// The [`AssetInfo`] that is being purchased with `new_initial_asset`.
    pub new_target_asset: AssetInfo,
    /// The time in seconds between DCA purchases.
    pub new_interval: u64,
    /// a [`Uint128`] amount of `new_initial_asset` to spend each DCA purchase.
    pub new_dca_amount: Uint128,
    /// An optional parameter that determines if the order's next purchase should be set to
    /// `new_first_purchase`.
    pub new_first_purchase: Option<u64>,
}

/// ## Description
/// Modifies an existing DCA order for a user such that the new parameters will apply to the
/// existing order.
///
/// If the user increases the size of their order, they must allocate the correct amount of new
/// assets to the contract.
///
/// If the user decreases the size of their order, they will be refunded with the difference.
///
/// Returns a [`ContractError`] as a failure, otherwise returns a [`Response`] with the specified
/// attributes if the operation was successful.
/// ## Arguments
/// * `deps` - A [`DepsMut`] that contains the dependencies.
///
/// * `env` - The [`Env`] of the blockchain.
///
/// * `info` - A [`MessageInfo`] from the sender who wants to modify their order, containing the
/// [`AssetInfo::NativeToken`] if the DCA order is being increased in size.
///
/// * `order_details` - The [`ModifyDcaOrderParameters`] details about the old and new DCA order
/// parameters.
pub fn modify_dca_order(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    order_details: ModifyDcaOrderParameters,
) -> Result<Response, ContractError> {
    let ModifyDcaOrderParameters {
        id,
        new_initial_asset,
        new_target_asset,
        new_interval,
        new_dca_amount,
        new_first_purchase,
    } = order_details;

    let mut orders = USER_DCA
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();

    // check that order with given id exists
    let order_position = orders
        .iter()
        .position(|order| order.id == id)
        .ok_or(ContractError::NonexistentDca {})?;

    let order = &orders[order_position];

    let should_refund = order.initial_asset.amount > new_initial_asset.amount;
    let asset_difference = Asset {
        info: new_initial_asset.info.clone(),
        amount: match should_refund {
            true => order
                .initial_asset
                .amount
                .checked_sub(new_initial_asset.amount)?,
            false => new_initial_asset
                .amount
                .checked_sub(order.initial_asset.amount)?,
        },
    };

    let mut messages = Vec::new();

    // check that user did not set new asset to the old asset target
    if new_initial_asset.info == new_target_asset {
        return Err(ContractError::DuplicateAsset {});
    }

    if order.initial_asset.info == new_initial_asset.info {
        if !should_refund {
            // if the user needs to have deposited more, check that we have the correct funds/allowance sent
            // this is the case only when the old_initial_asset and new_initial_asset are the same

            // if native token, they should have included it in the message
            // otherwise, if cw20 token, they should have provided the correct allowance
            match &order.initial_asset.info {
                AssetInfo::NativeToken { .. } => {
                    asset_difference.assert_sent_native_token_balance(&info)?
                }
                AssetInfo::Token { contract_addr } => {
                    // allowance should be greater than the sum of all orders with this initial asset
                    let total_allowance: Uint128 = orders
                        .iter()
                        .map(|o| match &o.initial_asset.info {
                            AssetInfo::Token {
                                contract_addr: o_contract_addr,
                            } if contract_addr == o_contract_addr => o.initial_asset.amount,
                            _ => Uint128::zero(),
                        })
                        .sum();

                    let allowance =
                        get_token_allowance(&deps.as_ref(), &env, &info.sender, contract_addr)?;
                    if total_allowance + asset_difference.amount > allowance {
                        return Err(ContractError::InvalidTokenDeposit {});
                    }
                }
            }
        } else {
            // we need to refund the user with the difference if it is a native token
            if let AssetInfo::NativeToken { denom } = &new_initial_asset.info {
                messages.push(BankMsg::Send {
                    to_address: info.sender.to_string(),
                    amount: coins(asset_difference.amount.u128(), denom),
                })
            }
        }
    } else {
        // they are different assets, so we will return the old_initial_asset if it is a native token
        if let AssetInfo::NativeToken { denom } = &order.initial_asset.info {
            messages.push(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: coins(order.initial_asset.amount.u128(), denom),
            })
        }

        // validate that user sent either native tokens or has set allowance for the new token
        match &new_initial_asset.info {
            AssetInfo::NativeToken { .. } => {
                new_initial_asset.assert_sent_native_token_balance(&info)?
            }
            AssetInfo::Token { contract_addr } => {
                // allowance should be greater than the sum of all orders with this initial asset
                let total_allowance: Uint128 = orders
                    .clone()
                    .iter()
                    .map(|o| match &o.initial_asset.info {
                        AssetInfo::Token {
                            contract_addr: o_contract_addr,
                        } if contract_addr == o_contract_addr => o.initial_asset.amount,
                        _ => Uint128::zero(),
                    })
                    .sum();

                let allowance =
                    get_token_allowance(&deps.as_ref(), &env, &info.sender, contract_addr)?;
                if total_allowance + new_initial_asset.amount > allowance {
                    return Err(ContractError::InvalidTokenDeposit {});
                }
            }
        }
    }

    // update order
    let mut order = &mut orders[order_position];

    order.initial_asset = new_initial_asset.clone();
    order.target_asset = new_target_asset.clone();
    order.interval = new_interval;
    order.dca_amount = new_dca_amount;

    if let Some(new_first_purchase) = new_first_purchase {
        order.last_purchase = new_first_purchase;
    }

    USER_DCA.save(deps.storage, &info.sender, &orders)?;

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "modify_dca_order"),
        attr("id", id.to_string()),
        attr("new_initial_asset", new_initial_asset.to_string()),
        attr("new_target_asset", new_target_asset.to_string()),
        attr("new_interval", new_interval.to_string()),
        attr("new_dca_amount", new_dca_amount),
        attr(
            "new_first_purchase",
            match new_first_purchase {
                Some(t) => t.to_string(),
                None => "none".to_string(),
            },
        ),
    ]))
}

#[cfg(test)]
mod test {
    use astroport::asset::{Asset, AssetInfo};
    use astroport_dca::dca::{DcaInfo, ExecuteMsg};
    use cosmwasm_std::{
        attr, coins,
        testing::{mock_dependencies, mock_env, mock_info},
        Addr, BankMsg, Response, StdError, Uint128,
    };
    use cw_multi_test::Executor;

    use crate::{
        contract::execute,
        error::ContractError,
        state::USER_DCA,
        tests::{
            app_mock_instantiate, mock_app, mock_app_with_balance, mock_creator,
            store_cw20_token_code, store_dca_module_code,
        },
    };

    #[test]
    fn does_modify_order() {
        let mut deps = mock_dependencies();

        let initial_asset = Asset {
            amount: Uint128::new(15_000),
            info: AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        };
        let target_asset = AssetInfo::NativeToken {
            denom: "ukrw".to_string(),
        };
        let new_target_asset = AssetInfo::NativeToken {
            denom: "ujpy".to_string(),
        };

        // create order
        execute(
            deps.as_mut(),
            mock_env(),
            mock_info("creator", &coins(initial_asset.amount.u128(), "uluna")),
            ExecuteMsg::CreateDcaOrder {
                initial_asset: initial_asset.clone(),
                target_asset,
                interval: 5_000,
                dca_amount: Uint128::new(1_000),
                first_purchase: None,
            },
        )
        .unwrap();

        // change target asset, interval, dca amount, first purchase
        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_creator(),
            ExecuteMsg::ModifyDcaOrder {
                id: 1,
                new_initial_asset: initial_asset.clone(),
                new_target_asset: new_target_asset.clone(),
                new_interval: 1_000,
                new_dca_amount: Uint128::new(500),
                new_first_purchase: Some(18_000),
            },
        )
        .unwrap();

        assert_eq!(
            res,
            Response::new().add_attributes(vec![
                attr("action", "modify_dca_order"),
                attr("id", "1"),
                attr("new_initial_asset", initial_asset.to_string()),
                attr("new_target_asset", new_target_asset.to_string()),
                attr("new_interval", "1000"),
                attr("new_dca_amount", "500"),
                attr("new_first_purchase", "18000"),
            ])
        );

        // check state
        let orders = USER_DCA
            .load(&deps.storage, &mock_creator().sender)
            .unwrap();
        assert_eq!(
            orders,
            vec![DcaInfo {
                id: 1,
                dca_amount: Uint128::new(500),
                initial_asset,
                interval: 1_000,
                last_purchase: 18_000,
                target_asset: new_target_asset
            }]
        );
    }

    #[test]
    fn does_refund_same_native() {
        // checks that the contract will refund the user if the order is modified where the new
        // initial asset is the same type as the old one, with a smaller amount
        let mut deps = mock_dependencies();

        let initial_asset = Asset {
            amount: Uint128::new(15_000),
            info: AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        };
        let target_asset = AssetInfo::NativeToken {
            denom: "ukrw".to_string(),
        };
        let new_initial_asset = Asset {
            info: initial_asset.info.clone(),
            amount: initial_asset.amount / Uint128::new(2),
        };
        let new_target_asset = AssetInfo::NativeToken {
            denom: "ujpy".to_string(),
        };

        // create order
        execute(
            deps.as_mut(),
            mock_env(),
            mock_info("creator", &coins(initial_asset.amount.u128(), "uluna")),
            ExecuteMsg::CreateDcaOrder {
                initial_asset: initial_asset.clone(),
                target_asset,
                interval: 5_000,
                dca_amount: Uint128::new(1_000),
                first_purchase: None,
            },
        )
        .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_creator(),
            ExecuteMsg::ModifyDcaOrder {
                id: 1,
                new_initial_asset: new_initial_asset.clone(),
                new_target_asset: new_target_asset.clone(),
                new_interval: 5_000,
                new_dca_amount: Uint128::new(1_000),
                new_first_purchase: None,
            },
        )
        .unwrap();

        assert_eq!(
            res,
            Response::new()
                .add_attributes(vec![
                    attr("action", "modify_dca_order"),
                    attr("id", "1"),
                    attr("new_initial_asset", new_initial_asset.to_string()),
                    attr("new_target_asset", new_target_asset.to_string()),
                    attr("new_interval", "5000"),
                    attr("new_dca_amount", "1000"),
                    attr("new_first_purchase", "none"),
                ])
                .add_message(BankMsg::Send {
                    amount: coins(
                        (initial_asset.amount - new_initial_asset.amount).u128(),
                        "uluna".to_string()
                    ),
                    to_address: mock_creator().sender.into_string()
                })
        );
    }

    #[test]
    fn does_validate_extra_sent_native() {
        // validates that when a user increases the initial_asset.amount, that they have attached
        // the required funds to their tx
        let mut deps = mock_dependencies();

        let initial_asset = Asset {
            amount: Uint128::new(15_000),
            info: AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        };
        let target_asset = AssetInfo::NativeToken {
            denom: "ukrw".to_string(),
        };
        let new_initial_asset = Asset {
            info: initial_asset.info.clone(),
            amount: initial_asset.amount * Uint128::new(2),
        };
        let new_target_asset = AssetInfo::NativeToken {
            denom: "ujpy".to_string(),
        };

        // create order
        execute(
            deps.as_mut(),
            mock_env(),
            mock_info("creator", &coins(initial_asset.amount.u128(), "uluna")),
            ExecuteMsg::CreateDcaOrder {
                initial_asset: initial_asset.clone(),
                target_asset,
                interval: 5_000,
                dca_amount: Uint128::new(1_000),
                first_purchase: None,
            },
        )
        .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_creator(),
            ExecuteMsg::ModifyDcaOrder {
                id: 1,
                new_initial_asset: new_initial_asset.clone(),
                new_target_asset: new_target_asset.clone(),
                new_interval: 5_000,
                new_dca_amount: Uint128::new(1_000),
                new_first_purchase: None,
            },
        )
        .unwrap_err();
        assert_eq!(
            res,
            ContractError::Std(StdError::generic_err(
                "Native token balance mismatch between the argument and the transferred"
            ))
        );

        // this time add the extra funds to the tx
        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info(
                "creator",
                &coins(
                    (new_initial_asset.amount - initial_asset.amount).u128(),
                    "uluna".to_string(),
                ),
            ),
            ExecuteMsg::ModifyDcaOrder {
                id: 1,
                new_initial_asset: new_initial_asset.clone(),
                new_target_asset: new_target_asset.clone(),
                new_interval: 5_000,
                new_dca_amount: Uint128::new(1_000),
                new_first_purchase: None,
            },
        )
        .unwrap();
        assert_eq!(
            res,
            Response::new().add_attributes(vec![
                attr("action", "modify_dca_order"),
                attr("id", "1"),
                attr("new_initial_asset", new_initial_asset.to_string()),
                attr("new_target_asset", new_target_asset.to_string()),
                attr("new_interval", "5000"),
                attr("new_dca_amount", "1000"),
                attr("new_first_purchase", "none"),
            ])
        );
    }

    #[test]
    fn does_validate_extra_sent_token() {
        // validates that when a user increases the initial_asset.amount, that they have attached
        // the required funds to their tx
        let mut app = mock_app_with_balance(vec![(mock_creator().sender, coins(100_000, "uluna"))]);

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

        let initial_asset = Asset {
            amount: Uint128::new(100_000),
            info: AssetInfo::Token {
                contract_addr: cw20_addr.clone(),
            },
        };
        let target_asset = AssetInfo::NativeToken {
            denom: "uluna".to_string(),
        };
        let new_initial_asset = Asset {
            info: initial_asset.info.clone(),
            amount: initial_asset.amount * Uint128::new(2),
        };

        // increase allowance
        app.execute_contract(
            mock_creator().sender,
            cw20_addr.clone(),
            &cw20_base::msg::ExecuteMsg::IncreaseAllowance {
                spender: dca_addr.clone().into_string(),
                amount: initial_asset.amount,
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
                initial_asset: initial_asset.clone(),
                target_asset: target_asset.clone(),
                interval: 1000,
                dca_amount: Uint128::new(25_000),
                first_purchase: None,
            },
            &[],
        )
        .unwrap();

        // create another random order
        app.execute_contract(
            mock_creator().sender,
            dca_addr.clone(),
            &ExecuteMsg::CreateDcaOrder {
                initial_asset: Asset {
                    amount: Uint128::new(20_000),
                    info: AssetInfo::NativeToken {
                        denom: "uluna".to_string(),
                    },
                },
                target_asset: AssetInfo::NativeToken {
                    denom: "ukrw".to_string(),
                },
                interval: 1_000,
                dca_amount: Uint128::new(1_000),
                first_purchase: None,
            },
            &coins(20_000, "uluna"),
        )
        .unwrap();

        // should error when we have not increased allowance
        let res = app
            .execute_contract(
                mock_creator().sender,
                dca_addr.clone(),
                &ExecuteMsg::ModifyDcaOrder {
                    id: 1,
                    new_initial_asset: new_initial_asset.clone(),
                    new_target_asset: target_asset.clone(),
                    new_interval: 5_000,
                    new_dca_amount: Uint128::new(1_000),
                    new_first_purchase: None,
                },
                &[],
            )
            .unwrap_err();
        assert_eq!(
            res.downcast::<ContractError>().unwrap(),
            ContractError::InvalidTokenDeposit {}
        );

        // this time add the extra funds to the tx
        app.execute_contract(
            mock_creator().sender,
            cw20_addr,
            &cw20_base::msg::ExecuteMsg::IncreaseAllowance {
                spender: dca_addr.clone().into_string(),
                amount: new_initial_asset.amount - initial_asset.amount,
                expires: None,
            },
            &[],
        )
        .unwrap();

        app.execute_contract(
            mock_creator().sender,
            dca_addr,
            &ExecuteMsg::ModifyDcaOrder {
                id: 1,
                new_initial_asset,
                new_target_asset: target_asset,
                new_interval: 5_000,
                new_dca_amount: Uint128::new(1_000),
                new_first_purchase: None,
            },
            &[],
        )
        .unwrap();
    }

    #[test]
    fn can_change_initial_asset_native() {
        let mut deps = mock_dependencies();

        let initial_asset = Asset {
            amount: Uint128::new(15_000),
            info: AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        };
        let target_asset = AssetInfo::NativeToken {
            denom: "ukrw".to_string(),
        };
        let new_initial_asset = Asset {
            info: AssetInfo::NativeToken {
                denom: "ukrw".to_string(),
            },
            amount: initial_asset.amount * Uint128::new(2),
        };
        let new_target_asset = AssetInfo::NativeToken {
            denom: "uluna".to_string(),
        };

        // create order
        execute(
            deps.as_mut(),
            mock_env(),
            mock_info("creator", &coins(initial_asset.amount.u128(), "uluna")),
            ExecuteMsg::CreateDcaOrder {
                initial_asset: initial_asset.clone(),
                target_asset,
                interval: 5_000,
                dca_amount: Uint128::new(1_000),
                first_purchase: None,
            },
        )
        .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_creator(),
            ExecuteMsg::ModifyDcaOrder {
                id: 1,
                new_initial_asset: new_initial_asset.clone(),
                new_target_asset: new_target_asset.clone(),
                new_interval: 5_000,
                new_dca_amount: Uint128::new(1_000),
                new_first_purchase: None,
            },
        )
        .unwrap_err();
        assert_eq!(
            res,
            ContractError::Std(StdError::generic_err(
                "Native token balance mismatch between the argument and the transferred"
            ))
        );

        // this time add the extra funds to the tx
        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info(
                "creator",
                &coins(new_initial_asset.amount.u128(), "ukrw".to_string()),
            ),
            ExecuteMsg::ModifyDcaOrder {
                id: 1,
                new_initial_asset: new_initial_asset.clone(),
                new_target_asset: new_target_asset.clone(),
                new_interval: 5_000,
                new_dca_amount: Uint128::new(1_000),
                new_first_purchase: None,
            },
        )
        .unwrap();
        assert_eq!(
            res,
            Response::new()
                .add_message(BankMsg::Send {
                    amount: coins(initial_asset.amount.u128(), "uluna"),
                    to_address: mock_creator().sender.into_string()
                })
                .add_attributes(vec![
                    attr("action", "modify_dca_order"),
                    attr("id", "1"),
                    attr("new_initial_asset", new_initial_asset.to_string()),
                    attr("new_target_asset", new_target_asset.to_string()),
                    attr("new_interval", "5000"),
                    attr("new_dca_amount", "1000"),
                    attr("new_first_purchase", "none"),
                ])
        );
    }

    #[test]
    fn can_change_initial_asset_token() {
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

        let cw20_other_addr = app
            .instantiate_contract(
                cw20_token_id,
                mock_creator().sender,
                &cw20_base::msg::InstantiateMsg {
                    decimals: 6,
                    initial_balances: vec![],
                    marketing: None,
                    mint: None,
                    name: "cw20 token #2".to_string(),
                    symbol: "cwTwo".to_string(),
                },
                &[],
                "mock 2nd cw20 token",
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

        let initial_asset = Asset {
            amount: Uint128::new(100_000),
            info: AssetInfo::Token {
                contract_addr: cw20_other_addr.clone(),
            },
        };
        let target_asset = AssetInfo::NativeToken {
            denom: "uluna".to_string(),
        };
        let new_initial_asset = Asset {
            info: AssetInfo::Token {
                contract_addr: cw20_addr.clone(),
            },
            amount: initial_asset.amount,
        };

        // increase allowance to create order
        app.execute_contract(
            mock_creator().sender,
            cw20_other_addr,
            &cw20_base::msg::ExecuteMsg::IncreaseAllowance {
                spender: dca_addr.clone().into_string(),
                amount: initial_asset.amount,
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
                initial_asset,
                target_asset: target_asset.clone(),
                interval: 1000,
                dca_amount: Uint128::new(25_000),
                first_purchase: None,
            },
            &[],
        )
        .unwrap();

        // attempt to modify should error, as we have not increased allowance
        let res = app
            .execute_contract(
                mock_creator().sender,
                dca_addr.clone(),
                &ExecuteMsg::ModifyDcaOrder {
                    id: 1,
                    new_initial_asset: new_initial_asset.clone(),
                    new_target_asset: target_asset.clone(),
                    new_interval: 1_000,
                    new_dca_amount: Uint128::new(25_000),
                    new_first_purchase: None,
                },
                &[],
            )
            .unwrap_err();
        assert_eq!(
            res.downcast::<ContractError>().unwrap(),
            ContractError::InvalidTokenDeposit {}
        );

        // increase allowance
        app.execute_contract(
            mock_creator().sender,
            cw20_addr,
            &cw20_base::msg::ExecuteMsg::IncreaseAllowance {
                spender: dca_addr.clone().into_string(),
                amount: new_initial_asset.amount,
                expires: None,
            },
            &[],
        )
        .unwrap();

        // should succeed now
        app.execute_contract(
            mock_creator().sender,
            dca_addr,
            &ExecuteMsg::ModifyDcaOrder {
                id: 1,
                new_initial_asset,
                new_target_asset: target_asset,
                new_interval: 1_000,
                new_dca_amount: Uint128::new(25_000),
                new_first_purchase: None,
            },
            &[],
        )
        .unwrap();
    }

    #[test]
    fn does_error_on_invalid_id() {
        let mut deps = mock_dependencies();

        let initial_asset = Asset {
            amount: Uint128::new(15_000),
            info: AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        };
        let target_asset = AssetInfo::NativeToken {
            denom: "ukrw".to_string(),
        };

        // create order
        execute(
            deps.as_mut(),
            mock_env(),
            mock_info("creator", &coins(initial_asset.amount.u128(), "uluna")),
            ExecuteMsg::CreateDcaOrder {
                initial_asset: initial_asset.clone(),
                target_asset: target_asset.clone(),
                interval: 5_000,
                dca_amount: Uint128::new(1_000),
                first_purchase: None,
            },
        )
        .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_creator(),
            ExecuteMsg::ModifyDcaOrder {
                id: 2,
                new_initial_asset: initial_asset,
                new_target_asset: target_asset,
                new_interval: 1_000,
                new_dca_amount: Uint128::new(500),
                new_first_purchase: Some(18_000),
            },
        )
        .unwrap_err();
        assert_eq!(res, ContractError::NonexistentDca {});
    }

    #[test]
    fn cannot_change_to_duplicate_asset() {
        let mut deps = mock_dependencies();

        let initial_asset = Asset {
            amount: Uint128::new(15_000),
            info: AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        };
        let target_asset = AssetInfo::NativeToken {
            denom: "ukrw".to_string(),
        };
        let new_initial_asset = Asset {
            amount: Uint128::new(15_000),
            info: target_asset.clone(),
        };

        // create order
        execute(
            deps.as_mut(),
            mock_env(),
            mock_info("creator", &coins(initial_asset.amount.u128(), "uluna")),
            ExecuteMsg::CreateDcaOrder {
                initial_asset,
                target_asset: target_asset.clone(),
                interval: 5_000,
                dca_amount: Uint128::new(1_000),
                first_purchase: None,
            },
        )
        .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_creator(),
            ExecuteMsg::ModifyDcaOrder {
                id: 1,
                new_initial_asset,
                new_target_asset: target_asset,
                new_interval: 1_000,
                new_dca_amount: Uint128::new(500),
                new_first_purchase: Some(18_000),
            },
        )
        .unwrap_err();
        assert_eq!(res, ContractError::DuplicateAsset {});
    }
}
