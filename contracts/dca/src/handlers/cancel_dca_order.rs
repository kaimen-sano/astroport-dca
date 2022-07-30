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
/// * `initial_asset` The [`u64`] ID which the user wants to cancel the DCA order for.
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

    Ok(Response::new()
        .add_messages(funds)
        .add_attributes(vec![attr("action", "cancel_dca_order")]))
}

#[cfg(test)]
mod tests {
    use astroport::asset::{Asset, AssetInfo};
    use astroport_dca::dca::ExecuteMsg;
    use cosmwasm_std::{testing::mock_env, DepsMut, MessageInfo, Uint128};

    use crate::contract::execute;

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

    /* #[test]
    fn does_cancel_token_order() {
        let mut deps = mock_dependencies();

        deps.querier.with_token_balances(&[(
            &"token".to_string(),
            &[(&"owner".to_string(), &Uint128::from(15_000_000u64))],
        )]);

        let token = AssetInfo::Token {
            contract_addr: Addr::unchecked("token"),
        };
        let target_asset = AssetInfo::Token {
            contract_addr: Addr::unchecked("token2"),
        };

        let info = mock_info("creator", &[]);

        // create dca order
        create_order(deps.as_mut(), info.clone(), token.clone(), target_asset);

        // cancel dca order
        let msg = ExecuteMsg::CancelDcaOrder {
            initial_asset: token,
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        println!("{:?}", res);
    } */
}
