# Astroport DCA Module

[![codecov](https://codecov.io/gh/astroport-fi/astroport-dca/branch/main/graph/badge.svg?token=WDA8WEI7MI)](https://codecov.io/gh/astroport-fi/astroport-dca)

This repo contains Astroport DCA related contracts.

## Contracts

| Name                   | Description                       |
| ---------------------- | --------------------------------- |
| [`dca`](contracts/dca) | The Astroport DCA module contract |

## Building Contracts

You will need Rust 1.58.1+ with `wasm32-unknown-unknown` target installed.

You can run unit tests for each contract directory via:

```
cargo test
```

#### For a production-ready (compressed) build:

Run the following from the repository root

```
./scripts/build_release.sh
```

The optimized contracts are generated in the artifacts/ directory.
