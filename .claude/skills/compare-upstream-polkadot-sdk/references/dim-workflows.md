<!-- Generated for thxnet-sdk v1.12.0 vs polkadot-sdk polkadot-v1.12.0. Re-run comparison when upgrading. -->

# Dimension 4: CI/CD Workflows

## thxnet-sdk Workflows

| File | Purpose |
|------|---------|
| `ci.yml` | Main CI: build, test, Docker image push, try-runtime checks |
| `release.yml` | Release automation: WASM build, Docker images, GitHub release |
| `tests.yml` | Test suite execution |
| `fmt-check.yml` | Rust formatting check (`cargo fmt`) |
| `check-links.yml` | Documentation link validation |
| `check-semver.yml` | Semver compatibility check |
| `check-features.yml` | Feature flag validation |
| `rust.yml` | Rust toolchain / clippy checks |

## Comparison with Upstream

```bash
THXNET_ROOT="$(pwd)"
UPSTREAM_ROOT="$(dirname "$THXNET_ROOT")/polkadot-sdk"

echo "--- thxnet-sdk workflows ---"
ls "$THXNET_ROOT/.github/workflows/"*.yml 2>/dev/null | xargs -I{} basename {} | sort

echo ""
echo "--- polkadot-sdk workflows ---"
ls "$UPSTREAM_ROOT/.github/workflows/"*.yml 2>/dev/null | xargs -I{} basename {} | sort
```

## Classification

| Upstream Workflow | thxnet-sdk Equivalent | Status |
|------------------|--------------------|--------|
| `tests-linux-stable.yml` | `tests.yml` (partial) | EQUIVALENT |
| `tests.yml` | `tests.yml` (partial) | EQUIVALENT |
| `check-features.yml` | `check-features.yml` | EQUIVALENT |
| `check-links.yml` | `check-links.yml` | EQUIVALENT |
| `check-semver.yml` | `check-semver.yml` | EQUIVALENT |
| `checks-quick.yml` | `fmt-check.yml` (partial) | EQUIVALENT |
| `check-labels.yml` | — | MISSING |
| `check-licenses.yml` | — | MISSING |
| `check-prdoc.yml` | — | MISSING |
| `check-runtime-migration.yml` | `ci.yml` (try-runtime jobs) | EQUIVALENT (different approach) |
| `gitspiegel-trigger.yml` | — | NOT APPLICABLE (Parity internal) |
| `issues-auto-add-parachain.yml` | — | NOT APPLICABLE |
| `issues-auto-label.yml` | — | NOT APPLICABLE |
| `misc-*.yml` | — | NOT APPLICABLE (Parity internal) |
| `publish-check-crates.yml` | — | NOT APPLICABLE (THXNET. doesn't publish to crates.io) |
| `publish-claim-crates.yml` | — | NOT APPLICABLE |
| `release-10_rc-automation.yml` | — | MISSING (RC automation) |
| `release-30_publish_release_draft.yml` | `release.yml` | EQUIVALENT |
| `release-50_publish-docker.yml` | `release.yml` (Docker jobs) | EQUIVALENT |
| `review-*.yml` | — | NOT APPLICABLE (Parity code review bots) |

## Critical Coverage Gaps

### Should Consider Adopting

| Upstream Check | Risk of Not Having | Priority |
|---------------|-------------------|----------|
| `check-runtime-migration.yml` | Migration failures on upgrade | **Already covered** in `ci.yml` try-runtime jobs |
| `check-licenses.yml` | License compliance issues | Medium |
| `check-labels.yml` | PR hygiene | Low |
| `check-prdoc.yml` | Missing PR documentation | Low |

### THXNET.-Specific (Not in Upstream)

| thxnet-sdk Check | Purpose |
|-----------------|---------|
| `ci.yml` try-runtime jobs | Tests 6 live chains against new runtime |
| `ci.yml` Docker image builds | Builds rootchain + leafchain Docker images |
| `release.yml` subwasm metadata | Generates runtime metadata JSON |

## How to Compare Shared Workflows

For workflows that exist in both repos (e.g., `check-features.yml`), diff them to understand THXNET.-specific modifications:

```bash
diff "$THXNET_ROOT/.github/workflows/check-features.yml" \
     "$UPSTREAM_ROOT/.github/workflows/check-features.yml" | head -40
```

Most shared workflows will be MODIFIED because THXNET. changes branch triggers, runner labels, and sometimes job definitions.
