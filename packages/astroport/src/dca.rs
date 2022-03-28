use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    asset::{Asset, AssetInfo},
    router::SwapOperation,
};

use cosmwasm_std::Uint128;

/// Describes information about a dca strategy
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DcaInfo {
    /// The starting asset deposited by the user, with the amount representing the users deposited
    /// amount of the token
    pub initial_asset: Asset,
    /// The asset being purchased in DCA purchases
    pub target_asset: AssetInfo,
    /// The interval in seconds between DCA purchases
    pub interval: u64,
    /// The last time the `target_asset` was purchased
    pub last_purchase: u64,
    /// The amount of `initial_asset` to spend each DCA purchase
    pub dca_amount: Uint128,
}

/// Describes the parameters used for creating a contract
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// The maximum amount of hops to perform from `initial_asset` to `target_asset` when DCAing if the user does not specify a custom max hop amount
    pub max_hops: u32,
    /// The fee a user must pay per hop performed in a DCA purchase
    pub per_hop_fee: Uint128,
    /// The whitelisted tokens that can be used in a DCA purchase route
    pub whitelisted_tokens: Vec<AssetInfo>,
    /// The maximum amount of spread
    pub max_spread: String,
    /// The address of the Astroport factory contract
    pub factory_addr: String,
    /// The address of the Astroport router contract
    pub router_addr: String,
}

/// This structure describes the execute messages available in the contract
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Updates the configuration of the contract
    UpdateConfig {
        /// The new maximum amount of hops to perform from `initial_asset` to `target_asset` when DCAing if the user does not specify a custom max hop amount
        max_hops: Option<u32>,
        /// The new fee a user must pay per hop performed in a DCA purchase
        per_hop_fee: Option<Uint128>,
        /// The new whitelisted tokens that can be used in a DCA purchase route
        whitelisted_tokens: Option<Vec<AssetInfo>>,
        /// The new maximum spread for DCA purchases
        max_spread: Option<String>,
    },
    /// Update the configuration for a user
    UpdateUserConfig {
        /// The maximum amount of hops per swap
        max_hops: Option<u32>,
        /// The maximum spread per token when DCAing
        ///
        /// Must be between 0 and `MAX_ALLOWED_SLIPPAGE`
        max_spread: Option<String>,
    },
    /// Creates a new DCA strategy where `dca_amount` of token `initial_asset` will purchase
    /// `target_asset` every `interval`
    ///
    /// If `initial_asset` is a cw20 token, the user needs to have increased the allowance prior to
    /// calling this execution
    CreateDcaOrder {
        initial_asset: Asset,
        target_asset: AssetInfo,
        interval: u64,
        dca_amount: Uint128,
    },
    /// Add uusd top-up for bots to perform DCA requests
    AddBotTip {},
    /// Withdraws a users bot tip from the contract.
    Withdraw { tip: Uint128 },
    // Performs a DCA purchase for a specified user given a route
    PerformDcaPurchase {
        user: String,
        hops: Vec<SwapOperation>,
    },
    /// Cancels a DCA order, returning any native asset back to the user
    CancelDcaOrder { initial_asset: Asset },
}

/// This structure describes the query messages available in the contract
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Returns information about the users current being DCA orders in a [`GetUserAssetsResponse`] object.
    UserDcaOrders { user: String },
    /// Returns information about the contract configuration in a [`Config`] object.
    Config {},
    /// Returns the users current configuration as a [`UserConfig`] object.
    UserConfig { user: String },
}

/// This structure describes a migration message.
/// We currently take no arguments for migrations.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}
