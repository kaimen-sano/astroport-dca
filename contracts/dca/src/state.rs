use astroport::asset::{Asset, AssetInfo};
use cosmwasm_std::{Addr, Decimal};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use astroport_dca::dca::DcaInfo;

/// Stores the main dca module parameters.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// The maximum amount of hops to perform from `initial_asset` to `target_asset` when DCAing if
    /// the user does not specify a custom max hop amount
    pub max_hops: u32,
    /// The default for the maximum amount of spread in a swap
    pub max_spread: Decimal,
    /// The whitelisted tokens that can be used for bot tips, along with the fee for each hop
    pub whitelisted_fee_assets: Vec<Asset>,
    /// The whitelisted tokens that can be used in a DCA hop route
    pub whitelisted_tokens: Vec<AssetInfo>,
    /// The address of the Astroport factory contract
    pub factory_addr: Addr,
    /// The address of the Astroport router contract
    pub router_addr: Addr,
}

impl Config {
    /// Checks if a given `asset` is a whitelisted asset that can be used in a hop route
    pub fn is_whitelisted_asset(&self, asset: &AssetInfo) -> bool {
        self.whitelisted_tokens.contains(asset)
    }

    /// Checks if a given `asset` is a whitelisted asset for paying bot tips
    pub fn is_whitelisted_fee_asset(&self, asset: &AssetInfo) -> bool {
        self.whitelisted_fee_assets.iter().any(|a| &a.info == asset)
    }
}

/// Stores the users custom configuration
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct UserConfig {
    /// A unique identifier for DCA orders, increments each time `create_order` is called.
    pub last_id: u64,
    /// An override for the maximum amount of hops to perform from `initial_asset` to `target_asset`
    /// when DCAing
    pub max_hops: Option<u32>,
    /// An override for the maximum amount of spread when performing a swap from `initial_asset` to
    /// `target_asset` when DCAing
    pub max_spread: Option<Decimal>,
    /// The tip balance the user has deposited for their tips when performing DCA purchases
    pub tip_balance: Vec<Asset>,
}

/// The contract configuration
pub const CONFIG: Item<Config> = Item::new("config");
/// The configuration set by each user
pub const USER_CONFIG: Map<&Addr, UserConfig> = Map::new("user_config");
/// The DCA orders for a user
pub const USER_DCA: Map<&Addr, Vec<DcaInfo>> = Map::new("user_dca");
