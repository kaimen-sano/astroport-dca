use astroport::asset::addr_validate_to_lower;
use cosmwasm_std::{Deps, StdResult};

use crate::state::{UserConfig, USER_CONFIG};

/// ## Description
/// Returns the configuration set for a user to override the default contract configuration.
///
/// The result is returned in a [`UserConfig`] object.
///
/// ## Arguments
/// * `deps` - A [`Deps`] that contains the dependencies.
///
/// * `user` - The users lowercase address as a [`String`].
pub fn get_user_config(deps: Deps, user: String) -> StdResult<UserConfig> {
    let user_address = addr_validate_to_lower(deps.api, &user)?;

    USER_CONFIG.load(deps.storage, &user_address)
}

#[cfg(test)]
mod test {
    use astroport::asset::{Asset, AssetInfo};
    use astroport_dca::dca::QueryMsg;
    use cosmwasm_std::{
        from_binary,
        testing::{mock_dependencies, mock_env},
        Addr, Uint128,
    };

    use crate::{
        contract::query,
        state::{UserConfig, USER_CONFIG},
    };

    #[test]
    fn does_get_user_config() {
        let mut deps = mock_dependencies();

        let config = UserConfig {
            last_id: 5,
            max_hops: Some(3),
            max_spread: None,
            tip_balance: vec![Asset {
                amount: Uint128::new(20_000),
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
            }],
        };

        let key = Addr::unchecked("user_addr");
        USER_CONFIG.save(&mut deps.storage, &key, &config).unwrap();

        let res: UserConfig = from_binary(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::UserConfig {
                    user: key.into_string(),
                },
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(res, config);
    }
}
