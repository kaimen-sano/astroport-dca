use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use astroport::{
    asset::{Asset, AssetInfo},
    router::SwapOperation,
};

use cosmwasm_std::{Decimal, Uint128};

/// Describes information about a DCA order
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DcaInfo {
    /// The ID of the order
    pub id: u64,
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
    /// The maximum amount of hops to perform from `initial_asset` to `target_asset` when DCAing if
    /// the user does not specify a custom max hop amount
    pub max_hops: u32,
    /// The whitelisted tokens that can be used in a DCA hop route
    pub whitelisted_tokens: Vec<AssetInfo>,
    /// The whitelisted tokens that can be used for bot tips, along with the fee for each hop
    pub whitelisted_fee_assets: Vec<Asset>,
    /// The default for the maximum amount of spread in a swap
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
    /// Add top-up for bots to perform DCA requests with the specified `assets` sent in the tx
    AddBotTip { assets: Vec<Asset> },
    /// Cancels a DCA order given the id of the order, returning any native asset back to the user
    CancelDcaOrder { id: u64 },
    /// Creates a new DCA order where `dca_amount` of token `initial_asset` will purchase
    /// `target_asset` every `interval`
    ///
    /// If `initial_asset` is a cw20 token, the user needs to have increased the allowance prior to
    /// calling this execution
    ///
    /// DCA purchases can start from `first_purchase` if specified, otherwise as soon as the DCA
    /// order is made
    CreateDcaOrder {
        initial_asset: Asset,
        target_asset: AssetInfo,
        interval: u64,
        dca_amount: Uint128,
        first_purchase: Option<u64>,
    },
    /// Modifies an existing DCA order, allowing the user to change certain parameters
    ModifyDcaOrder {
        id: u64,
        new_initial_asset: Asset,
        new_target_asset: AssetInfo,
        new_interval: u64,
        new_dca_amount: Uint128,
        new_first_purchase: Option<u64>,
    },
    /// Performs a DCA purchase for a specified user given a hop route and requested fee redemption
    PerformDcaPurchase {
        user: String,
        id: u64,
        hops: Vec<SwapOperation>,
        fee_redeem: Vec<Asset>,
    },
    /// Updates the configuration of the contract
    UpdateConfig {
        /// The new maximum amount of hops to perform from `initial_asset` to `target_asset` when
        /// performing DCA purchases if the user does not specify a custom max hop amount
        max_hops: Option<u32>,
        /// The new whitelisted tokens that can be used in a DCA hop route
        whitelisted_tokens: Option<Vec<AssetInfo>>,
        /// The whitelisted tokens that can be used for bot tips, along with the fee for each hop
        whitelisted_fee_assets: Option<Vec<Asset>>,
        /// The new maximum spread for DCA purchases
        max_spread: Option<Decimal>,
    },
    /// Update the configuration for a user
    UpdateUserConfig {
        /// The maximum amount of hops per swap
        max_hops: Option<u32>,
        /// The maximum spread per token when performing DCA purchases
        max_spread: Option<Decimal>,
    },
    /// Withdraws the `assets` amount of a users bot tip from the contract.
    Withdraw { assets: Vec<Asset> },
}

/// This structure describes the query messages available in the contract
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Returns information about the users current active DCA orders in a [`Vec<DcaInfo>`] object.
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

/// Describes information for a UserDcaOrders query
///
/// Contains both the user DCA order and the cw20 token allowance, or, if the initial asset is a
/// native token, the balance.
///
/// This is useful for bots and front-end to distinguish between a users token allowance (which may
/// have changed) for the DCA contract, and the created DCA order size.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DcaQueryInfo {
    pub token_allowance: Uint128,
    pub order: DcaInfo,
}
