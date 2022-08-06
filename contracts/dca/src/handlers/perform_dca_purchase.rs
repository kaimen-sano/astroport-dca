use astroport::{
    asset::{addr_validate_to_lower, Asset, AssetInfo},
    router::{ExecuteMsg as RouterExecuteMsg, SwapOperation},
};
use astroport_dca::dca::DcaInfo;
use cosmwasm_std::{
    attr, to_binary, BankMsg, Coin, CosmosMsg, DepsMut, Env, MessageInfo, Response, StdError,
    Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;

use crate::{
    error::ContractError,
    state::{CONFIG, USER_CONFIG, USER_DCA},
};

/// ## Description
/// Performs a DCA purchase on behalf of another user using the hop route specified.
///
/// Returns a [`ContractError`] as a failure, otherwise returns a [`Response`] with the specified
/// attributes if the operation was successful.
/// ## Params
/// * `deps` - A [`DepsMut`] that contains the dependencies.
///
/// * `env` - The [`Env`] of the blockchain.
///
/// * `info` - A [`MessageInfo`] from the bot who is performing a DCA purchase on behalf of another
/// user, who will be rewarded with a uusd tip.
///
/// * `user` - The address of the user as a [`String`] who is having a DCA purchase fulfilled.
///
/// * `id` - A [`u64`] representing the ID of the DCA order for the user
///
/// * `hops` - A [`Vec<SwapOperation>`] of the hop operations to complete in the swap to purchase
/// the target asset.
///
/// * `fee_redeem` - A [`Vec<Asset>`] of the fees redeemed by the sender for processing the DCA
/// order.
pub fn perform_dca_purchase(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    user: String,
    id: u64,
    hops: Vec<SwapOperation>,
    fee_redeem: Vec<Asset>,
) -> Result<Response, ContractError> {
    // validate user address
    let user_address = addr_validate_to_lower(deps.api, &user)?;

    // retrieve configs
    let mut user_config = USER_CONFIG
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();
    let contract_config = CONFIG.load(deps.storage)?;

    // validate hops is at least one
    if hops.is_empty() {
        return Err(ContractError::EmptyHopRoute {});
    }

    // validate hops does not exceed max_hops
    let hops_len = hops.len() as u32;
    if hops_len > user_config.max_hops.unwrap_or(contract_config.max_hops) {
        return Err(ContractError::MaxHopsAssertion { hops: hops_len });
    }

    // validate that all middle hops (last hop excluded) are whitelisted tokens for the ask_denom or ask_asset
    let middle_hops = &hops[..hops.len() - 1];
    for swap in middle_hops {
        match swap {
            SwapOperation::NativeSwap { ask_denom, .. } => {
                if !contract_config
                    .whitelisted_tokens
                    .iter()
                    .any(|token| match token {
                        AssetInfo::NativeToken { denom } => ask_denom == denom,
                        AssetInfo::Token { .. } => false,
                    })
                {
                    // not a whitelisted native token
                    return Err(ContractError::InvalidHopRoute {
                        token: ask_denom.to_string(),
                    });
                }
            }
            SwapOperation::AstroSwap { ask_asset_info, .. } => {
                if !contract_config.is_whitelisted_asset(ask_asset_info) {
                    return Err(ContractError::InvalidHopRoute {
                        token: ask_asset_info.to_string(),
                    });
                }
            }
        }
    }

    // validate that fee_redeem is a valid combination
    let requested_fee_hops: Uint128 = fee_redeem
        .iter()
        .map(|a| {
            let whitelisted_asset = contract_config
                .whitelisted_fee_assets
                .iter()
                .find(|w| a.info == w.info)
                .ok_or(ContractError::NonWhitelistedTipAsset {
                    asset: a.info.clone(),
                })?;

            // ensure that it is exactly divisible
            if !a
                .amount
                .checked_rem(whitelisted_asset.amount)
                .map_err(|e| StdError::DivideByZero { source: e })?
                .is_zero()
            {
                return Err(ContractError::IndivisibleDeposit {});
            }

            // we don't need to use `checked_div` here as we early exit above if
            // `whitelisted_asset.amount` is zero
            Ok(a.amount / whitelisted_asset.amount)
        })
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .sum();

    if requested_fee_hops > Uint128::new(u128::from(hops_len)) {
        return Err(ContractError::RedeemTipTooLarge {
            requested: requested_fee_hops,
            performed: Uint128::new(u128::from(hops_len)),
        });
    }

    // store messages to send in response
    let mut messages: Vec<CosmosMsg> = Vec::new();

    // validate purchaser has enough funds to pay the sender
    for fee_asset in fee_redeem {
        let mut user_balance = user_config
            .tip_balance
            .iter_mut()
            .find(|a| a.info == fee_asset.info)
            .ok_or(ContractError::InsufficientTipBalance {})?;

        // remove tip from purchaser
        let new_balance = user_balance
            .amount
            .checked_sub(fee_asset.amount)
            .map_err(|_| ContractError::InsufficientTipBalance {})?;

        user_balance.amount = new_balance;

        // add tip payment to messages
        let tip_payment_message = match fee_asset.info {
            AssetInfo::NativeToken { denom } => BankMsg::Send {
                to_address: info.clone().sender.to_string(),
                amount: vec![Coin {
                    amount: fee_asset.amount,
                    denom,
                }],
            }
            .into(),
            AssetInfo::Token { contract_addr } => WasmMsg::Execute {
                contract_addr: contract_addr.into_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: user_address.clone().into_string(),
                    recipient: info.sender.clone().into_string(),
                    amount: fee_asset.amount,
                })?,
                funds: vec![],
            }
            .into(),
        };

        messages.push(tip_payment_message);
    }

    // retrieve max_spread from user config, or default to contract set max_spread
    let max_spread = user_config.max_spread.unwrap_or(contract_config.max_spread);

    // load user dca orders and update the relevant one
    USER_DCA.update(
        deps.storage,
        &user_address,
        |orders| -> Result<Vec<DcaInfo>, ContractError> {
            let mut orders = orders.ok_or(ContractError::NonexistentDca {})?;

            let order_idx = orders
                .iter()
                .position(|order| order.id == id)
                .ok_or(ContractError::NonexistentDca {})?;

            let mut order = &mut orders[order_idx];

            // check that it has been long enough between dca purchases
            if order.last_purchase + order.interval > env.block.time.seconds() {
                return Err(ContractError::PurchaseTooEarly {});
            }

            // check that last hop is target asset
            let last_hop = hops.last().ok_or(ContractError::EmptyHopRoute {})?;
            if last_hop.get_target_asset_info() != order.target_asset {
                return Err(ContractError::TargetAssetAssertion {});
            }

            // subtract dca_amount from order and update last_purchase time
            order.initial_asset.amount = order
                .initial_asset
                .amount
                .checked_sub(order.dca_amount)
                .map_err(|_| ContractError::InsufficientBalance {})?;
            order.last_purchase = env.block.time.seconds();

            // add funds and router message to response
            if let AssetInfo::Token { contract_addr } = &order.initial_asset.info {
                // send a TransferFrom request to the token to the router
                messages.push(
                    WasmMsg::Execute {
                        contract_addr: contract_addr.to_string(),
                        funds: vec![],
                        msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                            owner: user_address.to_string(),
                            recipient: contract_config.router_addr.to_string(),
                            amount: order.dca_amount,
                        })?,
                    }
                    .into(),
                );
            }

            // if it is a native token, we need to send the funds
            let funds = match &order.initial_asset.info {
                AssetInfo::NativeToken { denom } => vec![Coin {
                    amount: order.dca_amount,
                    denom: denom.clone(),
                }],
                AssetInfo::Token { .. } => vec![],
            };

            // tell the router to perform swap operations
            messages.push(
                WasmMsg::Execute {
                    contract_addr: contract_config.router_addr.to_string(),
                    funds,
                    msg: to_binary(&RouterExecuteMsg::ExecuteSwapOperations {
                        operations: hops,
                        minimum_receive: None,
                        to: Some(user_address.clone().into_string()),
                        max_spread: Some(max_spread),
                    })?,
                }
                .into(),
            );

            // remove order if it was fulfilled
            if order.initial_asset.amount.is_zero() {
                orders.remove(order_idx);
            }

            Ok(orders)
        },
    )?;

    // save new config
    USER_CONFIG.save(deps.storage, &user_address, &user_config)?;

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "perform_dca_purchase"),
        attr("user", user_address),
        attr("id", id.to_string()),
    ]))
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use astroport::{
        asset::{Asset, AssetInfo},
        factory::{PairConfig, PairType},
        router::SwapOperation,
    };
    use astroport_dca::dca::{DcaInfo, ExecuteMsg, InstantiateMsg};
    use cosmwasm_std::{
        attr, coin, coins,
        testing::{mock_dependencies, mock_env, mock_info},
        to_binary, Addr, BankMsg, CosmosMsg, Decimal, DivideByZeroError, Response, StdError,
        Uint128, WasmMsg,
    };
    use cw20::{BalanceResponse, Cw20Coin};
    use cw_multi_test::{App, Executor};

    use crate::{
        contract::execute,
        error::ContractError,
        state::{Config, UserConfig, CONFIG, USER_CONFIG, USER_DCA},
        tests::{
            mock_app_with_balance, mock_creator, mock_instantiate, read_map,
            store_astroport_pair_code, store_astroport_token_code, store_cw20_token_code,
            store_dca_module_code, store_factory_code, store_router_code,
        },
    };

    fn instantiate(max_hops: Option<u32>) -> (App, Addr, Addr) {
        let admin = Addr::unchecked("admin");

        let mut app = mock_app_with_balance(vec![
            (mock_creator().sender, coins(500_000, "uluna")),
            (
                admin.clone(),
                vec![
                    coin(1_000_000, "uluna"),
                    coin(1_500_000, "ujpy"),
                    coin(1_000_000, "ukrw"),
                ],
            ),
        ]);

        let cw20_token_id = store_cw20_token_code(&mut app);
        let dca_module_id = store_dca_module_code(&mut app);
        let astroport_pair_id = store_astroport_pair_code(&mut app);
        let astroport_token_id = store_astroport_token_code(&mut app);
        let factory_id = store_factory_code(&mut app);
        let router_id = store_router_code(&mut app);

        // instantiate cw20 token
        let cw20_addr = app
            .instantiate_contract(
                cw20_token_id,
                mock_creator().sender,
                &cw20_base::msg::InstantiateMsg {
                    decimals: 6,
                    initial_balances: vec![
                        Cw20Coin {
                            address: admin.clone().into_string(),
                            amount: Uint128::new(1_000_000),
                        },
                        Cw20Coin {
                            address: mock_creator().sender.into_string(),
                            amount: Uint128::new(500_000),
                        },
                    ],
                    marketing: None,
                    mint: None,
                    name: "cw20 token".to_string(),
                    symbol: "cwT".to_string(),
                },
                &[],
                "cw20 mock contract",
                None,
            )
            .unwrap();

        // instantiate random whitelisted contract
        let cw20_whitelist_addr = app
            .instantiate_contract(
                cw20_token_id,
                mock_creator().sender,
                &cw20_base::msg::InstantiateMsg {
                    decimals: 6,
                    initial_balances: vec![],
                    marketing: None,
                    mint: None,
                    name: "cw20 whitelisted token".to_string(),
                    symbol: "cwWT".to_string(),
                },
                &[],
                "cw20 mock whitelisted contract",
                None,
            )
            .unwrap();

        // instantiate factory
        let factory_addr = app
            .instantiate_contract(
                factory_id,
                mock_creator().sender,
                &astroport::factory::InstantiateMsg {
                    fee_address: None,
                    generator_address: None,
                    owner: mock_creator().sender.into_string(),
                    pair_configs: vec![PairConfig {
                        pair_type: PairType::Xyk {},
                        is_disabled: false,
                        is_generator_disabled: true,
                        maker_fee_bps: 30,
                        total_fee_bps: 30,
                        code_id: astroport_pair_id,
                    }],
                    token_code_id: cw20_token_id,
                    whitelist_code_id: 100,
                },
                &[],
                "astroport factory",
                None,
            )
            .unwrap();

        // instantiate router
        let router_addr = app
            .instantiate_contract(
                router_id,
                mock_creator().sender,
                &astroport::router::InstantiateMsg {
                    astroport_factory: factory_addr.clone().into_string(),
                },
                &[],
                "astroport router",
                None,
            )
            .unwrap();

        // instantiate dca module
        let dca_addr = app
            .instantiate_contract(
                dca_module_id,
                mock_creator().sender,
                &InstantiateMsg {
                    factory_addr: factory_addr.clone().into_string(),
                    max_hops: max_hops.unwrap_or(4),
                    max_spread: "0.05".to_string(),
                    router_addr: router_addr.into_string(),
                    whitelisted_fee_assets: vec![
                        Asset {
                            amount: Uint128::new(15_000),
                            info: AssetInfo::NativeToken {
                                denom: "uluna".to_string(),
                            },
                        },
                        Asset {
                            amount: Uint128::new(15_000),
                            info: AssetInfo::Token {
                                contract_addr: cw20_addr.clone(),
                            },
                        },
                    ],
                    whitelisted_tokens: vec![
                        AssetInfo::Token {
                            contract_addr: cw20_whitelist_addr,
                        },
                        AssetInfo::NativeToken {
                            denom: "ukrw".to_string(),
                        },
                        AssetInfo::NativeToken {
                            denom: "ujpy".to_string(),
                        },
                    ],
                },
                &[],
                "dca module",
                None,
            )
            .unwrap();

        // create some pools
        let mut create_pool = |assets: [AssetInfo; 2]| {
            let pair_res = app
                .execute_contract(
                    mock_creator().sender,
                    factory_addr.clone(),
                    &astroport::factory::ExecuteMsg::CreatePair {
                        pair_type: PairType::Xyk {},
                        asset_infos: [assets[0].clone(), assets[1].clone()],
                        init_params: None,
                    },
                    &[],
                )
                .unwrap();

            let pair_addr = Addr::unchecked(pair_res.events[2].attributes[0].value.clone());

            // add some liquidity to the pools
            let mut funds = vec![];
            for asset in &assets {
                match asset {
                    AssetInfo::NativeToken { denom } => funds.push(coin(500_000, denom)),
                    AssetInfo::Token { contract_addr } => {
                        app.execute_contract(
                            admin.clone(),
                            contract_addr.clone(),
                            &cw20_base::msg::ExecuteMsg::IncreaseAllowance {
                                spender: pair_addr.clone().into_string(),
                                amount: Uint128::new(500_000),
                                expires: None,
                            },
                            &[],
                        )
                        .unwrap();
                    }
                }
            }

            app.execute_contract(
                admin.clone(),
                pair_addr,
                &astroport::pair::ExecuteMsg::ProvideLiquidity {
                    assets: [
                        Asset {
                            amount: Uint128::new(500_000),
                            info: assets[0].clone(),
                        },
                        Asset {
                            amount: Uint128::new(500_000),
                            info: assets[1].clone(),
                        },
                    ],
                    slippage_tolerance: None,
                    auto_stake: None,
                    receiver: None,
                },
                &funds,
            )
            .unwrap();
        };

        create_pool([
            AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
            AssetInfo::NativeToken {
                denom: "ujpy".to_string(),
            },
        ]);
        create_pool([
            AssetInfo::NativeToken {
                denom: "ujpy".to_string(),
            },
            AssetInfo::NativeToken {
                denom: "ukrw".to_string(),
            },
        ]);
        create_pool([
            AssetInfo::Token {
                contract_addr: cw20_addr.clone(),
            },
            AssetInfo::NativeToken {
                denom: "ujpy".to_string(),
            },
        ]);

        (app, dca_addr, cw20_addr)
    }

    const NORMAL_ORDER_INTERVAL: u64 = 500;

    fn create_normal_order(
        app: &mut App,
        dca_addr: Addr,
        initial_info: AssetInfo,
        target_info: AssetInfo,
    ) {
        let funds = &match initial_info.clone() {
            AssetInfo::NativeToken { denom } => coins(100_000, denom),
            AssetInfo::Token { .. } => vec![],
        };

        app.execute_contract(
            mock_creator().sender,
            dca_addr,
            &ExecuteMsg::CreateDcaOrder {
                initial_asset: Asset {
                    amount: Uint128::new(100_000),
                    info: initial_info,
                },
                target_asset: target_info,
                interval: NORMAL_ORDER_INTERVAL,
                dca_amount: Uint128::new(10_000),
                first_purchase: None,
            },
            funds,
        )
        .unwrap();
    }

    fn add_tip_balance(app: &mut App, dca_addr: Addr) {
        app.execute_contract(
            mock_creator().sender,
            dca_addr,
            &ExecuteMsg::AddBotTip {
                assets: vec![Asset {
                    amount: Uint128::new(150_000),
                    info: AssetInfo::NativeToken {
                        denom: "uluna".to_string(),
                    },
                }],
            },
            &coins(150_000, "uluna"),
        )
        .unwrap();
    }

    #[test]
    fn can_perform_native_purchase() {
        let (mut app, dca_addr, ..) = instantiate(None);

        create_normal_order(
            &mut app,
            dca_addr.clone(),
            AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
            AssetInfo::NativeToken {
                denom: "ukrw".to_string(),
            },
        );

        add_tip_balance(&mut app, dca_addr.clone());

        let bot_user = Addr::unchecked("bot_user");

        // perform purchase
        app.execute_contract(
            bot_user.clone(),
            dca_addr.clone(),
            &ExecuteMsg::PerformDcaPurchase {
                user: mock_creator().sender.into_string(),
                id: 1,
                hops: vec![
                    SwapOperation::AstroSwap {
                        offer_asset_info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                        ask_asset_info: AssetInfo::NativeToken {
                            denom: "ujpy".to_string(),
                        },
                    },
                    SwapOperation::AstroSwap {
                        offer_asset_info: AssetInfo::NativeToken {
                            denom: "ujpy".to_string(),
                        },
                        ask_asset_info: AssetInfo::NativeToken {
                            denom: "ukrw".to_string(),
                        },
                    },
                ],
                fee_redeem: vec![Asset {
                    amount: Uint128::new(30_000),
                    info: AssetInfo::NativeToken {
                        denom: "uluna".to_string(),
                    },
                }],
            },
            &[],
        )
        .unwrap();

        // should have removed from tip balance
        let user_config = read_map(&app, dca_addr.clone(), &mock_creator().sender, USER_CONFIG);
        // 150,000 start - 30,000 fee redeem
        assert_eq!(
            user_config.tip_balance,
            vec![Asset {
                amount: Uint128::new(120_000),
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string()
                }
            }]
        );

        // should have added to bot balance
        let bot_balance = app.wrap().query_balance(bot_user, "uluna").unwrap();
        assert_eq!(bot_balance, coin(30_000, "uluna"));

        // should have bought the target token
        let user_balance = app
            .wrap()
            .query_balance(mock_creator().sender, "ukrw")
            .unwrap();
        // dca amount of 10_000 - fee = 9_558
        assert_eq!(user_balance, coin(9_558, "ukrw"));

        // should have updated dca order
        let user_dca_orders = read_map(&app, dca_addr, &mock_creator().sender, USER_DCA);
        let expected_orders = vec![DcaInfo {
            id: 1,
            interval: NORMAL_ORDER_INTERVAL,
            dca_amount: Uint128::new(10_000),
            initial_asset: Asset {
                amount: Uint128::new(90_000),
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
            },
            last_purchase: app.block_info().time.seconds(),
            target_asset: AssetInfo::NativeToken {
                denom: "ukrw".to_string(),
            },
        }];
        assert_eq!(user_dca_orders, expected_orders);
    }

    #[test]
    fn can_perform_token_purchase() {
        let (mut app, dca_addr, cw20_addr) = instantiate(None);

        // increase allowance for order
        app.execute_contract(
            mock_creator().sender,
            cw20_addr.clone(),
            &cw20_base::msg::ExecuteMsg::IncreaseAllowance {
                spender: dca_addr.clone().into_string(),
                amount: Uint128::new(100_000),
                expires: None,
            },
            &[],
        )
        .unwrap();

        create_normal_order(
            &mut app,
            dca_addr.clone(),
            AssetInfo::Token {
                contract_addr: cw20_addr.clone(),
            },
            AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        );

        add_tip_balance(&mut app, dca_addr.clone());

        let bot_user = Addr::unchecked("bot_user");

        // perform purchase
        app.execute_contract(
            bot_user.clone(),
            dca_addr.clone(),
            &ExecuteMsg::PerformDcaPurchase {
                user: mock_creator().sender.into_string(),
                id: 1,
                hops: vec![
                    SwapOperation::AstroSwap {
                        offer_asset_info: AssetInfo::Token {
                            contract_addr: cw20_addr.clone(),
                        },
                        ask_asset_info: AssetInfo::NativeToken {
                            denom: "ujpy".to_string(),
                        },
                    },
                    SwapOperation::AstroSwap {
                        offer_asset_info: AssetInfo::NativeToken {
                            denom: "ujpy".to_string(),
                        },
                        ask_asset_info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                    },
                ],
                fee_redeem: vec![Asset {
                    amount: Uint128::new(30_000),
                    info: AssetInfo::NativeToken {
                        denom: "uluna".to_string(),
                    },
                }],
            },
            &[],
        )
        .unwrap();

        // should have removed from tip balance
        let user_config = read_map(&app, dca_addr.clone(), &mock_creator().sender, USER_CONFIG);
        // 150,000 start - 30,000 fee redeem
        assert_eq!(
            user_config.tip_balance,
            vec![Asset {
                amount: Uint128::new(120_000),
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string()
                }
            }]
        );

        // should have added to bot balance
        let bot_balance = app.wrap().query_balance(bot_user, "uluna").unwrap();
        assert_eq!(bot_balance, coin(30_000, "uluna"));

        // should have spent the initial asset
        let user_balance: cw20::BalanceResponse = app
            .wrap()
            .query_wasm_smart(
                cw20_addr.clone(),
                &cw20_base::msg::QueryMsg::Balance {
                    address: mock_creator().sender.into_string(),
                },
            )
            .unwrap();
        // 500_000 - 10_000 = 490_000 left in balance
        assert_eq!(user_balance.balance, Uint128::new(490_000));

        // should have reduced from allowance
        let dca_allowance: cw20::AllowanceResponse = app
            .wrap()
            .query_wasm_smart(
                cw20_addr.clone(),
                &cw20_base::msg::QueryMsg::Allowance {
                    owner: mock_creator().sender.into_string(),
                    spender: dca_addr.clone().into_string(),
                },
            )
            .unwrap();
        assert_eq!(dca_allowance.allowance, Uint128::new(90_000));

        // should have bought the target asset
        let user_balance = app
            .wrap()
            .query_balance(mock_creator().sender, "uluna")
            .unwrap();
        // 500_000 starting balance - 150_000 tip + 9_558 from swap
        assert_eq!(user_balance, coin(350_000 + 9_558, "uluna"));

        // should have updated dca order
        let user_dca_orders = read_map(&app, dca_addr, &mock_creator().sender, USER_DCA);
        let expected_orders = vec![DcaInfo {
            id: 1,
            interval: NORMAL_ORDER_INTERVAL,
            dca_amount: Uint128::new(10_000),
            initial_asset: Asset {
                amount: Uint128::new(90_000),
                info: AssetInfo::Token {
                    contract_addr: cw20_addr,
                },
            },
            last_purchase: app.block_info().time.seconds(),
            target_asset: AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        }];
        assert_eq!(user_dca_orders, expected_orders);
    }

    #[test]
    fn does_error_if_empty_hops() {
        let (mut app, dca_addr, ..) = instantiate(None);

        create_normal_order(
            &mut app,
            dca_addr.clone(),
            AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
            AssetInfo::NativeToken {
                denom: "ukrw".to_string(),
            },
        );

        add_tip_balance(&mut app, dca_addr.clone());

        let bot_user = Addr::unchecked("bot_user");

        // perform purchase
        let res = app
            .execute_contract(
                bot_user,
                dca_addr,
                &ExecuteMsg::PerformDcaPurchase {
                    user: mock_creator().sender.into_string(),
                    id: 1,
                    hops: vec![],
                    fee_redeem: vec![Asset {
                        amount: Uint128::new(30_000),
                        info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                    }],
                },
                &[],
            )
            .unwrap_err();

        assert_eq!(
            res.downcast::<ContractError>().unwrap(),
            ContractError::EmptyHopRoute {}
        );
    }

    #[test]
    fn does_error_if_too_many_hops() {
        let (mut app, dca_addr, ..) = instantiate(Some(2));

        create_normal_order(
            &mut app,
            dca_addr.clone(),
            AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
            AssetInfo::NativeToken {
                denom: "ukrw".to_string(),
            },
        );

        add_tip_balance(&mut app, dca_addr.clone());

        let bot_user = Addr::unchecked("bot_user");

        // perform purchase
        let res = app
            .execute_contract(
                bot_user,
                dca_addr,
                &ExecuteMsg::PerformDcaPurchase {
                    user: mock_creator().sender.into_string(),
                    id: 1,
                    hops: vec![
                        SwapOperation::AstroSwap {
                            offer_asset_info: AssetInfo::NativeToken {
                                denom: "uluna".to_string(),
                            },
                            ask_asset_info: AssetInfo::NativeToken {
                                denom: "ujpy".to_string(),
                            },
                        },
                        SwapOperation::AstroSwap {
                            offer_asset_info: AssetInfo::NativeToken {
                                denom: "ujpy".to_string(),
                            },
                            ask_asset_info: AssetInfo::NativeToken {
                                denom: "ukrw".to_string(),
                            },
                        },
                        SwapOperation::AstroSwap {
                            offer_asset_info: AssetInfo::NativeToken {
                                denom: "ukrw".to_string(),
                            },
                            ask_asset_info: AssetInfo::NativeToken {
                                denom: "ujpy".to_string(),
                            },
                        },
                    ],
                    fee_redeem: vec![Asset {
                        amount: Uint128::new(30_000),
                        info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                    }],
                },
                &[],
            )
            .unwrap_err();

        assert_eq!(
            res.downcast::<ContractError>().unwrap(),
            ContractError::MaxHopsAssertion { hops: 3 }
        );
    }

    #[test]
    fn does_error_if_non_whitelisted_hop() {
        let (mut app, dca_addr, cw20_addr) = instantiate(None);

        create_normal_order(
            &mut app,
            dca_addr.clone(),
            AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
            AssetInfo::NativeToken {
                denom: "ukrw".to_string(),
            },
        );

        add_tip_balance(&mut app, dca_addr.clone());

        let bot_user = Addr::unchecked("bot_user");

        // perform purchase
        let res = app
            .execute_contract(
                bot_user.clone(),
                dca_addr.clone(),
                &ExecuteMsg::PerformDcaPurchase {
                    user: mock_creator().sender.into_string(),
                    id: 1,
                    hops: vec![
                        SwapOperation::AstroSwap {
                            offer_asset_info: AssetInfo::NativeToken {
                                denom: "uluna".to_string(),
                            },
                            ask_asset_info: AssetInfo::NativeToken {
                                denom: "ujpy".to_string(),
                            },
                        },
                        SwapOperation::AstroSwap {
                            offer_asset_info: AssetInfo::NativeToken {
                                denom: "ujpy".to_string(),
                            },
                            ask_asset_info: AssetInfo::Token {
                                contract_addr: cw20_addr.clone(),
                            },
                        },
                        SwapOperation::AstroSwap {
                            offer_asset_info: AssetInfo::Token {
                                contract_addr: cw20_addr.clone(),
                            },
                            ask_asset_info: AssetInfo::NativeToken {
                                denom: "ukrw".to_string(),
                            },
                        },
                    ],
                    fee_redeem: vec![Asset {
                        amount: Uint128::new(30_000),
                        info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                    }],
                },
                &[],
            )
            .unwrap_err();

        assert_eq!(
            res.downcast::<ContractError>().unwrap(),
            ContractError::InvalidHopRoute {
                token: cw20_addr.into_string()
            }
        );

        // should also error for native swap
        let res = app
            .execute_contract(
                bot_user,
                dca_addr,
                &ExecuteMsg::PerformDcaPurchase {
                    user: mock_creator().sender.into_string(),
                    id: 1,
                    hops: vec![
                        SwapOperation::NativeSwap {
                            offer_denom: "uluna".to_string(),
                            ask_denom: "ugbp".to_string(),
                        },
                        SwapOperation::AstroSwap {
                            offer_asset_info: AssetInfo::NativeToken {
                                denom: "ugbp".to_string(),
                            },
                            ask_asset_info: AssetInfo::NativeToken {
                                denom: "ukrw".to_string(),
                            },
                        },
                    ],
                    fee_redeem: vec![Asset {
                        amount: Uint128::new(30_000),
                        info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                    }],
                },
                &[],
            )
            .unwrap_err();

        assert_eq!(
            res.downcast::<ContractError>().unwrap(),
            ContractError::InvalidHopRoute {
                token: "ugbp".to_string()
            }
        );
    }

    #[test]
    fn does_check_requested_fee_whitelisted() {
        let (mut app, dca_addr, ..) = instantiate(None);

        create_normal_order(
            &mut app,
            dca_addr.clone(),
            AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
            AssetInfo::NativeToken {
                denom: "ukrw".to_string(),
            },
        );

        add_tip_balance(&mut app, dca_addr.clone());

        let bot_user = Addr::unchecked("bot_user");

        // perform purchase
        let res = app
            .execute_contract(
                bot_user,
                dca_addr,
                &ExecuteMsg::PerformDcaPurchase {
                    user: mock_creator().sender.into_string(),
                    id: 1,
                    hops: vec![
                        SwapOperation::AstroSwap {
                            offer_asset_info: AssetInfo::NativeToken {
                                denom: "uluna".to_string(),
                            },
                            ask_asset_info: AssetInfo::NativeToken {
                                denom: "ujpy".to_string(),
                            },
                        },
                        SwapOperation::AstroSwap {
                            offer_asset_info: AssetInfo::NativeToken {
                                denom: "ujpy".to_string(),
                            },
                            ask_asset_info: AssetInfo::NativeToken {
                                denom: "ukrw".to_string(),
                            },
                        },
                    ],
                    fee_redeem: vec![Asset {
                        amount: Uint128::new(35_000),
                        info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                    }],
                },
                &[],
            )
            .unwrap_err();

        assert_eq!(
            res.downcast::<ContractError>().unwrap(),
            ContractError::IndivisibleDeposit {}
        );
    }

    #[test]
    fn does_not_panic_for_zero_asset() {
        let mut deps = mock_dependencies();

        let cw20_addr = Addr::unchecked("cw20_addr");

        CONFIG
            .save(
                &mut deps.storage,
                &Config {
                    router_addr: Addr::unchecked("router"),
                    factory_addr: Addr::unchecked("factory"),
                    max_hops: 4,
                    max_spread: Decimal::from_str("0.05").unwrap(),
                    whitelisted_fee_assets: vec![Asset {
                        amount: Uint128::new(0),
                        info: AssetInfo::Token {
                            contract_addr: cw20_addr.clone(),
                        },
                    }],
                    whitelisted_tokens: vec![AssetInfo::NativeToken {
                        denom: "ujpy".to_string(),
                    }],
                },
            )
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_creator(),
            ExecuteMsg::PerformDcaPurchase {
                user: mock_creator().sender.into_string(),
                id: 1,
                hops: vec![
                    SwapOperation::AstroSwap {
                        offer_asset_info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                        ask_asset_info: AssetInfo::NativeToken {
                            denom: "ujpy".to_string(),
                        },
                    },
                    SwapOperation::AstroSwap {
                        offer_asset_info: AssetInfo::NativeToken {
                            denom: "ujpy".to_string(),
                        },
                        ask_asset_info: AssetInfo::NativeToken {
                            denom: "ukrw".to_string(),
                        },
                    },
                ],
                fee_redeem: vec![Asset {
                    amount: Uint128::new(30_000),
                    info: AssetInfo::Token {
                        contract_addr: cw20_addr,
                    },
                }],
            },
        )
        .unwrap_err();

        assert_eq!(
            res,
            ContractError::Std(StdError::DivideByZero {
                source: DivideByZeroError {
                    operand: "30000".to_string()
                }
            })
        );
    }

    #[test]
    fn does_check_tip_redeem_size() {
        let mut deps = mock_dependencies();

        CONFIG
            .save(
                &mut deps.storage,
                &Config {
                    router_addr: Addr::unchecked("router"),
                    factory_addr: Addr::unchecked("factory"),
                    max_hops: 4,
                    max_spread: Decimal::from_str("0.05").unwrap(),
                    whitelisted_fee_assets: vec![Asset {
                        amount: Uint128::new(15_000),
                        info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                    }],
                    whitelisted_tokens: vec![
                        AssetInfo::NativeToken {
                            denom: "ujpy".to_string(),
                        },
                        AssetInfo::NativeToken {
                            denom: "ukrw".to_string(),
                        },
                    ],
                },
            )
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_creator(),
            ExecuteMsg::PerformDcaPurchase {
                user: mock_creator().sender.into_string(),
                id: 1,
                hops: vec![
                    SwapOperation::AstroSwap {
                        offer_asset_info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                        ask_asset_info: AssetInfo::NativeToken {
                            denom: "ujpy".to_string(),
                        },
                    },
                    SwapOperation::AstroSwap {
                        offer_asset_info: AssetInfo::NativeToken {
                            denom: "ujpy".to_string(),
                        },
                        ask_asset_info: AssetInfo::NativeToken {
                            denom: "ukrw".to_string(),
                        },
                    },
                    SwapOperation::AstroSwap {
                        offer_asset_info: AssetInfo::NativeToken {
                            denom: "ukrw".to_string(),
                        },
                        ask_asset_info: AssetInfo::NativeToken {
                            denom: "ugbp".to_string(),
                        },
                    },
                ],
                fee_redeem: vec![Asset {
                    amount: Uint128::new(60_000),
                    info: AssetInfo::NativeToken {
                        denom: "uluna".to_string(),
                    },
                }],
            },
        )
        .unwrap_err();

        // should error because we wanted to do 15_000 * 4 = 60_000 fee redeem for a 15_000 * 3 swap
        assert_eq!(
            res,
            ContractError::RedeemTipTooLarge {
                requested: Uint128::new(4),
                performed: Uint128::new(3)
            }
        );
    }

    #[test]
    fn does_error_if_not_enough_balance() {
        let mut deps = mock_dependencies();

        CONFIG
            .save(
                &mut deps.storage,
                &Config {
                    router_addr: Addr::unchecked("router"),
                    factory_addr: Addr::unchecked("factory"),
                    max_hops: 4,
                    max_spread: Decimal::from_str("0.05").unwrap(),
                    whitelisted_fee_assets: vec![Asset {
                        amount: Uint128::new(15_000),
                        info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                    }],
                    whitelisted_tokens: vec![AssetInfo::NativeToken {
                        denom: "ujpy".to_string(),
                    }],
                },
            )
            .unwrap();

        USER_CONFIG
            .save(
                &mut deps.storage,
                &mock_creator().sender,
                &UserConfig {
                    last_id: 0,
                    max_hops: None,
                    max_spread: None,
                    tip_balance: vec![Asset {
                        info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                        amount: Uint128::new(25_000),
                    }],
                },
            )
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_creator(),
            ExecuteMsg::PerformDcaPurchase {
                user: mock_creator().sender.into_string(),
                id: 1,
                hops: vec![
                    SwapOperation::AstroSwap {
                        offer_asset_info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                        ask_asset_info: AssetInfo::NativeToken {
                            denom: "ujpy".to_string(),
                        },
                    },
                    SwapOperation::AstroSwap {
                        offer_asset_info: AssetInfo::NativeToken {
                            denom: "ujpy".to_string(),
                        },
                        ask_asset_info: AssetInfo::NativeToken {
                            denom: "ugbp".to_string(),
                        },
                    },
                ],
                fee_redeem: vec![Asset {
                    amount: Uint128::new(30_000),
                    info: AssetInfo::NativeToken {
                        denom: "uluna".to_string(),
                    },
                }],
            },
        )
        .unwrap_err();

        assert_eq!(res, ContractError::InsufficientTipBalance {});
    }

    #[test]
    fn can_purchase_with_token_fee() {
        let (mut app, dca_addr, cw20_addr) = instantiate(None);

        create_normal_order(
            &mut app,
            dca_addr.clone(),
            AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
            AssetInfo::NativeToken {
                denom: "ukrw".to_string(),
            },
        );

        // add bot tips
        app.execute_contract(
            mock_creator().sender,
            cw20_addr.clone(),
            &cw20_base::msg::ExecuteMsg::IncreaseAllowance {
                spender: dca_addr.clone().into_string(),
                amount: Uint128::new(45_000),
                expires: None,
            },
            &[],
        )
        .unwrap();

        app.execute_contract(
            mock_creator().sender,
            dca_addr.clone(),
            &ExecuteMsg::AddBotTip {
                assets: vec![Asset {
                    amount: Uint128::new(45_000),
                    info: AssetInfo::Token {
                        contract_addr: cw20_addr.clone(),
                    },
                }],
            },
            &[],
        )
        .unwrap();

        // should succeed
        app.execute_contract(
            Addr::unchecked("bot_addr"),
            dca_addr,
            &ExecuteMsg::PerformDcaPurchase {
                user: mock_creator().sender.into_string(),
                id: 1,
                hops: vec![
                    SwapOperation::AstroSwap {
                        offer_asset_info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                        ask_asset_info: AssetInfo::NativeToken {
                            denom: "ujpy".to_string(),
                        },
                    },
                    SwapOperation::AstroSwap {
                        offer_asset_info: AssetInfo::NativeToken {
                            denom: "ujpy".to_string(),
                        },
                        ask_asset_info: AssetInfo::NativeToken {
                            denom: "ukrw".to_string(),
                        },
                    },
                ],
                fee_redeem: vec![Asset {
                    amount: Uint128::new(30_000),
                    info: AssetInfo::Token {
                        contract_addr: cw20_addr.clone(),
                    },
                }],
            },
            &[],
        )
        .unwrap();

        // should have sent the funds to the bot addr
        let res: BalanceResponse = app
            .wrap()
            .query_wasm_smart(
                cw20_addr.into_string(),
                &cw20::Cw20QueryMsg::Balance {
                    address: "bot_addr".to_string(),
                },
            )
            .unwrap();
        assert_eq!(res.balance, Uint128::new(30_000));
    }

    #[test]
    fn does_error_if_purchase_too_early() {
        let (mut deps, env) = mock_instantiate(
            Addr::unchecked("factory"),
            Addr::unchecked("router"),
            vec![Asset {
                amount: Uint128::new(15_000),
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
            }],
            vec![AssetInfo::NativeToken {
                denom: "ujpy".to_string(),
            }],
        );

        USER_CONFIG
            .save(
                &mut deps.storage,
                &mock_creator().sender,
                &UserConfig {
                    last_id: 1,
                    max_hops: None,
                    max_spread: None,
                    tip_balance: vec![Asset {
                        amount: Uint128::new(45_000),
                        info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                    }],
                },
            )
            .unwrap();

        USER_DCA
            .save(
                &mut deps.storage,
                &mock_creator().sender,
                &vec![DcaInfo {
                    id: 1,
                    dca_amount: Uint128::new(10_000),
                    initial_asset: Asset {
                        info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                        amount: Uint128::new(100_000),
                    },
                    target_asset: AssetInfo::NativeToken {
                        denom: "ukrw".to_string(),
                    },
                    interval: 500,
                    last_purchase: env.block.time.seconds(),
                }],
            )
            .unwrap();

        // should fail when purchasing
        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_creator(),
            ExecuteMsg::PerformDcaPurchase {
                user: mock_creator().sender.into_string(),
                id: 1,
                hops: vec![
                    SwapOperation::AstroSwap {
                        offer_asset_info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                        ask_asset_info: AssetInfo::NativeToken {
                            denom: "ujpy".to_string(),
                        },
                    },
                    SwapOperation::AstroSwap {
                        offer_asset_info: AssetInfo::NativeToken {
                            denom: "ujpy".to_string(),
                        },
                        ask_asset_info: AssetInfo::NativeToken {
                            denom: "ukrw".to_string(),
                        },
                    },
                ],
                fee_redeem: vec![Asset {
                    amount: Uint128::new(30_000),
                    info: AssetInfo::NativeToken {
                        denom: "uluna".to_string(),
                    },
                }],
            },
        )
        .unwrap_err();

        assert_eq!(res, ContractError::PurchaseTooEarly {});
    }

    #[test]
    fn does_not_panic_if_dca_too_big() {
        let (mut deps, ..) = mock_instantiate(
            Addr::unchecked("factory"),
            Addr::unchecked("router"),
            vec![Asset {
                amount: Uint128::new(15_000),
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
            }],
            vec![AssetInfo::NativeToken {
                denom: "ujpy".to_string(),
            }],
        );

        USER_CONFIG
            .save(
                &mut deps.storage,
                &mock_creator().sender,
                &UserConfig {
                    last_id: 1,
                    max_hops: None,
                    max_spread: None,
                    tip_balance: vec![Asset {
                        amount: Uint128::new(45_000),
                        info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                    }],
                },
            )
            .unwrap();

        USER_DCA
            .save(
                &mut deps.storage,
                &mock_creator().sender,
                &vec![DcaInfo {
                    id: 1,
                    dca_amount: Uint128::new(10_000),
                    initial_asset: Asset {
                        info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                        amount: Uint128::new(5_000),
                    },
                    target_asset: AssetInfo::NativeToken {
                        denom: "ukrw".to_string(),
                    },
                    interval: 500,
                    last_purchase: 0,
                }],
            )
            .unwrap();

        // should fail when purchasing
        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_creator(),
            ExecuteMsg::PerformDcaPurchase {
                user: mock_creator().sender.into_string(),
                id: 1,
                hops: vec![
                    SwapOperation::AstroSwap {
                        offer_asset_info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                        ask_asset_info: AssetInfo::NativeToken {
                            denom: "ujpy".to_string(),
                        },
                    },
                    SwapOperation::AstroSwap {
                        offer_asset_info: AssetInfo::NativeToken {
                            denom: "ujpy".to_string(),
                        },
                        ask_asset_info: AssetInfo::NativeToken {
                            denom: "ukrw".to_string(),
                        },
                    },
                ],
                fee_redeem: vec![Asset {
                    amount: Uint128::new(30_000),
                    info: AssetInfo::NativeToken {
                        denom: "uluna".to_string(),
                    },
                }],
            },
        )
        .unwrap_err();

        assert_eq!(res, ContractError::InsufficientBalance {});
    }

    #[test]
    fn does_error_if_not_target_hop() {
        let (mut deps, ..) = mock_instantiate(
            Addr::unchecked("factory"),
            Addr::unchecked("router"),
            vec![Asset {
                amount: Uint128::new(15_000),
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
            }],
            vec![AssetInfo::NativeToken {
                denom: "ujpy".to_string(),
            }],
        );

        USER_CONFIG
            .save(
                &mut deps.storage,
                &mock_creator().sender,
                &UserConfig {
                    last_id: 1,
                    max_hops: None,
                    max_spread: None,
                    tip_balance: vec![Asset {
                        amount: Uint128::new(45_000),
                        info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                    }],
                },
            )
            .unwrap();

        USER_DCA
            .save(
                &mut deps.storage,
                &mock_creator().sender,
                &vec![DcaInfo {
                    id: 1,
                    dca_amount: Uint128::new(10_000),
                    initial_asset: Asset {
                        info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                        amount: Uint128::new(100_000),
                    },
                    target_asset: AssetInfo::NativeToken {
                        denom: "ukrw".to_string(),
                    },
                    interval: 500,
                    last_purchase: 0,
                }],
            )
            .unwrap();

        // should fail when purchasing
        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_creator(),
            ExecuteMsg::PerformDcaPurchase {
                user: mock_creator().sender.into_string(),
                id: 1,
                hops: vec![
                    SwapOperation::AstroSwap {
                        offer_asset_info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                        ask_asset_info: AssetInfo::NativeToken {
                            denom: "ujpy".to_string(),
                        },
                    },
                    SwapOperation::AstroSwap {
                        offer_asset_info: AssetInfo::NativeToken {
                            denom: "ujpy".to_string(),
                        },
                        ask_asset_info: AssetInfo::NativeToken {
                            denom: "ugbp".to_string(),
                        },
                    },
                ],
                fee_redeem: vec![Asset {
                    amount: Uint128::new(30_000),
                    info: AssetInfo::NativeToken {
                        denom: "uluna".to_string(),
                    },
                }],
            },
        )
        .unwrap_err();

        assert_eq!(res, ContractError::TargetAssetAssertion {});
    }

    #[test]
    fn does_delete_order_if_fulfilled() {
        let (mut deps, ..) = mock_instantiate(
            Addr::unchecked("factory"),
            Addr::unchecked("router"),
            vec![Asset {
                amount: Uint128::new(15_000),
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
            }],
            vec![AssetInfo::NativeToken {
                denom: "ujpy".to_string(),
            }],
        );

        USER_CONFIG
            .save(
                &mut deps.storage,
                &mock_creator().sender,
                &UserConfig {
                    last_id: 1,
                    max_hops: None,
                    max_spread: None,
                    tip_balance: vec![Asset {
                        amount: Uint128::new(45_000),
                        info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                    }],
                },
            )
            .unwrap();

        USER_DCA
            .save(
                &mut deps.storage,
                &mock_creator().sender,
                &vec![DcaInfo {
                    id: 1,
                    dca_amount: Uint128::new(10_000),
                    initial_asset: Asset {
                        info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                        amount: Uint128::new(10_000),
                    },
                    target_asset: AssetInfo::NativeToken {
                        denom: "ukrw".to_string(),
                    },
                    interval: 500,
                    last_purchase: 0,
                }],
            )
            .unwrap();

        // should fail when purchasing
        let hops = vec![
            SwapOperation::AstroSwap {
                offer_asset_info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
                ask_asset_info: AssetInfo::NativeToken {
                    denom: "ujpy".to_string(),
                },
            },
            SwapOperation::AstroSwap {
                offer_asset_info: AssetInfo::NativeToken {
                    denom: "ujpy".to_string(),
                },
                ask_asset_info: AssetInfo::NativeToken {
                    denom: "ukrw".to_string(),
                },
            },
        ];

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bot_addr", &[]),
            ExecuteMsg::PerformDcaPurchase {
                user: mock_creator().sender.into_string(),
                id: 1,
                hops: hops.clone(),
                fee_redeem: vec![Asset {
                    amount: Uint128::new(30_000),
                    info: AssetInfo::NativeToken {
                        denom: "uluna".to_string(),
                    },
                }],
            },
        )
        .unwrap();

        let expected_msgs: Vec<CosmosMsg> = vec![
            BankMsg::Send {
                amount: coins(30_000, "uluna"),
                to_address: "bot_addr".to_string(),
            }
            .into(),
            WasmMsg::Execute {
                contract_addr: "router".to_string(),
                funds: coins(10_000, "uluna"),
                msg: to_binary(&astroport::router::ExecuteMsg::ExecuteSwapOperations {
                    operations: hops,
                    minimum_receive: None,
                    to: Some(mock_creator().sender.into_string()),
                    max_spread: Some(Decimal::from_str("0.05").unwrap()),
                })
                .unwrap(),
            }
            .into(),
        ];

        assert_eq!(
            res,
            Response::new()
                .add_messages(expected_msgs)
                .add_attributes(vec![
                    attr("action", "perform_dca_purchase"),
                    attr("user", mock_creator().sender.into_string()),
                    attr("id", "1"),
                ])
        );
    }
}
