<!-- Generated for thxnet-sdk v1.12.0 vs polkadot-sdk polkadot-v1.12.0. Re-run comparison when upgrading. -->

# Dimension 5: Node Binaries

## Binary Inventory

| Binary | thxnet-sdk Path | Upstream Path | Verdict |
|--------|----------------|--------------|---------|
| Rootchain node | `polkadot/src/main.rs` | `polkadot/src/main.rs` | IDENTICAL entry point, different runtime linkage |
| Leafchain node | `thxnet/leafchain/node/` | (none) | ADDITION — no upstream equivalent |

## Rootchain Node

The rootchain binary entry point (`polkadot/src/main.rs`) is shared with upstream. The difference is in **what runtimes it links against**, controlled by `Cargo.toml` dependencies and feature flags.

```bash
THXNET_ROOT="$(pwd)"
UPSTREAM_ROOT="$(dirname "$THXNET_ROOT")/polkadot-sdk"

# Diff the polkadot binary source
diff -rq "$THXNET_ROOT/polkadot/src/" "$UPSTREAM_ROOT/polkadot/src/" 2>/dev/null

# If files differ, inspect the diff
diff -u "$THXNET_ROOT/polkadot/src/main.rs" "$UPSTREAM_ROOT/polkadot/src/main.rs"
```

Key areas where THXNET. modifies the node:
- `polkadot/node/service/` — runtime linkage, chain spec loading
- `polkadot/node/service/Cargo.toml` — dependencies on THXNET. runtimes
- Chain spec definitions — genesis state for THXNET. networks

## Leafchain Node

`thxnet/leafchain/node/` is a cumulus-based parachain collator node. Key files:

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry point (minimal, delegates to service) |
| `src/service.rs` | Collator service configuration |
| `src/chain_spec.rs` | Chain spec definitions for all leafchains |
| `src/rpc.rs` | RPC endpoint registration (crowdfunding, RWA custom RPCs) |
| `src/command.rs` | CLI command handling |
| `Cargo.toml` | Dependencies on general-runtime and custom pallets |

This is a pure ADDITION — document as such. No upstream comparison needed.

## Chain Specs

Chain specs define genesis state and are critical for the Chain Integrity Invariant. Check:

```bash
# Find chain spec files or generators
rg 'fn chain_spec\|ChainSpec\|chain_spec_' thxnet/leafchain/node/src/ --files-with-matches
rg 'fn chain_spec\|ChainSpec' polkadot/node/service/src/ --files-with-matches
```

Each THXNET. chain (11 total) has its own chain spec. These embed genesis state, bootnodes, and telemetry endpoints.

## Docker Images

THXNET. produces two Docker images (via `release.yml`):

| Image | Binary | Source |
|-------|--------|--------|
| `ghcr.io/thxnet/rootchain` | polkadot binary | `thxnet/docker/rootchain/` |
| `ghcr.io/thxnet/leafchain` | leafchain binary | `thxnet/docker/leafchain/` |

Upstream produces its own Docker images with different naming and structure.
