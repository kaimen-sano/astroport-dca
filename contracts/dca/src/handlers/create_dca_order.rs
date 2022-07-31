use astroport::asset::{Asset, AssetInfo};
use astroport_dca::dca::DcaInfo;
use cosmwasm_std::{
    attr, DepsMut, Env, MessageInfo, OverflowError, OverflowOperation, Response, StdError, Uint128,
};

use crate::{
    error::ContractError,
    get_token_allowance::get_token_allowance,
    state::{USER_CONFIG, USER_DCA},
};

pub struct CreateDcaOrder {
    pub initial_asset: Asset,
    pub target_asset: AssetInfo,
    pub interval: u64,
    pub dca_amount: Uint128,
    pub first_purchase: Option<u64>,
}

/// ## Description
/// Creates a new DCA order for a user where the `target_asset` will be purchased with `dca_amount`
/// of token `initial_asset` every `interval`.
///
/// Returns a [`ContractError`] as a failure, otherwise returns a [`Response`] with the specified
/// attributes if the operation was successful.
/// ## Arguments
/// * `deps` - A [`DepsMut`] that contains the dependencies.
///
/// * `env` - The [`Env`] of the blockchain.
///
/// * `info` - A [`MessageInfo`] from the sender who wants to create their order, containing the
/// [`AssetInfo::NativeToken`] if the `initial_asset` is a native token.
///
/// * `initial_asset` - The [`Asset`] that is being spent to purchase DCA orders. If the asset is a
/// Token (non-native), the contact will need to have the allowance for the DCA contract set to the
/// `initial_asset.amount`.
///
/// * `target_asset` - The [`AssetInfo`] that is being purchased with `initial_asset`.
///
/// * `interval` - The time in seconds between DCA purchases.
///
/// * `dca_amount` - A [`Uint128`] representing the amount of `initial_asset` to spend each DCA
/// purchase.
///
/// * `first_purchase` - A [`Option<u64>`] representing the first time the users DCA order should be
/// processed if specified, otherwise as soon as the order is made it can be processed.
pub fn create_dca_order(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    order_info: CreateDcaOrder,
) -> Result<Response, ContractError> {
    let CreateDcaOrder {
        initial_asset,
        target_asset,
        interval,
        dca_amount,
        first_purchase,
    } = order_info;

    // check that user has not previously created dca strategy with this initial_asset
    let mut orders = USER_DCA
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();

    // check that assets are not duplicate
    if initial_asset.info == target_asset {
        return Err(ContractError::DuplicateAsset {});
    }

    // check that dca_amount is less than initial_asset.amount
    if dca_amount > initial_asset.amount {
        return Err(ContractError::DepositTooSmall {});
    }

    // check that initial_asset.amount is divisible by dca_amount
    let remainder = initial_asset
        .amount
        .checked_rem(dca_amount)
        .map_err(|e| StdError::DivideByZero { source: e })?;
    if !remainder.is_zero() {
        return Err(ContractError::IndivisibleDeposit {});
    }

    // check that user has sent the valid tokens to the contract
    // if native token, they should have included it in the message
    // otherwise, if cw20 token, they should have provided the correct allowance
    match &initial_asset.info {
        AssetInfo::NativeToken { .. } => initial_asset.assert_sent_native_token_balance(&info)?,
        AssetInfo::Token { contract_addr } => {
            let allowance = get_token_allowance(&deps.as_ref(), &env, &info.sender, contract_addr)?;
            if allowance != initial_asset.amount {
                return Err(ContractError::InvalidTokenDeposit {});
            }
        }
    }

    let id = USER_CONFIG
        .update::<_, StdError>(deps.storage, &info.sender, |config| {
            let mut config = config.unwrap_or_default();

            config.last_id = config
                .last_id
                .checked_add(1)
                .ok_or_else(|| OverflowError::new(OverflowOperation::Add, config.last_id, 1))?;

            Ok(config)
        })?
        .last_id;

    // store dca order
    orders.push(DcaInfo {
        id,
        initial_asset: initial_asset.clone(),
        target_asset: target_asset.clone(),
        interval,
        last_purchase: first_purchase.unwrap_or_default(),
        dca_amount,
    });

    USER_DCA.save(deps.storage, &info.sender, &orders)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "create_dca_order"),
        attr("initial_asset", initial_asset.to_string()),
        attr("target_asset", target_asset.to_string()),
        attr("interval", interval.to_string()),
        attr("dca_amount", dca_amount),
    ]))
}

#[cfg(test)]
mod tests {
    use astroport::asset::{Asset, AssetInfo};
    use astroport_dca::dca::{DcaInfo, ExecuteMsg};
    use cosmwasm_std::{
        attr, coins,
        testing::{mock_dependencies, mock_env, mock_info},
        Addr, DivideByZeroError, Response, StdError, Uint128,
    };
    use cw_multi_test::Executor;

    use crate::{
        contract::execute,
        error::ContractError,
        state::USER_DCA,
        tests::{
            app_mock_instantiate, mock_app, mock_creator, store_cw20_token_code,
            store_dca_module_code,
        },
    };

    #[test]
    fn does_create_native() {
        let mut deps = mock_dependencies();

        let initial_asset = Asset {
            amount: Uint128::new(100_000),
            info: AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        };
        let target_asset = AssetInfo::NativeToken {
            denom: "ukrw".to_string(),
        };

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("creator", &coins(initial_asset.amount.u128(), "uluna")),
            ExecuteMsg::CreateDcaOrder {
                initial_asset: initial_asset.clone(),
                target_asset: target_asset.clone(),
                interval: 1_000,
                dca_amount: Uint128::new(25_000),
                first_purchase: None,
            },
        )
        .unwrap();

        assert_eq!(
            res,
            Response::new().add_attributes(vec![
                attr("action", "create_dca_order"),
                attr("initial_asset", initial_asset.to_string()),
                attr("target_asset", target_asset.to_string()),
                attr("interval", "1000"),
                attr("dca_amount", "25000"),
            ])
        );

        // check that it got added to state
        let orders = USER_DCA
            .load(&deps.storage, &mock_creator().sender)
            .unwrap();

        assert_eq!(
            orders,
            vec![DcaInfo {
                id: 1,
                dca_amount: Uint128::new(25_000),
                initial_asset,
                target_asset,
                interval: 1_000,
                last_purchase: 0
            }]
        );
    }

    #[test]
    fn does_create_token() {
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

        let initial_asset = Asset {
            amount: Uint128::new(100_000),
            info: AssetInfo::Token {
                contract_addr: cw20_addr.clone(),
            },
        };
        let target_asset = AssetInfo::NativeToken {
            denom: "uluna".to_string(),
        };

        // increment allowance
        app.execute_contract(
            mock_creator().sender,
            cw20_addr,
            &cw20_base::msg::ExecuteMsg::IncreaseAllowance {
                spender: dca_addr.clone().into_string(),
                amount: initial_asset.amount,
                expires: None,
            },
            &[],
        )
        .unwrap();

        app.execute_contract(
            mock_creator().sender,
            dca_addr,
            &ExecuteMsg::CreateDcaOrder {
                initial_asset,
                target_asset,
                interval: 1000,
                dca_amount: Uint128::new(25_000),
                first_purchase: None,
            },
            &[],
        )
        .unwrap();
    }

    #[test]
    fn cannot_create_duplicate_asset() {
        let mut deps = mock_dependencies();

        let asset = Asset {
            amount: Uint128::new(25_000),
            info: AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        };

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_creator(),
            ExecuteMsg::CreateDcaOrder {
                initial_asset: asset.clone(),
                target_asset: asset.info,
                interval: 1_000,
                dca_amount: Uint128::new(5_000),
                first_purchase: None,
            },
        )
        .unwrap_err();

        assert_eq!(res, ContractError::DuplicateAsset {});
    }

    #[test]
    fn cannot_create_greater_dca_order() {
        let mut deps = mock_dependencies();

        let initial_asset = Asset {
            amount: Uint128::new(100_000),
            info: astroport::asset::AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        };
        let target_asset = AssetInfo::NativeToken {
            denom: "ukrw".to_string(),
        };

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("creator", &coins(initial_asset.amount.u128(), "uluna")),
            ExecuteMsg::CreateDcaOrder {
                initial_asset: initial_asset.clone(),
                target_asset,
                interval: 1_000,
                dca_amount: initial_asset.amount * Uint128::new(2),
                first_purchase: None,
            },
        )
        .unwrap_err();

        assert_eq!(res, ContractError::DepositTooSmall {});
    }

    #[test]
    fn cannot_create_indivisible_order() {
        let mut deps = mock_dependencies();

        let initial_asset = Asset {
            amount: Uint128::new(100_000),
            info: astroport::asset::AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        };
        let target_asset = AssetInfo::NativeToken {
            denom: "ukrw".to_string(),
        };

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("creator", &coins(initial_asset.amount.u128(), "uluna")),
            ExecuteMsg::CreateDcaOrder {
                initial_asset: initial_asset.clone(),
                target_asset: target_asset.clone(),
                interval: 1_000,
                dca_amount: Uint128::new(999),
                first_purchase: None,
            },
        )
        .unwrap_err();
        assert_eq!(res, ContractError::IndivisibleDeposit {});

        // does not panic when using size of zero to create order
        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("creator", &coins(initial_asset.amount.u128(), "uluna")),
            ExecuteMsg::CreateDcaOrder {
                initial_asset,
                target_asset,
                interval: 1_000,
                dca_amount: Uint128::new(0),
                first_purchase: None,
            },
        )
        .unwrap_err();
        assert_eq!(
            res,
            ContractError::Std(StdError::DivideByZero {
                source: DivideByZeroError::new("100000")
            })
        );
    }

    #[test]
    fn does_require_native_sent() {
        let mut deps = mock_dependencies();

        let initial_asset = Asset {
            amount: Uint128::new(100_000),
            info: astroport::asset::AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        };
        let target_asset = AssetInfo::NativeToken {
            denom: "ukrw".to_string(),
        };

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_creator(),
            ExecuteMsg::CreateDcaOrder {
                initial_asset,
                target_asset,
                interval: 1_000,
                dca_amount: Uint128::new(25_000),
                first_purchase: None,
            },
        )
        .unwrap_err();

        assert_eq!(
            res,
            ContractError::Std(StdError::generic_err(
                "Native token balance mismatch between the argument and the transferred"
            ))
        );
    }

    #[test]
    fn does_require_token_allowance_set() {
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

        let initial_asset = Asset {
            amount: Uint128::new(100_000),
            info: AssetInfo::Token {
                contract_addr: cw20_addr,
            },
        };
        let target_asset = AssetInfo::NativeToken {
            denom: "uluna".to_string(),
        };

        let res = app
            .execute_contract(
                mock_creator().sender,
                dca_addr,
                &ExecuteMsg::CreateDcaOrder {
                    initial_asset,
                    target_asset,
                    interval: 1000,
                    dca_amount: Uint128::new(25_000),
                    first_purchase: None,
                },
                &[],
            )
            .unwrap_err();

        assert_eq!(
            res.downcast::<ContractError>().unwrap(),
            ContractError::InvalidTokenDeposit {}
        );
    }

    #[test]
    fn does_increment_id() {
        let mut deps = mock_dependencies();

        let initial_asset = Asset {
            amount: Uint128::new(100_000),
            info: AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        };
        let target_asset = AssetInfo::NativeToken {
            denom: "ukrw".to_string(),
        };

        let mut create_order = || {
            execute(
                deps.as_mut(),
                mock_env(),
                mock_info("creator", &coins(initial_asset.amount.u128(), "uluna")),
                ExecuteMsg::CreateDcaOrder {
                    initial_asset: initial_asset.clone(),
                    target_asset: target_asset.clone(),
                    interval: 1_000,
                    dca_amount: Uint128::new(25_000),
                    first_purchase: None,
                },
            )
            .unwrap();
        };
        create_order();
        create_order();

        // check that it got added to state
        let orders = USER_DCA
            .load(&deps.storage, &mock_creator().sender)
            .unwrap();

        assert_eq!(
            orders,
            vec![
                DcaInfo {
                    id: 1,
                    dca_amount: Uint128::new(25_000),
                    initial_asset: initial_asset.clone(),
                    target_asset: target_asset.clone(),
                    interval: 1_000,
                    last_purchase: 0
                },
                DcaInfo {
                    id: 2,
                    dca_amount: Uint128::new(25_000),
                    initial_asset,
                    target_asset,
                    interval: 1_000,
                    last_purchase: 0
                }
            ]
        );
    }
}
