# Astroport Factory

The factory contract can perform creation of astroport pair contract and used as directory contract for all pairs. Available pair types are stable and xyk only.

README has updated with new messages (Astroport v1 messages follow).

---

## InstantiateMsg

Code ID for a pair type is provided when instantiating a new pair. So, you don’t have to execute pair contract additionally.

```json
{
  "paid_code_id": 123,
  "token_code_id": 123,
  "fee_address": "terra...",
  "gov": "terra...",
  "init_hook": {
    "msg": "<base64_encoded_json_string>",
    "contract_addr": "terra..."
  }
}
```

## ExecuteMsg

### `update_config`

```json
{
  "update_config": {
    "gov": "terra...",
    "owner": "terra...",
    "token_code_id": 123,
    "fee_address": "terra..."
  }
}
```

### `update_pair_config`

Updating code id and fees for specified pair type. All fields are optional.

```json
{
  "update_pair_config": {
    "config": {
      "code_id": 123,
      "pair_type": {
        "xyk": {}
      },
      "total_fee_bps": 100,
      "maker_fee_bps": 10
    }
  }
}
```

### `remove_pair_config`

Removing config for specified pair type.

```json
{
  "pair_type": {
    "stable": {}
  }
}
```

### `create_pair`

Anyone can execute it to create swap pair. When a user executes `CreatePair` operation, it creates `Pair` contract and `LP(liquidity provider)` token contract. It also creates not fully initialized `PairInfo`, which will be initialized with `Register` operation from the pair contract's `InitHook`.

```json
{
  "create_pair": {
    "pair_type": {
      "xyk": {}
    },
    "asset_infos": [
      {
        "token": {
          "contract_addr": "terra..."
        }
      },
      {
        "native_token": {
          "denom": "uusd"
        }
      }
    ]
  },
  "init_hook": {
    "msg": "<base64_encoded_json_string>",
    "contract_addr": "terra..."
  }
}
```

### `register`

When a user executes `CreatePair` operation, it passes `InitHook` to `Pair` contract and `Pair` contract will invoke passed `InitHook` registering created `Pair` contract to the factory. This operation is only allowed for a pair, which is not fully initialized.

Once a `Pair` contract invokes it, the sender address is registered as `Pair` contract address for the given asset_infos.

```json
{
  "register": {
    "asset_infos": [
      {
        "token": {
          "contract_address": "terra..."
        }
      },
      {
        "native_token": {
          "denom": "uusd"
        }
      }
    ]
  }
}
```

### `deregister`

Deregisters already registered pair (deletes pair).

```json
{
  "deregister": {
    "asset_infos": [
      {
        "token": {
          "contract_address": "terra..."
        }
      },
      {
        "native_token": {
          "denom": "uusd"
        }
      }
    ]
  }
}
```

## QueryMsg

All query messages are described below. A custom struct is defined for each query response.

### `config`

```json
{
  "config": {}
}
```

### `pair`

Gives info for specified assets pair.

```json
{
  "pair": {
    "asset_infos": [
      {
        "token": {
          "contract_address": "terra..."
        }
      },
      {
        "native_token": {
          "denom": "uusd"
        }
      }
    ]
  }
}
```

### `pairs`

Gives paginated pair infos using specified start_after and limit. Given fields are optional.

```json
{
  "pairs": {
    "start_after": [
      {
        "token": {
          "contract_address": "terra..."
        }
      },
      {
        "native_token": {
          "denom": "uusd"
        }
      }
    ],
    "limit": 10
  }
}
```

### `fee_info`

Gives fees for specified pair type.

```json
{
  "pair_type": {
    "xyk": {}
  }
}
```
