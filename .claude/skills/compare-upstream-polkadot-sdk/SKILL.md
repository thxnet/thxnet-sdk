---
name: compare-upstream-polkadot-sdk
description: >
  Compare thxnet-sdk against upstream polkadot-sdk at matching version tags.
  Covers 7 dimensions: workspace members/dependencies, runtimes (construct_runtime!
  pallet sets, Config differences, governance model), custom pallets, CI/CD workflows,
  node binaries, migration chains, and livenet deployment operations (11 live chains,
  feature toggles, chain-data ops, deployment matrix).
  Supports incremental comparison (single dimension) or full audit.
  Use when: compare upstream, diff upstream, compare polkadot-sdk, upstream differences,
  fork delta, fork drift, compare with polkadot, thxnet vs polkadot,
  upstream sync, upstream divergence, what changed from upstream,
  THXNET additions, fork inventory, compare runtimes upstream,
  compare workflows upstream, compare dependencies upstream,
  what did we add, what did we change, how does thxnet-sdk differ,
  fork audit, upst, upstream, compare with ../polkadot-sdk,
  migration chain, livenet deployment, feature toggle, chain-data ops,
  which migrations need to run, deployment matrix, rollout plan,
  跟上游比, 跟上游比較, 跟 polkadot-sdk 對比, fork 差異,
  上游同步, 上游偏移, 部署矩陣, 遷移鏈, 功能開關.
---

# Compare Upstream

Compare thxnet-sdk against its upstream polkadot-sdk at a matching version tag.

## Mental Model

```
thxnet-sdk vX.Y.Z = polkadot-sdk polkadot-vX.Y.Z
                   + THXNET. custom business logic
                   + custom runtime/pallet/node behavior
                   + cumulative migrations/fixes/patches/chain-data-ops
                   + livenet operational state (11 chains, feature toggles, deployment matrix)
```

Every difference is exactly one of:

- **ADDITION**: Exists in thxnet-sdk only (new file/crate/pallet)
- **MODIFICATION**: Exists in both but differs (changed config, added pallet, etc.)
- **REMOVAL**: Exists in upstream only (intentionally excluded)
- **IDENTICAL**: Same in both repos

## Phase 0: Ensure Upstream Availability

Before any comparison, verify upstream exists at the correct version.

**Version mapping**: thxnet-sdk `vX.Y.Z` ↔ polkadot-sdk `polkadot-vX.Y.Z`

```bash
THXNET_ROOT="$(pwd)"
UPSTREAM_ROOT="${UPSTREAM_ROOT:-$(dirname "$THXNET_ROOT")/polkadot-sdk}"

# Check upstream exists
if [ ! -d "$UPSTREAM_ROOT/.git" ]; then
  echo "ERROR: $UPSTREAM_ROOT not found or not a git repo"
  echo "Fix: cd $(dirname "$THXNET_ROOT") && git clone https://github.com/paritytech/polkadot-sdk.git && cd polkadot-sdk && git checkout polkadot-vX.Y.Z"
  exit 1
fi

# Check version
git -C "$UPSTREAM_ROOT" describe --tags --exact-match 2>/dev/null || echo "WARNING: upstream not on exact tag"
```

If upstream is missing or at wrong version, guide the user to fix it before proceeding.

## Dimensions

| Dim | Name           | What it answers                                           | Reference                                         |
| --- | -------------- | --------------------------------------------------------- | ------------------------------------------------- |
| 1   | Workspace      | What crates did THXNET. add? What deps differ?            | [dim-workspace.md](references/dim-workspace.md)   |
| 2   | Runtimes       | Which pallets in construct_runtime!, what configs differ? | [dim-runtimes.md](references/dim-runtimes.md)     |
| 3   | Custom Pallets | What pallets are wholly THXNET. originals?                | [dim-pallets.md](references/dim-pallets.md)       |
| 4   | CI/CD          | What CI coverage is missing vs upstream?                  | [dim-workflows.md](references/dim-workflows.md)   |
| 5   | Node Binaries  | How do node entry points differ?                          | [dim-nodes.md](references/dim-nodes.md)           |
| 6   | Migrations     | What migration chain does THXNET. carry?                  | [dim-migrations.md](references/dim-migrations.md) |
| 7   | Livenet Ops    | What's deployed where? Feature toggles? Chain-data ops?   | [dim-livenet.md](references/dim-livenet.md)       |

## Dimension Summaries

### Dim 1: Workspace Members & Dependencies

Extract and compare `[workspace] members` from both `Cargo.toml` files:

```bash
# Extract sorted member lists
grep '^\s*"' "$THXNET_ROOT/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/' | sort > /tmp/thx-members.txt
grep '^\s*"' "$UPSTREAM_ROOT/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/' | sort > /tmp/upst-members.txt

# THXNET. additions
comm -23 /tmp/thx-members.txt /tmp/upst-members.txt

# Upstream-only (removed/excluded by THXNET.)
comm -13 /tmp/thx-members.txt /tmp/upst-members.txt
```

Also compare `[workspace.dependencies]` and `[patch]` sections for version divergence. Read [dim-workspace.md](references/dim-workspace.md) for full procedure.

### Dim 2: Runtimes

Compare construct_runtime! pallet composition:

| THXNET. Runtime                     | Upstream Equivalent         | Notes                   |
| ----------------------------------- | --------------------------- | ----------------------- |
| `thxnet/runtime/thxnet/`            | `polkadot/runtime/westend/` | Mainnet rootchain       |
| `thxnet/runtime/thxnet-testnet/`    | `polkadot/runtime/westend/` | Testnet rootchain       |
| `thxnet/leafchain/runtime/general/` | (no equivalent)             | Parachain, pure THXNET. |

Key structural difference: THXNET. uses **Governance V1** (Democracy + Council + TechnicalCommittee) while upstream uses **OpenGov** (Referenda + ConvictionVoting). Also: THXNET. retains **Sudo** (index 255).

Read [dim-runtimes.md](references/dim-runtimes.md) for pallet index mapping and Config diff procedures.

### Dim 3: Custom Pallets

5 wholly THXNET.-original pallets:

| Pallet                 | Path                                        | RPC | Runtime API | Migrations | Attack Tests |
| ---------------------- | ------------------------------------------- | --- | ----------- | ---------- | ------------ |
| pallet-dao             | `thxnet/pallets/dao/`                       | -   | -           | -          | -            |
| pallet-finality-rescue | `thxnet/pallets/finality-rescue/`           | -   | -           | -          | -            |
| pallet-crowdfunding    | `thxnet/leafchain/pallets/crowdfunding/`    | Yes | Yes         | Yes        | Yes          |
| pallet-rwa             | `thxnet/leafchain/pallets/rwa/`             | Yes | Yes         | Yes        | Yes          |
| pallet-trustless-agent | `thxnet/leafchain/pallets/trustless-agent/` | -   | -           | Yes        | -            |

Read [dim-pallets.md](references/dim-pallets.md) for per-pallet inventory and verification checklist.

### Dim 4: CI/CD Workflows

```bash
# List and compare workflow files
diff <(ls "$THXNET_ROOT/.github/workflows/"*.yml | xargs -I{} basename {} | sort) \
     <(ls "$UPSTREAM_ROOT/.github/workflows/"*.yml | xargs -I{} basename {} | sort)
```

thxnet-sdk has ~8 simplified workflows vs upstream's ~28+. Read [dim-workflows.md](references/dim-workflows.md) for classification and critical coverage gaps.

### Dim 5: Node Binaries

- **Rootchain**: `polkadot/src/main.rs` — same entry point, different runtime linkage
- **Leafchain**: `thxnet/leafchain/node/` — entirely THXNET.-specific, no upstream equivalent

```bash
diff -rq "$THXNET_ROOT/polkadot/src/" "$UPSTREAM_ROOT/polkadot/src/" 2>/dev/null | head -20
```

Read [dim-nodes.md](references/dim-nodes.md) for details.

### Dim 6: Migrations

THXNET. carries ~7 migration entries not present in upstream at the same version. These are critical for the **Chain Integrity Invariant** (genesis-to-tip sync + in-place upgrade).

Categories: ported-from-removed-upstream, custom VersionedMigration wrappers, version stamps, THXNET.-specific fixes.

Read [dim-migrations.md](references/dim-migrations.md) for the full migration inventory with rationale for each entry.

### Dim 7: Livenet Operations

THXNET. operates **11 live chains** across 3 runtimes. Each code change must be reasoned about in terms of:

- Which chains does it affect? (runtime → chain mapping)
- What deployment operations are needed? (WASM upload, Sudo calls, feature toggles)
- What's the rollout order? (testnet → validate → mainnet)
- What chain-data operations must accompany the upgrade?

Read [dim-livenet.md](references/dim-livenet.md) for chain topology, deployment matrix template, and operational procedures.

## Output Format

### Per-Dimension Verdict Table

```markdown
## Dimension N: [Name]

| Item | Verdict                                       | Detail            |
| ---- | --------------------------------------------- | ----------------- |
| ...  | ADDITION / MODIFICATION / REMOVAL / IDENTICAL | Brief explanation |
```

### Full Comparison Dashboard

```markdown
## thxnet-sdk vs polkadot-sdk — Full Comparison

Version: thxnet-sdk vX.Y.Z vs polkadot-sdk polkadot-vX.Y.Z

| Dimension         | Additions  | Modifications | Removals  | Notes                    |
| ----------------- | ---------- | ------------- | --------- | ------------------------ |
| 1. Workspace      | N crates   | N             | N         |                          |
| 2. Runtimes       | N runtimes | N configs     | N         |                          |
| 3. Custom Pallets | N pallets  | N/A           | N/A       |                          |
| 4. CI/CD          | N          | N             | N missing |                          |
| 5. Node Binaries  | N          | N             | N         |                          |
| 6. Migrations     | N entries  | N wrappers    | N         |                          |
| 7. Livenet Ops    | —          | —             | —         | Deployment state summary |
```

### Actionable Items

After the dashboard, list items that need attention:

- Missing upstream CI checks to consider adopting
- Modified upstream files that may need re-syncing on version bump
- Migration chain gaps that could break the Chain Integrity Invariant
- Pending livenet deployments or feature toggles

## Incremental Usage

| User says                                                                   | Dimension(s) to run          |
| --------------------------------------------------------------------------- | ---------------------------- |
| "compare upstream" / "full comparison" / "fork audit"                       | All 7                        |
| "compare dependencies" / "compare workspace" / "compare crates"             | 1                            |
| "compare runtimes" / "compare pallets in runtime" / "compare governance"    | 2                            |
| "list custom pallets" / "THXNET additions" / "what pallets did we add"      | 3                            |
| "compare CI" / "compare workflows" / "CI coverage gap"                      | 4                            |
| "compare nodes" / "compare binaries"                                        | 5                            |
| "compare migrations" / "migration chain" / "migration delta"                | 6                            |
| "livenet status" / "deployment matrix" / "feature toggles" / "rollout plan" | 7                            |
| "what's different from upstream"                                            | All, highlight MODIFICATIONS |
| "what did we add"                                                           | All, highlight ADDITIONS     |
| "what do we need to deploy"                                                 | 6 + 7                        |

## Important Conventions

- **THXNET.** always with trailing period (the brand name)
- **upstream** = `../polkadot-sdk/` at the corresponding version tag
- **Chain Integrity Invariant**: genesis-to-tip sync + in-place upgrade must both work
- **Rollout order**: testnet first → validate → mainnet (never skip testnet)
- Dim 7 data is point-in-time — always verify against actual chain state before acting on it
