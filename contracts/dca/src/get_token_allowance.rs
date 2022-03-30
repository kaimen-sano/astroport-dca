use cosmwasm_std::{Addr, Deps, Env, StdResult, Uint128};
use cw20::{AllowanceResponse, Cw20QueryMsg};

/// ## Description
/// Retrieves the allowed token allowance for the contract for a Cw20 token as a [`Uint128`].
/// ## Arguments
/// * `deps` - A [`Deps`] that contains the dependencies.
///
/// * `env` - The [`Env`] of the blockchain.
///
/// * `owner` - The user that holds the Cw20 token.
///
/// * `contract_address` - The address of the Cw20 token.
pub fn get_token_allowance(
    deps: &Deps,
    env: &Env,
    owner: &Addr,
    contract_address: &Addr,
) -> StdResult<Uint128> {
    let allowance_response: AllowanceResponse = deps.querier.query_wasm_smart(
        contract_address,
        &Cw20QueryMsg::Allowance {
            owner: owner.to_string(),
            spender: env.contract.address.to_string(),
        },
    )?;

    Ok(allowance_response.allowance)
}
