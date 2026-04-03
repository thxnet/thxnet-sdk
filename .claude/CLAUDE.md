# Git Workflow

- **Never** commit or push without explicit confirmation. Present changes for review first; await instructions before proceeding.

# Terminology

- **"upstream" / "upst"** — refers to `@../polkadot-sdk/` (if it does not exist locally, fetch from remote with corresponding version tag) at the corresponding version. Compare differences between the two repositories, scoped to whatever the prompt specifies.

# Brand Name

The project name is **THXNET.** (with a trailing full stop/period). All occurrences in string values, comments, documentation, and commit messages must use `THXNET.` — never `THXNET`, `THXNet`, `Thxnet`, or any other variation without the trailing period. Exception: when `THXNET.` ends a sentence, write `THXNET.` (single period), not `THXNET..`.

# Chain Integrity Invariant

Every runtime and node binary — across mainnet rootchain, testnet rootchain, and all mainnet/testnet leafchains — must carry **all cumulative migrations, fixes, patches, and special chain-data operations**. Two properties must hold at all times:

1. **Genesis-to-tip sync** — A freshly started node binary must sync from genesis to the latest block without manual intervention, with all features working as expected.
2. **In-place upgrade** — An existing (older-version) node binary must upgrade to the latest version via the normal process (new binary with existing chain data, session keys, and node keys; on-chain runtime upgrade) without manual intervention, with all features working as expected afterwards.

# Quality Gates

## `release/*` branches

Before pushing to any `release/*` branch, every check required by the Chain Integrity Invariant above must be scrutinised and passing at 100%. The CI/CD pipeline must be green.

## PRs into `main`

A PR into `main` signifies that all node binaries and runtimes in the PR can sync from genesis to the latest block without manual intervention, all features work as expected, and all upgrades have **already been applied** to livenet (mainnet and testnet).

## Post-deployment merge-back

Once all chains are upgraded and stable:

1. `release/vX.Y.Z` → `main` — main reflects the deployed state.
2. `release/vX.Y.Z` → `develop` (or `main` → `develop`) — sync any release-phase fixes back.
3. Delete the `release/vX.Y.Z` branch.

# Development Methodology

Write one function, then immediately write behavioural tests for that function and verify it works as expected. Only after that, move on to the next function. Repeat until all functions are covered.
