<!-- Generated for thxnet-sdk v1.12.0 vs polkadot-sdk polkadot-v1.12.0. Re-run comparison when upgrading. -->

# Dimension 1: Workspace Members & Dependencies

## What to Compare

### Workspace Members

The `[workspace] members` array in root `Cargo.toml` lists every crate in the monorepo.

**Extraction and diff**:

```bash
THXNET_ROOT="$(pwd)"
UPSTREAM_ROOT="$(dirname "$THXNET_ROOT")/polkadot-sdk"

# Extract sorted member lists (matches lines like "path/to/crate",)
grep '^\s*"' "$THXNET_ROOT/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/' | sort > /tmp/thx-members.txt
grep '^\s*"' "$UPSTREAM_ROOT/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/' | sort > /tmp/upst-members.txt

echo "=== thxnet-sdk: $(wc -l < /tmp/thx-members.txt) members ==="
echo "=== polkadot-sdk: $(wc -l < /tmp/upst-members.txt) members ==="

echo ""
echo "--- THXNET. Additions ---"
comm -23 /tmp/thx-members.txt /tmp/upst-members.txt

echo ""
echo "--- Upstream-only (removed/excluded) ---"
comm -13 /tmp/thx-members.txt /tmp/upst-members.txt

echo ""
echo "--- Shared: $(comm -12 /tmp/thx-members.txt /tmp/upst-members.txt | wc -l) members ---"
```

### Expected THXNET. Additions

All THXNET.-specific members live under the `thxnet/` prefix:

| Category | Members |
|----------|---------|
| Rootchain runtimes | `thxnet/runtime/thxnet`, `thxnet/runtime/thxnet-testnet` |
| Runtime constants | `thxnet/runtime/thxnet/constants`, `thxnet/runtime/thxnet-testnet/constants` |
| Rootchain pallets | `thxnet/pallets/dao`, `thxnet/pallets/finality-rescue` |
| Leafchain node | `thxnet/leafchain/node` |
| Leafchain runtime | `thxnet/leafchain/runtime/general` |
| Leafchain pallets | `thxnet/leafchain/pallets/crowdfunding`, `crowdfunding/rpc`, `crowdfunding/runtime-api` |
| | `thxnet/leafchain/pallets/rwa`, `rwa/rpc`, `rwa/runtime-api` |
| | `thxnet/leafchain/pallets/trustless-agent` |
| Leafchain tests | `thxnet/leafchain/integration-tests/xcm` |
| Leafchain XCM | `thxnet/leafchain/xcm/xcm-emulator` |

If new members appear that are NOT under `thxnet/`, investigate — they may be upstream modifications.

### Dependency Overrides

Compare `[workspace.dependencies]` sections for version divergence:

```bash
# Extract dependency sections and diff
sed -n '/\[workspace.dependencies\]/,/^\[/p' "$THXNET_ROOT/Cargo.toml" | sort > /tmp/thx-deps.txt
sed -n '/\[workspace.dependencies\]/,/^\[/p' "$UPSTREAM_ROOT/Cargo.toml" | sort > /tmp/upst-deps.txt
diff /tmp/thx-deps.txt /tmp/upst-deps.txt | head -50
```

Also check for `[patch.crates-io]` or `[patch]` sections — THXNET. may pin specific crate versions.

### Spotting Modified Upstream Crates

For shared workspace members, check if THXNET. modified any upstream crate files:

```bash
# Example: check if staking pallet was modified
diff -rq "$THXNET_ROOT/substrate/frame/staking/" "$UPSTREAM_ROOT/substrate/frame/staking/" 2>/dev/null | head -20
```

Focus on pallets that THXNET. configures with custom parameters or that have migration wrappers (staking, nomination-pools, grandpa, bounties, parachains).

## Output Format

```markdown
| Crate Path | Category | Verdict |
|-----------|----------|---------|
| thxnet/pallets/dao | Rootchain pallet | ADDITION |
| thxnet/leafchain/pallets/rwa | Leafchain pallet | ADDITION |
| substrate/frame/staking | Upstream pallet | MODIFICATION (migration wrapper) |
| ... | ... | ... |
```
