use astroport::asset::AssetInfo;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use astroport::dca::DcaInfo;

/// Stores the main dca module parameters.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// The maximum amount of hops to perform from `initial_asset` to `target_asset` when DCAing if the user does not specify
    pub max_hops: u32,
    /// The maximum amount of spread when performing a swap from `initial_asset` to `target_asset` when DCAing if the user does not specify
    pub max_spread: String,
    /// The fee a user must pay per hop performed in a DCA purchase
    pub per_hop_fee: Uint128,
    /// The whitelisted tokens that can be used in a DCA purchase route
    pub whitelisted_tokens: Vec<AssetInfo>,
    /// The address of the Astroport factory contract
    pub factory_addr: Addr,
    /// The address of the Astroport router contract
    pub router_addr: Addr,
}

impl Config {
    pub fn is_whitelisted_asset(&self, asset: &AssetInfo) -> bool {
        self.whitelisted_tokens.contains(asset)
    }
}

/// Stores the users custom configuration
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserConfig {
    /// An override for the maximum amount of hops to perform from `initial_asset` to `target_asset` when DCAing
    pub max_hops: Option<u32>,
    /// An override for the maximum amount of spread when performing a swap from `initial_asset` to `target_asset` when DCAing
    pub max_spread: Option<String>,
    /// The amount of uusd the user has deposited for their tips when performing DCA purchases
    pub tip_balance: Uint128,
}

impl Default for UserConfig {
    fn default() -> Self {
        UserConfig {
            max_hops: None,
            max_spread: None,
            tip_balance: Uint128::zero(),
        }
    }
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const USER_CONFIG: Map<&Addr, UserConfig> = Map::new("user_config");
pub const USER_DCA: Map<&Addr, Vec<DcaInfo>> = Map::new("user_dca");
