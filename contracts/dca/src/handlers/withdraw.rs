use astroport::asset::{Asset, AssetInfo};
use cosmwasm_std::{attr, coins, BankMsg, DepsMut, MessageInfo, Response};

use crate::{
    error::ContractError,
    state::{CONFIG, USER_CONFIG},
};

/// ## Description
/// Withdraws a users bot tip from the contract.
///
/// Returns a [`ContractError`] as a failure, otherwise returns a [`Response`] with the specified
/// attributes if the operation was successful.
/// ## Arguments
/// * `deps` - A [`DepsMut`] that contains the dependencies.
///
/// * `info` - A [`MessageInfo`] from the sender who wants to withdraw their bot tip.
///
/// * `assets`` - A [`Vec<Asset>`] representing the assets the user wants to withdraw from bot tip.
pub fn withdraw(
    deps: DepsMut,
    info: MessageInfo,
    assets: Vec<Asset>,
) -> Result<Response, ContractError> {
    let mut user_config = USER_CONFIG
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();

    let config = CONFIG.load(deps.storage)?;

    let mut send_msgs: Vec<BankMsg> = vec![];

    for asset in assets {
        if !config.is_whitelisted_fee_asset(&asset.info) {
            return Err(ContractError::NonWhitelistedTipAsset { asset: asset.info });
        }

        let user_balance = user_config
            .tip_balance
            .iter_mut()
            .find(|a| a.info == asset.info)
            .ok_or(ContractError::TipAssetNotDeposited {
                asset: asset.clone().info,
            })?;

        user_balance.amount = user_balance.amount.checked_sub(asset.amount)?;

        if let AssetInfo::NativeToken { denom } = asset.info {
            send_msgs.push(BankMsg::Send {
                to_address: info.clone().sender.into_string(),
                amount: coins(asset.amount.u128(), denom),
            });
        }
    }

    // optimization: if the user is withdrawing all their tips, they are probably never going to
    // interact with the contract again. In this case, we can delete their config to save space
    // otherwise, we save their new configuration
    match user_config.tip_balance.iter().all(|a| a.amount.is_zero()) {
        true => {
            USER_CONFIG.remove(deps.storage, &info.sender);
            Ok(())
        }
        false => USER_CONFIG.save(deps.storage, &info.sender, &user_config),
    }?;

    Ok(Response::new()
        .add_attributes(vec![attr("action", "withdraw")])
        .add_messages(send_msgs))
}

#[cfg(test)]
mod tests {
    use astroport::asset::{Asset, AssetInfo};
    use astroport_dca::dca::ExecuteMsg;
    use cosmwasm_std::{
        attr, coin, coins, testing::mock_info, Addr, BankMsg, DepsMut, Env, MessageInfo,
        OverflowError, OverflowOperation, Response, Uint128,
    };

    use crate::{
        contract::execute,
        error::ContractError,
        state::{UserConfig, USER_CONFIG},
        tests::{mock_creator, mock_instantiate},
    };

    fn add_tip(deps: DepsMut, env: Env, info: MessageInfo, asset: Asset) {
        execute(
            deps,
            env,
            info,
            ExecuteMsg::AddBotTip {
                assets: vec![asset],
            },
        )
        .unwrap();
    }

    #[test]
    fn will_withdraw_tip() {
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

        let tip_sent = coin(10_000, "uluna");

        let msg = ExecuteMsg::Withdraw {
            assets: vec![Asset {
                amount: Uint128::new(5_000),
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
            }],
        };

        add_tip(
            deps.as_mut(),
            env.clone(),
            mock_info("creator", &[tip_sent]),
            Asset {
                amount: Uint128::new(10_000),
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
            },
        );

        // attempt bot withdraw and check that we got the expected response
        let res = execute(deps.as_mut(), env, mock_creator(), msg).unwrap();
        assert_eq!(
            res,
            Response::new()
                .add_attributes(vec![attr("action", "withdraw"),])
                .add_message(BankMsg::Send {
                    to_address: "creator".to_string(),
                    amount: coins(5_000, "uluna")
                })
        )
    }

    #[test]
    fn does_update_config() {
        let (mut deps, env) = mock_instantiate(
            Addr::unchecked("factory"),
            Addr::unchecked("router"),
            vec![Asset {
                amount: Uint128::new(2_000),
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
            }],
            vec![],
        );

        let tip_sent = coin(10_000, "uluna");

        let msg = ExecuteMsg::Withdraw {
            assets: vec![Asset {
                amount: Uint128::new(5_000),
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
            }],
        };

        // add bot tip
        add_tip(
            deps.as_mut(),
            env.clone(),
            mock_info("creator", &[tip_sent.clone()]),
            Asset {
                info: AssetInfo::NativeToken {
                    denom: tip_sent.denom,
                },
                amount: tip_sent.amount,
            },
        );

        // attempt bot withdraw and check that config was updated
        execute(deps.as_mut(), env, mock_creator(), msg).unwrap();
        let config = USER_CONFIG
            .load(&deps.storage, &Addr::unchecked("creator"))
            .unwrap();
        assert_eq!(
            config,
            UserConfig {
                tip_balance: vec![Asset {
                    amount: tip_sent.amount - Uint128::new(5_000),
                    info: AssetInfo::NativeToken {
                        denom: "uluna".to_string()
                    }
                }],
                ..UserConfig::default()
            }
        )
    }

    #[test]
    fn wont_excess_withdraw() {
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

        let tip_sent = coin(10_000, "uluna");
        let tip_withdraw = coin(15_000, "uluna");

        let msg = ExecuteMsg::Withdraw {
            assets: vec![Asset {
                amount: tip_withdraw.amount,
                info: AssetInfo::NativeToken {
                    denom: tip_withdraw.denom,
                },
            }],
        };

        // add bot tip
        add_tip(
            deps.as_mut(),
            env.clone(),
            mock_info("creator", &[tip_sent.clone()]),
            Asset {
                amount: tip_sent.amount,
                info: AssetInfo::NativeToken {
                    denom: tip_sent.denom,
                },
            },
        );

        // attempt bot withdraw and check that it failed
        let err = execute(deps.as_mut(), env, mock_creator(), msg).unwrap_err();
        assert_eq!(
            err,
            ContractError::OverflowError(OverflowError::new(
                OverflowOperation::Sub,
                tip_sent.amount,
                tip_withdraw.amount
            ))
        );
    }
}
