# THXNET. SDK

Fork of [polkadot-sdk](https://github.com/paritytech/polkadot-sdk) powering the THXNET. blockchain network: 2 rootchains (relay chains) and 9+ leafchains (parachains) across testnet and mainnet environments.

---

## Git Operations

### Branches

| Branch           | Lifetime  | Purpose                                                                          |
| ---------------- | --------- | -------------------------------------------------------------------------------- |
| `main`           | permanent | Production — reflects code that has been **released and deployed** to all chains |
| `develop`        | permanent | Integration — accumulates features and fixes before release                      |
| `release/vX.Y.Z` | temporary | Stabilisation, testing, and deployment of a specific version                     |
| `upgrade/vX.Y.Z` | temporary | Merging an upstream polkadot-sdk release into THXNET.                            |
| `feat/*`         | temporary | Feature development, branched from and merged back to `develop`                  |
| `fix/*`          | temporary | Bug fixes or hotfixes                                                            |
| `experiment/*`   | temporary | Exploratory work that may or may not land                                        |
| `upstream/main`  | permanent | Mirror of upstream polkadot-sdk (do not commit directly)                         |

### Day-to-day development

```
feat/my-feature ──PR──> develop
                          |
                     (accumulate features)
                          |
                     develop ──PR──> main  (after release + deploy)
```

- Branch `feat/*` from `develop`, open PR back to `develop`.
- Never push directly to `main`. All changes reach `main` through a release cycle.
- `develop` accepts direct pushes for small fixes and integration work.

### Upstream upgrades

```
git fetch upstream
git checkout -b upgrade/v1.13.0 main
git merge upstream/polkadot-v1.13.0
# resolve conflicts, adapt THXNET. code, fix migration gaps
# PR to develop or directly to the release branch
```

`upgrade/*` branches are short-lived. Once the upgrade is merged, archive as a tag if desired and delete the branch.

### Release process

Releases have three phases. **Do not skip or reorder them.**

#### Phase 1 — Stabilise

```
git checkout -b release/v1.13.0 develop
```

On the release branch, the full CI suite must pass:

- try-runtime migration tests against all live chains
- Idempotency tests (run migrations twice, verify no change)
- Pallet migration matrix (per-pallet isolation tests)
- Zombienet smoke tests
- Chopsticks upgrade tests
- XCM integration tests

Fix issues directly on the release branch or via `fix/* -> release/*` PRs.

#### Phase 2 — Tag and build artifacts

```
git tag thxnet-v1.13.0 release/v1.13.0
git push origin thxnet-v1.13.0
```

The tag push triggers the **release workflow**, which:

1. Builds deterministic runtime WASMs (rootchain, rootchain-testnet, leafchain)
2. Generates `subwasm` metadata and SHA-256 checksums
3. Builds and pushes Docker images (rootchain + leafchain) to ghcr.io
4. Publishes a GitHub Release with all artifacts

#### Phase 3 — Deploy, then merge

Deploy in strict order:

1. **Testnet** — upgrade node binaries (Docker), then submit runtime upgrade governance proposal
2. **Observe** — wait at least one full epoch; confirm no errors, no stalls
3. **Mainnet** — repeat the above for each mainnet chain

After **all chains** are upgraded and stable:

```
# Merge release back to main (main now reflects deployed state)
PR: release/v1.13.0 -> main

# Sync any release-phase fixes back to develop
PR: release/v1.13.0 -> develop  (or main -> develop)

# Clean up
git branch -d release/v1.13.0
git push origin --delete release/v1.13.0
```

### Hotfixes

For critical issues discovered after release:

```
git checkout -b fix/critical-bug main
# fix the bug
git tag thxnet-v1.13.1 fix/critical-bug
git push origin thxnet-v1.13.1        # triggers release workflow

# After deployment
PR: fix/critical-bug -> main
PR: fix/critical-bug -> develop        # keep develop in sync
```

### Tag conventions

| Pattern              | Example                   | Purpose                                        |
| -------------------- | ------------------------- | ---------------------------------------------- |
| `thxnet-v*`          | `thxnet-v1.13.0`          | Production release (triggers release workflow) |
| `thxnet-v*-rc*`      | `thxnet-v1.13.0-rc1`      | Release candidate (manual testing)             |
| `archive/upgrade-v*` | `archive/upgrade-v1.11.0` | Historical upgrade branch preservation         |

### What not to do

| Action                                                              | Why                                                     |
| ------------------------------------------------------------------- | ------------------------------------------------------- |
| Push directly to `main`                                             | Bypasses all CI gates; 11 live chains depend on this    |
| Tag on `main` before deploying                                      | `main` should reflect deployed state, not pending state |
| Merge release to `main` before all chains are upgraded              | Creates ambiguity about what is actually in production  |
| Upgrade testnet and mainnet simultaneously                          | Testnet is your canary; let it run first                |
| Delete a release branch before merging to both `main` and `develop` | Loses release-phase hotfixes                            |
| Use `v*` tags (without `thxnet-` prefix)                            | Conflicts with upstream polkadot-sdk tag namespace      |

---

## CI Workflows

| Workflow               | Trigger                                                              | What it does                                                |
| ---------------------- | -------------------------------------------------------------------- | ----------------------------------------------------------- |
| **ci.yml**             | Push to `main`/`develop`/`release/*`/`upgrade/*`/`experiment/*`, PRs | Build binaries, Docker images, WASM OCI push, upgrade tests |
| **fmt-check.yml**      | Same branches + PRs                                                  | `cargo fmt`, `taplo` (TOML), `zepter` (feature propagation) |
| **rust.yml**           | Same branches + PRs                                                  | `cargo check` for all THXNET. crates                        |
| **tests.yml**          | Same branches + PRs                                                  | `cargo nextest`, benchmark compilation, syscall validation  |
| **check-features.yml** | Push to active branches + PRs                                        | `cargo-featalign` per-runtime feature alignment             |
| **check-links.yml**    | Push/PR (path-filtered: `*.rs`, `*.md`, `*.toml`)                    | Lychee dead-link checker                                    |
| **check-semver.yml**   | Push to active branches + PRs                                        | `cargo-semver-checks` API compatibility against `main`      |
| **release.yml**        | Tag `thxnet-v*` or manual dispatch                                   | Build WASMs, Docker images, publish GitHub Release          |

### Upgrade tests (conditional)

The following CI jobs only run on upgrade-related branches (`upgrade/*`, `release/*`, `experiment/*`, `feat/endgame*`), pushes to `main`/`develop`, or PRs targeting `main`:

- **build-upgrade-extras** — try-runtime WASMs + fast-runtime binary
- **try-runtime** — migration tests, idempotency, pallet matrix
- **zombienet** — network smoke test with fast-runtime
- **chopsticks** — fork-based upgrade simulation
- **xcm-tests** — cross-chain message integration tests

Regular `feat/*` PRs to `develop` run build + basic tests only, keeping CI fast for day-to-day work.

---

## Upstream Polkadot SDK

> This repository is a fork of polkadot-sdk. The original README follows below.

# Polkadot SDK

![](https://cms.polkadot.network/content/images/2021/06/1-xPcVR_fkITd0ssKBvJ3GMw.png)

[![StackExchange](https://img.shields.io/badge/StackExchange-Community%20&%20Support-222222?logo=stackexchange)](https://substrate.stackexchange.com/)

The Polkadot SDK repository provides all the resources needed to start building on the Polkadot network, a multi-chain
blockchain platform that enables different blockchains to interoperate and share information in a secure and scalable
way. The Polkadot SDK comprises three main pieces of software:

## [Polkadot](./polkadot/)

[![PolkadotForum](https://img.shields.io/badge/Polkadot_Forum-e6007a?logo=polkadot)](https://forum.polkadot.network/)
[![Polkadot-license](https://img.shields.io/badge/License-GPL3-blue)](./polkadot/LICENSE)

Implementation of a node for the https://polkadot.network in Rust, using the Substrate framework. This directory
currently contains runtimes for the Westend and Rococo test networks. Polkadot, Kusama and their system chain runtimes
are located in the [`runtimes`](https://github.com/polkadot-fellows/runtimes/) repository maintained by
[the Polkadot Technical Fellowship](https://polkadot-fellows.github.io/dashboard/#/overview).

## [Substrate](./substrate/)

[![SubstrateRustDocs](https://img.shields.io/badge/Rust_Docs-Substrate-24CC85?logo=rust)](https://paritytech.github.io/polkadot-sdk/master/polkadot_sdk_docs/polkadot_sdk/substrate/index.html)
[![Substrate-license](https://img.shields.io/badge/License-GPL3%2FApache2.0-blue)](./substrate/README.md#LICENSE)

Substrate is the primary blockchain SDK used by developers to create the parachains that make up the Polkadot network.
Additionally, it allows for the development of self-sovereign blockchains that operate completely independently of
Polkadot.

## [Cumulus](./cumulus/)

[![CumulusRustDocs](https://img.shields.io/badge/Rust_Docs-Cumulus-222222?logo=rust)](https://paritytech.github.io/polkadot-sdk/master/polkadot_sdk_docs/polkadot_sdk/cumulus/index.html)
[![Cumulus-license](https://img.shields.io/badge/License-GPL3-blue)](./cumulus/LICENSE)

Cumulus is a set of tools for writing Substrate-based Polkadot parachains.

## Releases

> [!NOTE]  
> Our release process is still Work-In-Progress and may not yet reflect the aspired outline here.

The Polkadot-SDK has two release channels: `stable` and `nightly`. Production software is advised to only use `stable`.
`nightly` is meant for tinkerers to try out the latest features. The detailed release process is described in
[RELEASE.md](docs/RELEASE.md).

### Stable

`stable` releases have a support duration of **three months**. In this period, the release will not have any breaking
changes. It will receive bug fixes, security fixes, performance fixes and new non-breaking features on a **two week**
cadence.

### Nightly

`nightly` releases are released every night from the `master` branch, potentially with breaking changes. They have
pre-release version numbers in the format `major.0.0-nightlyYYMMDD`.

## Upstream Dependencies

Below are the primary upstream dependencies utilized in this project:

- [`parity-scale-codec`](https://crates.io/crates/parity-scale-codec)
- [`parity-db`](https://crates.io/crates/parity-db)
- [`parity-common`](https://github.com/paritytech/parity-common)
- [`trie`](https://github.com/paritytech/trie)

## Security

The security policy and procedures can be found in [docs/contributor/SECURITY.md](./docs/contributor/SECURITY.md).

## Contributing & Code of Conduct

Ensure you follow our [contribution guidelines](./docs/contributor/CONTRIBUTING.md). In every interaction and
contribution, this project adheres to the [Contributor Covenant Code of Conduct](./docs/contributor/CODE_OF_CONDUCT.md).

## Additional Resources

- For monitoring upcoming changes and current proposals related to the technical implementation of the Polkadot network,
  visit the [`Requests for Comment (RFC)`](https://github.com/polkadot-fellows/RFCs) repository. While it's maintained
  by the Polkadot Fellowship, the RFC process welcomes contributions from everyone.
