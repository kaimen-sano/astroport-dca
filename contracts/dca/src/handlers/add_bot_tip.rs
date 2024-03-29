use astroport::asset::{Asset, AssetInfo};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response};

use crate::{
    error::ContractError,
    get_token_allowance::get_token_allowance,
    state::{CONFIG, USER_CONFIG},
};

/// ## Description
/// Adds a tip to the contract for a users DCA purchases.
///
/// Returns a [`ContractError`] as a failure, otherwise returns a [`Response`] with the specified
/// attributes if the operation was successful.
/// ## Arguments
/// * `deps` - A [`DepsMut`] that contains the dependencies.
///
/// * `env` - The [`Env`] of the blockchain.
///
/// * `info` - A [`MessageInfo`] which contains any native funds to a users tip balance.
///
/// * `assets` - A [`Vec<Asset>`] which contains the assets added in the tip message.
pub fn add_bot_tip(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    assets: Vec<Asset>,
) -> Result<Response, ContractError> {
    let mut user_config = USER_CONFIG
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();

    let config = CONFIG.load(deps.storage)?;

    // check that all assets are whitelisted
    let invalid_asset = assets
        .iter()
        .find(|a| !config.is_whitelisted_fee_asset(&a.info));
    if let Some(a) = invalid_asset {
        return Err(ContractError::NonWhitelistedTipAsset {
            asset: a.info.clone(),
        });
    }

    for asset in assets {
        // validate user sent what they said they did
        match &asset.info {
            AssetInfo::NativeToken { denom } => {
                let sent_funds = info.funds.iter().find(|f| &f.denom == denom).ok_or(
                    ContractError::TipDepositMissingAsset {
                        asset: asset.clone(),
                    },
                )?;

                if sent_funds.amount != asset.amount {
                    return Err(ContractError::InvalidTipDeposit {
                        received: Asset {
                            amount: sent_funds.amount,
                            info: asset.info.clone(),
                        },
                        sent: asset,
                    });
                }
            }
            AssetInfo::Token { contract_addr } => {
                let allowance =
                    get_token_allowance(&deps.as_ref(), &env, &info.sender, contract_addr)?;
                if allowance != asset.amount {
                    return Err(ContractError::InvalidTipDeposit {
                        received: Asset {
                            amount: allowance,
                            info: asset.info.clone(),
                        },
                        sent: asset,
                    });
                }
            }
        }

        // update user tip in state
        let balance = user_config
            .tip_balance
            .iter_mut()
            .find(|a| a.info == asset.info);

        // increment balance
        match balance {
            Some(balance) => {
                balance.amount = balance.amount.checked_add(asset.amount)?;
            }
            None => user_config.tip_balance.push(asset),
        }
    }

    // save new config
    USER_CONFIG.save(deps.storage, &info.sender, &user_config)?;

    Ok(Response::new().add_attributes(vec![attr("action", "add_bot_tip")]))
}

#[cfg(test)]
mod tests {
    use astroport::asset::{Asset, AssetInfo};
    use astroport_dca::dca::ExecuteMsg;
    use cosmwasm_std::{attr, coin, testing::mock_info, Addr, Response, Uint128};
    use cw_multi_test::Executor;

    use crate::{
        contract::execute,
        error::ContractError,
        state::{UserConfig, USER_CONFIG},
        tests::{
            app_mock_instantiate, mock_app, mock_creator, mock_instantiate, read_map,
            store_cw20_token_code, store_dca_module_code,
        },
    };

    #[test]
    fn does_add_bot_tip_native() {
        let (mut deps, env) = mock_instantiate(
            Addr::unchecked("factory"),
            Addr::unchecked("router"),
            vec![Asset {
                amount: Uint128::new(15_000),
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
            }],
            vec![],
        );

        let tip_sent = coin(10000, "uluna");

        let info = mock_info("creator", &[tip_sent.clone()]);
        let msg = ExecuteMsg::AddBotTip {
            assets: vec![Asset {
                amount: tip_sent.amount,
                info: AssetInfo::NativeToken {
                    denom: tip_sent.denom.clone(),
                },
            }],
        };

        // check that we got the expected response
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(
            res,
            Response::new().add_attributes(vec![attr("action", "add_bot_tip"),])
        );

        // check that user tip balance was added
        let config = USER_CONFIG
            .load(&deps.storage, &Addr::unchecked("creator"))
            .unwrap();
        assert_eq!(
            config,
            UserConfig {
                tip_balance: vec![Asset {
                    amount: tip_sent.amount,
                    info: AssetInfo::NativeToken {
                        denom: tip_sent.denom
                    }
                }],
                ..UserConfig::default()
            }
        )
    }

    #[test]
    fn does_add_bot_tip_token() {
        // instantiate contracts
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

        let tip_asset = Asset {
            amount: Uint128::new(5_000),
            info: AssetInfo::Token {
                contract_addr: cw20_addr.clone(),
            },
        };

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
            cw20_addr,
            &cw20_base::msg::ExecuteMsg::IncreaseAllowance {
                spender: dca_addr.clone().into_string(),
                amount: tip_asset.amount,
                expires: None,
            },
            &[],
        )
        .unwrap();

        // add tip
        let msg = ExecuteMsg::AddBotTip {
            assets: vec![tip_asset.clone()],
        };

        app.execute_contract(mock_creator().sender, dca_addr.clone(), &msg, &[])
            .unwrap();

        let config = read_map(&app, dca_addr, &mock_creator().sender, USER_CONFIG);
        assert_eq!(config.tip_balance, vec![tip_asset])
    }

    #[test]
    fn does_require_funds_native() {
        let (mut deps, env) = mock_instantiate(
            Addr::unchecked("factory"),
            Addr::unchecked("router"),
            vec![Asset {
                amount: Uint128::new(15_000),
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
            }],
            vec![],
        );

        let tip_asset = Asset {
            amount: Uint128::new(5_000),
            info: AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        };

        let msg = ExecuteMsg::AddBotTip {
            assets: vec![tip_asset.clone()],
        };

        // should error with TipDepositMissingAsset failure
        let res = execute(deps.as_mut(), env, mock_creator(), msg).unwrap_err();
        assert_eq!(
            res,
            ContractError::TipDepositMissingAsset { asset: tip_asset }
        );
    }

    #[test]
    fn does_require_whitelisted_funds_native() {
        let (mut deps, env) = mock_instantiate(
            Addr::unchecked("factory"),
            Addr::unchecked("router"),
            vec![Asset {
                amount: Uint128::new(15_000),
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
            }],
            vec![],
        );

        let tip_denom = "ukrw".to_string();
        let tip_asset = Asset {
            amount: Uint128::new(20_000),
            info: AssetInfo::NativeToken {
                denom: tip_denom.clone(),
            },
        };

        let info = mock_info("creator", &[coin(tip_asset.amount.u128(), tip_denom)]);
        let msg = ExecuteMsg::AddBotTip {
            assets: vec![tip_asset.clone()],
        };

        // should error with NonWhitelistedTipAsset
        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(
            res,
            ContractError::NonWhitelistedTipAsset {
                asset: tip_asset.info
            }
        );
    }

    #[test]
    fn does_require_whitelisted_funds_token() {
        let tip_asset = Asset {
            amount: Uint128::new(20_000),
            info: AssetInfo::Token {
                contract_addr: Addr::unchecked("cw20_token"),
            },
        };

        // instantiate contracts
        let mut app = mock_app();

        let dca_module_id = store_dca_module_code(&mut app);

        let dca_addr = app_mock_instantiate(
            &mut app,
            dca_module_id,
            Addr::unchecked("factory"),
            Addr::unchecked("router"),
            vec![],
        );

        let msg = ExecuteMsg::AddBotTip {
            assets: vec![tip_asset.clone()],
        };

        // should error with NonWhitelistedTipAsset
        let res = app
            .execute_contract(mock_creator().sender, dca_addr, &msg, &[])
            .unwrap_err();
        assert_eq!(
            res.downcast::<ContractError>().unwrap(),
            ContractError::NonWhitelistedTipAsset {
                asset: tip_asset.info
            }
        );
    }

    #[test]
    fn does_error_on_sent_mismatch_native() {
        let (mut deps, env) = mock_instantiate(
            Addr::unchecked("factory"),
            Addr::unchecked("router"),
            vec![Asset {
                amount: Uint128::new(15_000),
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
            }],
            vec![],
        );

        let tip_asset = Asset {
            amount: Uint128::new(25_000),
            info: AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        };

        let info = mock_info("creator", &[coin(20_000, "uluna")]);
        let msg = ExecuteMsg::AddBotTip {
            assets: vec![tip_asset.clone()],
        };

        // should error with InvalidTipDeposit
        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(
            res,
            ContractError::InvalidTipDeposit {
                received: Asset {
                    info: AssetInfo::NativeToken {
                        denom: "uluna".to_string()
                    },
                    amount: Uint128::new(20_000)
                },
                sent: tip_asset
            }
        );
    }

    #[test]
    fn does_error_on_sent_mismatch_token() {
        // instantiate contracts
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

        // increase allowance by 20,000
        app.execute_contract(
            mock_creator().sender,
            cw20_addr.clone(),
            &cw20_base::msg::ExecuteMsg::IncreaseAllowance {
                spender: dca_addr.clone().into_string(),
                amount: Uint128::new(20_000),
                expires: None,
            },
            &[],
        )
        .unwrap();

        let tip_asset = Asset {
            amount: Uint128::new(25_000),
            info: AssetInfo::Token {
                contract_addr: cw20_addr.clone(),
            },
        };

        let msg = ExecuteMsg::AddBotTip {
            assets: vec![tip_asset.clone()],
        };

        // should error with InvalidTipDeposit
        let res = app
            .execute_contract(mock_creator().sender, dca_addr, &msg, &[])
            .unwrap_err();
        assert_eq!(
            res.downcast::<ContractError>().unwrap(),
            ContractError::InvalidTipDeposit {
                received: Asset {
                    info: AssetInfo::Token {
                        contract_addr: cw20_addr
                    },
                    amount: Uint128::new(20_000)
                },
                sent: tip_asset
            }
        );
    }

    #[test]
    fn does_increment_tip_balance() {
        let (mut deps, env) = mock_instantiate(
            Addr::unchecked("factory"),
            Addr::unchecked("router"),
            vec![Asset {
                amount: Uint128::new(15_000),
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
            }],
            vec![],
        );

        let tip_asset = Asset {
            amount: Uint128::new(25_000),
            info: AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        };

        let info = mock_info(
            "creator",
            &[coin(tip_asset.amount.u128(), "uluna".to_string())],
        );
        let msg = ExecuteMsg::AddBotTip {
            assets: vec![tip_asset.clone()],
        };

        execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
        execute(deps.as_mut(), env, info.clone(), msg).unwrap();

        // should have incremented balance
        let user_config = USER_CONFIG.load(&deps.storage, &info.sender).unwrap();
        assert_eq!(
            user_config.tip_balance,
            vec![Asset {
                amount: tip_asset.amount * Uint128::new(2),
                info: tip_asset.info
            }]
        );
    }
}
