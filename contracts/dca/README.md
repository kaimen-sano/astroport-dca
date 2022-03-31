# Astroport DCA Module

The DCA contract contains logic to facilitate users performing DCA orders (dollar cost averaging) over a period of time. Users can create DCA orders which will then be fulfilled by another user after enough time has occurred, with them specifying the purchase route from the deposited asset to the target asset. This route can only swap through whitelisted tokens by the contract.

## InstantiateMsg

Initializes the contract with the configuration settings, the [Astroport factory contract](https://github.com/astroport-fi/astroport-core/tree/main/contracts/factory) address and the [Astroport router contract](https://github.com/astroport-fi/astroport-core/tree/main/contracts/router) address.

```json
{
  "factory_addr": "terra...",
  "router_addr": "terra...",
  "max_hops": 4,
  "max_spread": "0.05",
  "per_hop_fee": "100000",
  "whitelisted_tokens": [
    { "native_token": { "denom": "uusd" } },
    { "token": { "contract_ddr": "terra..." } }
  ]
}
```

## ExecuteMsg

### `update_config`

Updates the contract configuration with the specified optional parameters.

Any parameters that are not specified will be left unchanged.

```json
{
  "update_config": {
    // set max_spread to 0.1
    "max_spread": "0.1",
    // leave max_hops, per_hop_fee, whitelisted_tokens unchanged
    "max_hops": null,
    "per_hop_fee": null,
    "whitelisted_tokens": null
  }
}
```

### `update_user_config`

Updates a users configuration with the specified parameters.

Any parameters that are not specified will be reset in the configuration so that the user uses the contract set configuration values.

```json
{
  "update_user_config": {
    // make the user use the contract set max_hops
    "max_hops": null,
    "max_spread": "0.15"
  }
}
```

### `add_bot_tip`

Add uusd top-up for bots to perform DCA requests

uusd fund must be added to message.

```json
{
  "add_bot_tip": {}
}
```

### `withdraw`

Withdraws a users previously deposited bot tip from the contract.

Tip specified will be returned back to the user.

```json
{
  "withdraw": {
    // withdraw 0.1 UST tip deposited
    "tip": "100000"
  }
}
```

### `create_dca_order`

Creates a new DCA order where a deposited asset will purchase a target asset at a specified interval.

If the deposited asset is a CW20 token, the user needs to have increased the allowance prior to calling this execution.

If the deposited asset is a native token, the user needs to attach the token to the execution message.

Example: Purchase 5 UST worth of Luna each day, with 15 UST.

```json
{
  "create_dca_order": {
    "dca_amount": "5000000",
    "initial_asset": {
      "info": { "native_token": { "denom": "uusd" } },
      "amount": "15000000"
    },
    "interval": "86400",
    "target_asset": {
      "native_token": { "denom": "uluna" }
    }
  }
}
```

### `modify_dca_order`

Modifies an existing DCA order, allowing the user to change certain parameters.

Example: Change existing order which used uusd to purchase luna to now purchase ukrw with uusd each week. Also increase the size of the order to now be 30 UST (we must send an additional 15 UST in the message).

```json
{
  "modify_dca_order": {
    "new_dca_amount": "1000000",
    "old_initial_asset": { "native_token": { "denom": "uusd" } },
    "new_initial_asset": {
      "info": { "native_token": { "denom": "uusd" } },
      "amount": "15000000"
    },
    "new_interval": 604800,
    "new_target_asset": { "native_token": { "denom": "ukrw" } },
    "should_reset_purchase_time": true
  }
}
```

### `cancel_dca_order`

Cancels a DCA order, returning any native asset back to the user.

```json
{
  "cancel_dca_order": {
    "initial_asset": { "native_token": { "denom": "uusd" } }
  }
}
```

### `perform_dca_purchase`

Performs a DCA purchase for a specified user given a hop route.

Returns a uusd tip from the user for purchasing the assets on their behalf.

For more information about the `hops`, see the [Astroport router](https://docs.astroport.fi/astroport/smart-contracts/router) documentation.

```json
{
	"perform_dca_purchase": {
		"user": "terra...",
		"hops": [
			"native_swap": {
				"ask_denom": "uluna",
				"offer_denom": "uusd"
			},
			"astro_swap": {
				"ask_asset_info": {
					"token": {
						"contract_addr": "terra..."
					}
				},
				"offer_denom": "uluna"
			}
		]
	}
}
```

## QueryMsg

All query messages are described below.

### `config`

Returns information about the contract configuration (`max_hops`, `max_spread`, etc).

```json
{
  "config": {}
}
```

Example response:

```json
{
  "config": {
    "factory_addr": "terra...",
    "router_addr": "terra...",
    "max_hops": 32,
    "max_spread": "0.05",
    "per_hop_fee": "100000",
    "whitelisted_tokens": [
      { "native_token": { "denom": "uusd" } },
      { "token": { "contract_addr": "terra..." } }
    ]
  }
}
```

### `user_config`

Returns the users current configuration (custom override `max_hops`, `max_spread`, uusd tip balance deposited).

```json
{
  "user_config": {}
}
```

Example response:

```json
{
  "max_hops": 2,
  "max_spread": "0.5",
  "tip_balance": "50000000"
}
```

### `user_dca_orders`

Returns information about the users current active DCA orders.

```json
{
  "user_dca_orders": {
    "user": "terra..."
  }
}
```

Example response for two DCA orders:

```json
[
  {
    "initial_asset": {
      "amount": "15000000",
      "info": {
        "native_token": { "denom": "uusd" }
      }
    },
    "target_asset": {
      "token": { "contract_addr": "terra..." }
    },
    "interval": 60,
    "last_purchase": 1230940800,
    "dca_amount": "3000000"
  },
  {
    "initial_asset": {
      "amount": "300000000",
      "info": {
        "token": { "contract_addr": "terra..." }
      }
    },
    "target_asset": {
      "token": { "contract_addr": "terra..." }
    },
    "interval": 3600,
    "last_purchase": 1230940800,
    "dca_amount": "10000000"
  }
]
```
