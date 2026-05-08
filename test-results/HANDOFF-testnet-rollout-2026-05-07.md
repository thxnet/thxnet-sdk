# THXNET Testnet Rollout Handoff — `release/v1.12.0`

**Document date**: 2026-05-07
**Target network**: THXNET testnet (rootchain + 5 leafchains)
**Subject release**: `release/v1.12.0` HEAD `6b7ee05aea` (PR #37 merge tip)
**Handoff intent**: This document is self-contained. Operators should be able to execute the rollout without further consultation with the engineering team that produced the release.

> **zh-tw 摘要**: 本次升級交付 `release/v1.12.0`，核心修正是 PR #37 的 unified `EnableAsyncBackingAndCoretime` migration —— 它把 `node_features[3]=true` 設定到 relay-side scheduler config，這是避免 v1.12.0 升級後 leafchain 因 `BlockedByBacking` 而凍結的關鍵。場景 A 為最小可上線版本（async backing 已啟用，para 出塊 12-18s）；場景 B 為加掛 PR #38 + #39 之後的加速版本（para 出塊穩定 ~6s）。後續所有營運說明採英文撰寫。

---

## 1. Executive summary

### Nature of this upgrade — IN-PLACE LIVENET RUNTIME UPGRADE, NO RESET

This rollout is an **in-place runtime upgrade against live testnet chain data**. It is **NOT** a chain re-genesis, a forknet snapshot replay, or a "minifork" deploy. Specifically:

- **Block history is preserved.** Every existing testnet rootchain block (current head ≈ #15.9M as of 2026-05-07) and every leafchain block remain in the canonical chain. Finalised blocks stay finalised. Block numbers continue monotonically across the upgrade window — the chain does NOT restart from #0.
- **Account state is preserved.** Balances, sudo, treasury, accounts pallet, staking ledger, all storage items remain byte-identical across the runtime swap. The `setCodeWithoutChecks` extrinsic only replaces the WASM under storage key `:code`; everything else is untouched aside from what `MigrationsLate::on_runtime_upgrade()` deliberately rewrites (which for PR #37 is exclusively `parachains_configuration::ActiveConfig` + `parachains_scheduler::AvailabilityCores` + `parachains_scheduler::ClaimQueue`).
- **Finality continues.** GRANDPA finality keeps advancing throughout the rollout window. There is no "reset to genesis" or "rebuild from snapshot" step.
- **The forknet rehearsal evidence in `REPORT-rehearsal-v5-2026-05-07.md` is a faithful simulation of this in-place mechanic** (boot v1.12.0 binary on v0.9.x livenet-shaped state → `setCodeWithoutChecks` → migration fires → chain continues), not a reset / fresh-genesis exercise.

If at any point during execution you find yourself looking at a chain with `chain_getHeader` returning a block number close to 0, or `chain_getFinalizedHead` not advancing past genesis, **STOP** — you are pointed at a forknet, not livenet. Re-verify endpoint URLs against §5 before proceeding.

### What is shipping

| Scenario | Artefacts | Outcome |
|---|---|---|
| **A — minimum safe rollout** | `release/v1.12.0` polkadot binary + `thxnet_testnet_runtime.compact.compressed.wasm` (spec `112_000_005`) + `general_runtime.compact.compressed.wasm` (spec `21`, `UNINCLUDED_SEGMENT_CAPACITY=1`) | Async backing primitives written to relay storage; 5 leafchains stay live; para block rate **12–18s/block** stable |
| **B — A + speedup follow-up** | Scenario A artefacts plus PR #38 polkadot binary (cherry-pick of polkadot-sdk #4937 + 6-val `dev_authority_set`) + PR #39 `general_runtime` (spec `22`, `UNINCLUDED_SEGMENT_CAPACITY=2`) | Para block rate engages at **~6s/block sustained** (forknet rehearsal observed 27/29 = 93% gaps at 6s) |

### Why ship

Pre-v1.12.0, the testnet relay scheduler config has `node_features[3]=false`. v1.12.0 cumulus collators advertise `CandidateReceiptV2`; the relay rejects them with `BlockedByBacking`, freezing all 5 testnet leafchains post-upgrade. **PR #37's unified `EnableAsyncBackingAndCoretime` migration sets `node_features[3]=true` atomically with the runtime swap; this is the critical unfreeze fix.**

### Success criteria (Scenario A — minimum)

- All 19 testnet relay validators run the v1.12.0 polkadot binary
- Testnet rootchain `state_getRuntimeVersion` reports `specVersion=112000005`
- Migration log line `EnableAsyncBackingAndCoretime: ... node_features[0,1,3]=true ...` present in at least one relay validator log
- All 5 leafchains have `parachainSystem.ValidationFunctionStored` event after their cumulus 2-step setCode
- All 5 leafchains continue importing blocks (no `BlockedByBacking` symptoms; gap ≤ 30s)
- 24-hour soak: no degradation in finality lag (≤ 3 blocks)

### Success criteria (Scenario B — speedup, on top of A)

- All testnet validators + collators run the PR #38 binary
- All 5 leafchain runtimes at spec_version `22`
- Sustained 6s/para-block observed for ≥ 100 consecutive blocks per leafchain post-upgrade

### Estimated time budget

| Scenario | Phase 1 | Phase 2 | Phase 2.1 | Phase 3 | Phase 4 | Phase 5 | Total |
|---|---|---|---|---|---|---|---|
| A | ~2 h (rolling, 19 vals × ~5 min) | ~5 min | ~10 min | ~30 min (5 leafchains, parallelisable) | ~50 min (5 × ~10 min, serial) | n/a | **~3.5 h active + 24 h soak** |
| B | ~2 h (PR #38 binary on relay validators) | ~5 min | ~10 min | ~30 min (PR #38 binary on collators, 5 leafchains, parallelisable) | ~50 min (5 × ~10 min using PR #39 wasm) | ~40 min (observe 5 leafchains × 6s × 100 blocks ≈ 10 min each, serial) | **~4.5 h active + 24 h soak** |

### Personnel on standby

- **Sudo key holder** — must be online for Phase 2 + Phase 4 (or pre-sign + queue if cold-storage)
- **Two cluster operators** with `kubectl` access to the testnet validator + collator deployments
- **One on-call SRE** to monitor finality lag dashboards
- **Engineering escalation** — at least one engineer familiar with this PR set on a hot pager

---

## 2. Pre-flight checklist

Run **all** of these immediately before opening the rollout window. Any single failed line aborts the rollout.

### 2.1 Build artefacts

- [ ] CI green for `release/v1.12.0` HEAD `6b7ee05aea` — `gh run list --branch release/v1.12.0 --limit 5 --workflow ci.yml`. Semver-noise jobs are ignored per `project_pr37_async_backing_unified.md`.
- [ ] Polkadot binary tarball downloaded; `sha256sum` verified; `./polkadot --version` reports `1.12.0-<git>-x86_64-linux-gnu`. Original CI run is `25221252584`; the post-PR-#37 successor run is `[OPERATOR FILL IN: post-merge CI run ID for 6b7ee05aea]`.
- [ ] `thxnet_testnet_runtime.compact.compressed.wasm` downloaded; `b2sum -l 256` matches `[OPERATOR FILL IN: expected blake2-256]`.
- [ ] `general_runtime.compact.compressed.wasm` (spec 21 Scenario A; spec 22 Scenario B) — `b2sum -l 256` matches `[OPERATOR FILL IN]`.
- [ ] try-runtime feature WASMs (`*-try-runtime.wasm`) staged for §2.2 re-verification.
- [ ] **Scenario B only**: PR #38 polkadot binary tarball + PR #39 general_runtime WASM (spec 22).

### 2.2 Live re-verification (within 24 hours of rollout window opening)

- [ ] try-runtime `on-runtime-upgrade live` × testnet rootchain — PASS, exit 0
- [ ] try-runtime `on-runtime-upgrade live` × each of the 5 leafchains — PASS, exit 0
- [ ] Chopsticks fork + setCode + advance ≥ 3 blocks × all 6 chains — PASS

```bash
TRY=/mnt/HC_Volume_105402799/tools/try-runtime-cli-git/bin/try-runtime
cd /mnt/HC_Volume_105402799/worktrees/thxnet-rehearsal   # CRITICAL: relative path; try-runtime lowercases absolute paths

# Rootchain (must show migration log: active_validators=19, num_cores=5,
# max_vals_per_core=Some(5), node_features[0,1,3]=true; 60 try-state PASS)
"$TRY" \
  --runtime target/release/wbuild/thxnet-testnet-runtime/thxnet_testnet_runtime.compact.compressed.wasm \
  on-runtime-upgrade --blocktime 6000 --checks=all \
  live --uri wss://node.testnet.thxnet.org/archive-001/ws

# Leafchains (all 5 must exit 0). Endpoints follow the canonical
# `wss://node.<leaf>.testnet.thxnet.org/<archive>/ws` form.
# ecq archive segment is unresolved (see §5 caveat) — set ECQ_ARCHIVE
# explicitly after the operator confirms archive-001 vs archive-002 against
# the live endpoint; the other four leafchains use archive-001.
ECQ_ARCHIVE=archive-002   # tentative per reference_paths_and_artefacts.md;
                           # MUST verify against live before running.
for LEAF in sand ecq lmt thx izutsuya; do
  ARCH=archive-001
  [ "$LEAF" = "ecq" ] && ARCH="$ECQ_ARCHIVE"
  "$TRY" \
    --runtime target/release/wbuild/general-runtime/general_runtime.compact.compressed.wasm \
    on-runtime-upgrade --blocktime 12000 --checks=all \
    live --uri "wss://node.${LEAF}.testnet.thxnet.org/${ARCH}/ws"
done

# Chopsticks (run each in its own shell + run setCode + advance per chain).
# Config filenames follow `rootchain-testnet.yml` for the relay and
# `leafchain-<leaf>-testnet.yml` for each parachain.
cd /mnt/HC_Volume_105402799/worktrees/thxnet-rehearsal
bun install && ( cd node_modules/sqlite3 && npx node-gyp rebuild )   # sqlite native binding

# Relay (rootchain) fork
bunx @acala-network/chopsticks --config scripts/chopsticks/rootchain-testnet.yml

# Each leafchain fork
for LEAF in sand ecq lmt thx izutsuya; do
  bunx @acala-network/chopsticks --config "scripts/chopsticks/leafchain-${LEAF}-testnet.yml"
done
```

- [ ] All re-verification runs timestamped within last 24 hours. Re-run any that are stale.

### 2.3 Operational readiness

- [ ] Sudo key holder confirmed online for the rollout window
- [ ] Two cluster operators present (one driver, one observer)
- [ ] **Kubernetes namespace confirmed** for the testnet relay + collator deployments. Every `kubectl` command in this handoff is written in unqualified form (no `-n <ns>`); operators MUST prefix each invocation with `-n <namespace placeholder>` (e.g. `kubectl -n thxnet-testnet ...`) or set `kubectl config set-context --current --namespace=<ns>` once at session start. Confirm the namespace name with the cluster owner and write it on the rollout-channel pinned message.
- [ ] On-call SRE paged in
- [ ] Stakeholder communication sent (T-2h announcement); see Section 10
- [ ] Rollback artefacts staged on the deployment host
  - [ ] Previous polkadot binary tarball (the build currently in production — `[OPERATOR FILL IN: actual production binary version, e.g. release/v0.9.40 or whatever is live]`) — `[OPERATOR FILL IN: path on deploy host]`
  - [ ] Previous `thxnet_testnet_runtime.compact.compressed.wasm` (whatever spec_version is currently live, expected `94000004`)
  - [ ] Previous `general_runtime` WASM per leafchain (current spec, expected `4`–`20` range)
- [ ] PagerDuty / chat channel for the rollout window pinned and named (e.g., `#thxnet-testnet-v1.12-rollout`)
- [ ] **Scenario B only**: PR #38 + #39 CI green and merged into a release branch with its own deployable head

### 2.4 Rollout window

- [ ] Rollout time confirmed off-peak for testnet usage (low parachain user activity)
- [ ] No conflicting deployments scheduled (no other validator restarts, no chain-spec changes)
- [ ] Dashboards open in operator's browser (validator finality gap, per-leafchain block height, per-chain spec_version)

---

## 3. Rollout sequence — Scenario A (minimum: just `release/v1.12.0`)

> **General rule throughout this section**: every operator action must be acknowledged by at least one observer who confirms the expected signal before the next action proceeds. Treat this as paired-commit operations.

### Phase 1 — Binary swap on relay validators

**Goal**: Replace the `polkadot` binary on all 19 testnet relay validators with the v1.12.0 build. The new binary executes the OLD runtime via WASM until Phase 2 setCode, so this phase is **passive** — no chain-side effect by design. ETA ~2 h. Operators: cluster (driver) + observer.

- **Pre-conditions**: §2 fully ticked. Validators currently healthy + finalising. **Important**: the four-flag forknet boot rule (`--discover-local --allow-private-ip --public-addr=/ip4/127.0.0.1/...` + static `--node-key`) is a **forknet/rehearsal-only** workaround for `Live`-chainType local boots; it MUST NOT appear in production manifests (production validators already advertise public addresses via Identify and use real authority-discovery DHT records). Confirm the new container image inherits the existing production `args:` block unchanged — do not graft any forknet flags in.
- **Action**: Rolling restart, one validator at a time, with the new binary baked into a new container image. Wait for each to resync to head + finalise the latest known block before the next.
- **Per-validator commands**:
  ```bash
  kubectl -n <namespace placeholder> set image deploy/validator-N polkadot=<registry>/polkadot:1.12.0-<sha>
  kubectl -n <namespace placeholder> rollout status deploy/validator-N --timeout=300s
  ```
- **Expected signals**: Pod cycles Running with new image hash; startup log shows `1.12.0`; resync to head within 60 s; `system_health` reports `peers >= 18`; `state_getRuntimeVersion.specVersion` still `94000004` (old runtime running via WASM execution).
- **Pass**: All 19 on new binary; finality gap ≤ 3 blocks; no panic/fatal lines.
- **Fail**: Validator unhealthy within 5 min, OR finality gap > 10 blocks, OR crash loop.
- **Rollback**: `kubectl -n <namespace placeholder> rollout undo deploy/validator-N`. Old binary is wire-compatible with both old runtime and other v1.12.0 binaries (no setCode has happened yet).
- **Parallelism**: With 19 validators, 2-at-a-time is safe (availability margin > 4).

### Phase 2 — Sudo `setCodeWithoutChecks` on testnet rootchain

**Goal**: Apply v1.12.0 testnet rootchain runtime; trigger `EnableAsyncBackingAndCoretime` migration; flip `node_features[3]=true`. ETA ~5 min. Operators: sudo key holder (driver) + observer.

- **Pre-conditions**: Phase 1 PASS; all 19 validators healthy on new binary; sudo holder online; WASM blake2-256 matches §2.1.
- **Action**: `sudo.sudoUncheckedWeight(system.setCodeWithoutChecks(thxnet_testnet_runtime.compact.compressed.wasm))` via the existing helper `polkadot/scripts/forknet/setcode-runtime-upgrade.ts`.
- **Mechanism (read this before signing)**: `setCodeWithoutChecks` is a sudo-gated extrinsic that overwrites storage key `:code` (the active runtime WASM blob). At the next block import, substrate's executive sees `LastRuntimeUpgrade.spec_version (94000004) ≠ Runtime::version().spec_version (112000005)` and invokes `MigrationsLate::on_runtime_upgrade()`. PR #37's `EnableAsyncBackingAndCoretime` migration runs there, **mutating only**: `parachains_configuration::ActiveConfig` (sets async-backing primitives + `node_features[0,1,3]=true`), `parachains_scheduler::AvailabilityCores` (force-frees stuck cores), and `parachains_scheduler::ClaimQueue` (kills stale claims). All other storage — accounts, balances, sudo, treasury, leafchain registry, staking, sessions — is **byte-identical pre and post**. The chain does not reset; the next block carries the v1.12.0 runtime forward from the existing finalised tip. `setCodeWithoutChecks` is preferred over `setCode` here because the latter rejects multi-version jumps (we are going `94000004 → 112000005`, a deliberate large bump) — the actual safety check is the operator's pre-flight WASM blake2-256 verification (§2.1) plus the migration's idempotent body. The hardened helper script also requires `SUDO_SEED` env var, rejects dev keys, and prints the derived signer address with a 5-second abort window before submitting; verify the address before letting the timer expire.
- **Command** (env-var form preferred — script reads `SUDO_SEED` from env, rejects dev keys //Alice .. //Ferdie, prints signer address, and waits 5 s before tx submit):
  ```bash
  cd polkadot/scripts/forknet
  SUDO_SEED="[OPERATOR FILL IN: sudo seed phrase from secure storage]" \
  WS_ENDPOINT=wss://node.testnet.thxnet.org/archive-001/ws \
  RUNTIME_WASM=/path/to/thxnet_testnet_runtime.compact.compressed.wasm \
  LABEL=rootchain \
    bun run setcode-runtime-upgrade.ts
  ```
  Equivalent CLI form (env vars take precedence if both set):
  ```bash
  SUDO_SEED="[OPERATOR FILL IN]" bun run setcode-runtime-upgrade.ts \
    --endpoint wss://node.testnet.thxnet.org/archive-001/ws \
    --wasm /path/to/thxnet_testnet_runtime.compact.compressed.wasm \
    --label rootchain
  ```
  Expected startup banner — **operator MUST verify this matches the production sudo address before letting the 5-second abort window expire**:
  ```
  [rootchain] sudo signer: 5G... <expected production sudo SS58>
  [rootchain] endpoint: wss://node.testnet.thxnet.org/archive-001/ws
  [rootchain] wasm path: /path/to/thxnet_testnet_runtime.compact.compressed.wasm
  Press Ctrl-C within 5s to abort
  ```
  Dev-key guard: if `SUDO_SEED=//Alice` (or //Bob, //Charlie, //Dave, //Eve, //Ferdie) is passed, the script throws `dev key rejected — production needs real sudo seed` and exits without connecting. This is an explicit safety net against accidental forknet-helper re-use.
- **Expected signals**: (1) Tx InBlock within ~12 s; (2) `system.CodeUpdated` event; (3) next block: spec_version `94000004 → 112000005`; (4) migration log line on at least one validator:
  > `EnableAsyncBackingAndCoretime: num_cores=5, max_vals_per_core=Some(5), lookahead=1, async_backing=(depth=1, ancestry=2), node_features[0,1,3]=true, AvailabilityCores freed, ClaimQueue cleared, active_validators=19`

  (minor drift in `num_cores` is acceptable if registered paraId count differs).
- **Verification**: queries in §5 — `state_getRuntimeVersion` shows `112000005`; ActiveConfig storage decodes with `node_features[3]=true`.
- **Pass**: spec_version flipped + migration log line present + `node_features[3]=true` decoded + finality gap ≤ 3 blocks.
- **Fail**: tx dispatch error, OR no spec_version flip, OR migration panic in validator log, OR finality stalls > 6 blocks.
- **Rollback**: Counter `setCodeWithoutChecks` to the previous runtime — **last resort only**. The migration's storage writes (`async_backing_params`, `node_features`, `AvailabilityCores=Free`, `ClaimQueue=killed`) are NOT undone; the old runtime's `HostConfiguration` decoder may panic on the v1.12.0 layout. **Preferred recovery**: forward-fix with a corrected artefact, not backward-revert. Engage engineering escalation before Phase-2 rollback.

### Phase 2.1 — `kubectl rollout restart` of relay validators

**Goal**: Flush relay-client subsystem caches (prospective-parachains, fragment-chain, SessionInfo) so they re-read the post-migration `ActiveConfig`. ETA ~10 min. Operators: cluster operator + observer.

> **CRITICAL — DO NOT SKIP.** The migration force-frees `AvailabilityCores` in storage, but validator processes hold the stale view in memory until either (a) restart or (b) the next session boundary (~2 h on testnet). Without restart, leafchains may see ghost `BlockedByBacking` for up to 2 hours.

- **Pre-conditions**: Phase 2 PASS; spec_version confirmed `112000005`.
- **Command**:
  ```bash
  kubectl -n <namespace placeholder> rollout restart deploy -l role=testnet-validator
  kubectl -n <namespace placeholder> rollout status   deploy -l role=testnet-validator --timeout=600s
  ```
- **Expected signals**: All 19 pods cycle Terminating → Running; each catches up to head within 60 s. The `EnableAsyncBackingAndCoretime` line may re-appear if the validator catches up across the upgrade block (informational; harmless — migration body is idempotent).
- **Pass**: All 19 `Ready 1/1`; finality gap ≤ 3 blocks; `peers >= 18` per validator.
- **Fail**: More than 1 validator fails to return; finality lag > 6 blocks.
- **Rollback**: Single stuck validator → `kubectl -n <namespace placeholder> describe pod` + diagnose (usually transient). Multiple stuck → revert image to the pre-rollout production binary version (see §2.3 staged artefact) and re-attempt; cache pinning self-resolves at next session boundary regardless.

### Phase 3 — Binary swap on collators (per-leafchain, parallelisable)

**Goal**: Roll the v1.12.0 `thxnet-leafchain` binary onto every collator across the 5 testnet leafchains. Like Phase 1, this is **passive** — runtime changes happen in Phase 4. ETA ~30 min total (5 leafchains in parallel; ~5 min × 2–3 collators each). Operators: cluster operator.

- **Pre-conditions**: Phase 2.1 PASS; new collator image built with v1.12.0 `thxnet-leafchain`.
- **Command (per leafchain × per collator)**:
  ```bash
  kubectl -n <namespace placeholder> set image deploy/collator-<leaf>-N thxnet-leafchain=<registry>/thxnet-leafchain:1.12.0-<sha>
  kubectl -n <namespace placeholder> rollout status deploy/collator-<leaf>-N --timeout=300s
  ```
- **Expected signals**: Collator restarts; logs show `thxnet-leafchain 1.12.0`. Existing leafchain runtime (pre-Phase-4) continues to be applied via WASM execution. Para block production resumes at the pre-existing rate (12-24 s) until Phase 4.
- **Verification**: §5 para block-rate query — monotonically increasing height, gap 12–24 s.
- **Pass**: All collators on new binary; per-leafchain height advances; no `BlockedByBacking` in collator logs.
- **Fail**: Collator crash-loops, OR para height stagnant > 60 s.
- **Rollback**: `kubectl -n <namespace placeholder> rollout undo deploy/collator-<leaf>-N`. Old leafchain binary is wire-compatible with both old runtime and the post-Phase-2 relay.

### Phase 4 — Per-leafchain cumulus 2-step setCode

**Goal**: Apply v1.12.0 `general_runtime` (spec 21 Scenario A; spec 22 Scenario B) to each of the 5 testnet leafchains. ETA ~10 min × 5 = ~50 min. Operators: sudo holder + observer.

> **Order**: `sand-testnet` first (smallest blast radius), then `ecq`, `lmt`, `thx`, `izutsuya`. Do **not** parallelise — each must verifiably succeed before the next starts.

> **Phase 3 → Phase 4 timing constraint**: For each leafchain, **Phase 4 must be initiated within 1 hour of Phase 3 completing for that leafchain**. Reasoning: between Phase 3 (collator binary swap) and Phase 4 (leafchain runtime swap), the collator is running v1.12.0 client code against the pre-v1.12.0 leafchain runtime via WASM execution. This sliver is verified safe (Path E.2 evidence: v1.12.0 leafchain binary boots cleanly on v0.3.3 livenet :code), but it is NOT a state we want to operate in long-term — collator log noise increases (`set_validation_data` warnings under fork-pressure) and any session boundary in this window would force a re-resync of the older runtime state. 1 h is comfortably below the testnet session period (~2 h), eliminates that risk, and keeps each leaf's "mixed" window short enough that on-call SRE can hold attention.

For each leafchain:

- **Pre-conditions**: Phase 3 PASS for that leaf; blake2-256 of WASM pre-verified; sudo key.
- **Action**: cumulus 2-step setCode via `polkadot/scripts/forknet/setcode-parachain.ts`:
  1. `sudo(parachainSystem.authorizeUpgrade(blake2_256(wasm), check_version=false))`
  2. `parachainSystem.enactAuthorizedUpgrade(wasm)` (unsigned)
- **Command** (env-var form preferred — script reads `SUDO_SEED`, rejects dev keys, prints signer banner, 5 s abort window). Look up the per-leaf endpoint in the **per-leaf endpoint table** below — do not free-form the URL.
  ```bash
  cd polkadot/scripts/forknet
  SUDO_SEED="[OPERATOR FILL IN: sudo seed phrase]" \
  WS_ENDPOINT="<endpoint from per-leaf table>" \
  RUNTIME_WASM=/path/to/general_runtime.compact.compressed.wasm \
  LABEL="<leaf>-testnet" \
    bun run setcode-parachain.ts
  ```
  Equivalent CLI form:
  ```bash
  SUDO_SEED="[OPERATOR FILL IN]" bun run setcode-parachain.ts \
    --endpoint "<endpoint from per-leaf table>" \
    --wasm /path/to/general_runtime.compact.compressed.wasm \
    --label "<leaf>-testnet"
  ```
  Expected startup banner — verify signer matches production sudo before the 5 s timer expires:
  ```
  [<leaf>-testnet] sudo signer: 5G... <expected production sudo SS58>
  [<leaf>-testnet] endpoint: wss://node.<leaf>.testnet.thxnet.org/archive-001/ws
  [<leaf>-testnet] wasm path: /path/to/general_runtime.compact.compressed.wasm
  Press Ctrl-C within 5s to abort
  ```
  Dev-key guard applies identically to the parachain helper.

  **Per-leaf endpoint table** (always `wss://...`, no `:443`). For `ecq-testnet`, see the §5 ecq archive-segment caveat — the operator must confirm `archive-001` vs `archive-002` and reflect the answer here AND in `scripts/chopsticks/leafchain-ecq-testnet.yml` before this Phase 4 sub-step runs:

  | Leaf | Endpoint |
  |---|---|
  | sand-testnet | `wss://node.sand.testnet.thxnet.org/archive-001/ws` |
  | ecq-testnet | `[OPERATOR FILL IN: see §5 ecq archive-segment caveat]` |
  | lmt-testnet | `wss://node.lmt.testnet.thxnet.org/archive-001/ws` |
  | thx-testnet | `wss://node.thx.testnet.thxnet.org/archive-001/ws` |
  | izutsuya-testnet | `wss://node.izutsuya.testnet.thxnet.org/archive-001/ws` |

  **Per-leaf paraId table** (operator MUST verify the script's connected-to chain reports the matching paraId via `state_getStorage` of `parachainInfo::ParachainId` — guards against pointing the helper at the wrong leaf):

  | Leaf | paraId |
  |---|---|
  | sand-testnet | `[OPERATOR FILL IN: actual sand-testnet paraId, expected 1003 per rehearsal evidence]` |
  | ecq-testnet | `[OPERATOR FILL IN]` |
  | lmt-testnet | `[OPERATOR FILL IN]` |
  | thx-testnet | `[OPERATOR FILL IN]` |
  | izutsuya-testnet | `[OPERATOR FILL IN]` |
- **Expected signals**: (1) `[1/2] authorizeUpgrade` InBlock ~12 s; (2) `[2/2] enactAuthorizedUpgrade` InBlock ~12 s; (3) wait ~24-36 s for next relay block; (4) `parachainSystem.ValidationFunctionStored` event; (5) leafchain `state_getRuntimeVersion.specVersion` → 21 (or 22 in Scenario B); (6) para continues importing blocks.
- **Verification**: §5 commands — `state_getRuntimeVersion` on the leafchain shows new spec; `kubectl -n <namespace placeholder> logs deploy/collator-<leaf>-1 --tail=200 | grep -E 'ValidationFunctionStored|setCode'`; §5 block-rate sample.
- **Pass**: spec_version flipped + `ValidationFunctionStored` observed + height advances within 60 s of confirmation.
- **Fail**: tx error, OR spec_version stuck > 60 s after enact, OR para halts.
- **Rollback**: Re-run cumulus 2-step setCode with the previous WASM. Same Phase-2 caveat applies; engage engineering before forcing this.

### Scenario A finalisation

After Phase 4 completes for all 5 leafchains:

- All 5 leafchain spec_versions = `21`
- Para block rate stable at 12–18s/block (no engagement of 6s pipelining; this is expected and not a defect)
- Finality gap on rootchain ≤ 3 blocks
- Collator logs free of `BlockedByBacking` / panic / `Cluster has too many pending statements`

Begin **24-hour soak monitoring** (Section 9). Mark rollout COMPLETE only after the soak window closes cleanly.

---

## 4. Rollout sequence — Scenario B (with PR #38 + PR #39)

Scenario B is structurally identical to Scenario A except for two artefact substitutions and one additional verification phase (Phase 5). The critical ordering invariant is:

> **The PR #38 binary must be deployed fleet-wide on relay validators AND collators BEFORE Phase 2's runtime setCode.** If the PR #39 runtime (capacity=2) is applied without the PR #38 binary's fragment-chain rework, candidates can be permanently rejected by `is_fork_or_cycle` in the old subsystem code. Capacity=2 + old subsystem = stall.

Phase-by-phase deltas from Scenario A:

| Phase | Delta from Scenario A |
|---|---|
| **Phase 1** | Binary artefact is the PR #38 build. Same kubectl-set-image mechanics. Same passive nature (no chain effect until Phase 2). PR #38 binary is wire-compatible with the pre-Phase-2 chain because the cherry-picked node-side subsystem changes are **operationally backward-compatible** with the existing capacity=1 runtime. (Validated by 27/27 unit tests + hell-eagle-eye 8/8 forensic gates + 6-val + cap=2 forknet rehearsal sustaining 6s.) |
| **Phase 2** | Identical to Scenario A. Same runtime WASM (`thxnet_testnet_runtime` spec `112_000_005`). |
| **Phase 2.1** | Identical to Scenario A. |
| **Phase 3** | Collators get the PR #38 `thxnet-leafchain` binary. Same passive nature. |
| **Phase 4** | The leafchain runtime injected here is the PR #39 build (`general_runtime` spec **22**, `UNINCLUDED_SEGMENT_CAPACITY=2`). Mechanics identical otherwise. |
| **Phase 5 (new)** | 6s/para-block engagement verification (below). |

### Phase 5 — 6 s/block engagement verification (Scenario B only)

ETA ~10 min per leafchain (100 blocks × 6 s = 600 s); run all 5 in parallel.

- **Pre-conditions**: Phase 4 PASS for that leaf; spec_version=22 confirmed.
- **Action**: Sample `chain_getHeader` ≥ 100 consecutive blocks; compute gap distribution.
- **Command (preferred — `chain_subscribeAllHeads` via polkadot.js, exact arrival timestamps)**:
  ```ts
  // Save as scripts/blocktime-sample.ts; run with:
  //   bun run scripts/blocktime-sample.ts wss://node.<leaf>.testnet.thxnet.org/archive-001/ws 100
  import { ApiPromise, WsProvider } from "@polkadot/api";
  const [, , wsUrl, nStr] = process.argv;
  const target = parseInt(nStr ?? "100", 10);
  const api = await ApiPromise.create({ provider: new WsProvider(wsUrl) });
  const arrivals: { number: number; t: number }[] = [];
  await new Promise<void>((resolve) => {
    api.rpc.chain.subscribeAllHeads((header) => {
      arrivals.push({ number: header.number.toNumber(), t: Date.now() });
      console.log(`#${header.number.toNumber()} @ ${new Date().toISOString()}`);
      if (arrivals.length >= target + 1) resolve();
    });
  });
  await api.disconnect();
  // Compute gaps between consecutive distinct heights (drop forks where number repeats):
  const heights = new Map<number, number>(); // height -> first-seen timestamp (ms)
  for (const a of arrivals) if (!heights.has(a.number)) heights.set(a.number, a.t);
  const sorted = [...heights.entries()].sort((a, b) => a[0] - b[0]);
  const gaps = sorted.slice(1).map(([, t], i) => (t - sorted[i][1]) / 1000);
  const bucket = (g: number) =>
    g < 5 ? "<5" : g <= 7 ? "[5,7]" : g <= 13 ? "[11,13]" : g <= 19 ? "[17,19]" : ">=20";
  const counts: Record<string, number> = {};
  for (const g of gaps) counts[bucket(g)] = (counts[bucket(g)] ?? 0) + 1;
  console.log("gap buckets:", counts, "n=", gaps.length);
  ```
  Equivalent shell-only fallback (smarter sample loop — only logs on hash change, no 1Hz busy-poll):
  ```bash
  # EP = HTTPS endpoint from the §5 per-chain endpoint table
  # (e.g. EP=https://node.sand.testnet.thxnet.org/archive-001 for sand-testnet)
  EP=https://<leaf-endpoint>; OUT=/tmp/blocktime-<leaf>.csv; : > "$OUT"
  PREV=""; SEEN=0
  while [ "$SEEN" -lt 101 ]; do
    HEX=$(curl -s -m 3 -X POST "$EP" -H 'Content-Type: application/json' \
      -d '{"jsonrpc":"2.0","id":1,"method":"chain_getHeader","params":[]}' \
      | grep -oE '"number":"0x[0-9a-f]+"' | cut -d'"' -f4)
    if [ "$HEX" != "$PREV" ] && [ -n "$HEX" ]; then
      printf '%s,%s\n' "$(date +%s.%N)" "$HEX" >> "$OUT"
      PREV="$HEX"; SEEN=$((SEEN + 1))
    fi
    sleep 0.5   # half of expected 6s gap, avoids missing hash transitions
  done
  # Compute gaps between consecutive distinct heights;
  # count buckets [5..7]s, [11..13]s, [17..19]s.
  ```
- **Expected**: ≥ 90% of gaps at 6s ± 1s; remainder at 12s; zero ≥18s gaps. Forknet rehearsal: 27/29 = 93% gaps at 6s.
- **Pass**: ≥ 80% of gaps in the 6 s bucket AND zero ≥18 s gaps over 100 consecutive blocks. (80% gives operational margin over 93% rehearsal.)
- **Fail**: < 80% at 6 s, OR any ≥18 s gap, OR para halts → trigger Scenario B rollback below.

### Scenario B rollback (if Phase 5 fails)

The cleanest rollback for Scenario B is **runtime-only**: re-run cumulus 2-step setCode on the failing leafchain with the **PR #39 baseline (general_runtime spec 21, capacity=1)**. The chain reverts to ~12–18s/block (Scenario A regime) without freezing. **Do not roll back the PR #38 binary** — it is operationally backward-compatible with capacity=1 runtime, and rolling it back is the more disruptive operation.

If rollback is needed for ALL 5 leafchains, run the spec-21 setCode serially in the same order (sand → ecq → lmt → thx → izutsuya).

---

## 5. Verification matrix

**Per-chain endpoint table** — canonical form is `wss://<host>/<archive>/ws` for WS clients (polkadot.js, try-runtime, chopsticks) and `https://<host>/<archive>` for the curl helper below. Do not insert `:443`.

> **ecq-testnet archive segment caveat — operator must confirm before §2.2 runs.** Internal infra notes record ecq-testnet on `/archive-002/ws`, while the committed `scripts/chopsticks/leafchain-ecq-testnet.yml` uses `/archive-001/ws`. These two sources disagree. Before §2.2 try-runtime + chopsticks runs, the operator MUST resolve this by `curl -s -m 5 wss-tested-host` against both forms (or `wscat -c` / polkadot.js tooling) and pick whichever returns a healthy `system_chain` response, then update both the table below and the chopsticks YAML to match. Do NOT proceed with a stale guess.

| Chain | WS endpoint | HTTPS endpoint (for curl `EP=...`) |
|---|---|---|
| Testnet rootchain | `wss://node.testnet.thxnet.org/archive-001/ws` | `https://node.testnet.thxnet.org/archive-001` |
| sand-testnet | `wss://node.sand.testnet.thxnet.org/archive-001/ws` | `https://node.sand.testnet.thxnet.org/archive-001` |
| ecq-testnet | `[OPERATOR FILL IN: confirm archive-001 or archive-002 — see caveat above; default tentative `wss://node.ecq.testnet.thxnet.org/archive-002/ws` per reference_paths_and_artefacts.md]` | `[OPERATOR FILL IN]` |
| lmt-testnet | `wss://node.lmt.testnet.thxnet.org/archive-001/ws` | `https://node.lmt.testnet.thxnet.org/archive-001` |
| thx-testnet | `wss://node.thx.testnet.thxnet.org/archive-001/ws` | `https://node.thx.testnet.thxnet.org/archive-001` |
| izutsuya-testnet | `wss://node.izutsuya.testnet.thxnet.org/archive-001/ws` | `https://node.izutsuya.testnet.thxnet.org/archive-001` |

Reusable helper (set `EP` to the `https://...` form from the right column before running the queries below):

```bash
rpc() { curl -s -m 5 -X POST "$EP" -H 'Content-Type: application/json' \
  -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$1\",\"params\":$2}"; }
```

```bash
# Runtime version (any chain)
rpc state_getRuntimeVersion '[]'
# Rootchain post-Phase-2: specVersion=112000005, specName=thxnet-testnet
# Leafchain post-Phase-4: specVersion=21 (Scenario A) or 22 (Scenario B)

# ActiveConfig storage (rootchain post-Phase-2; decode for async backing primitives)
AC_KEY=0x06de3d8a54d27e44a9d5ce189618f22db4b49d95320d9021994c850f25b8e385
rpc state_getStorage "[\"$AC_KEY\"]"
# Decoded fields to confirm:
#   async_backing_params: max_candidate_depth=1, allowed_ancestry_len=2
#   scheduler_params:    lookahead=1, num_cores=5, max_validators_per_core=Some(5)
#   node_features BitVec: bits 0, 1, 3 all set (bit 3 = CandidateReceiptV2 acceptance)

# Migration log line (across all 19 relay validators).
# Replace <namespace placeholder> with the confirmed namespace from §2.3
# (or `kubectl config set-context --current --namespace=...` once before this loop).
for N in $(seq -w 1 19); do
  kubectl -n <namespace placeholder> logs "deploy/validator-${N}" --since=15m 2>/dev/null \
    | grep -E 'EnableAsyncBackingAndCoretime|CodeUpdated|panic' \
    && echo "  ^ validator-${N}"
done
# At least one MUST show: EnableAsyncBackingAndCoretime: ... node_features[0,1,3]=true ...

# Para block-production rate (5 samples 12s apart)
for i in 1 2 3 4 5; do
  rpc chain_getHeader '[]' | grep -oE '"number":"0x[0-9a-f]+"'
  sleep 12
done
# Scenario A: +1 height per sample (12-18s/block). Scenario B post-Phase-5: +2 per sample (6s/block).

# Finality gap on rootchain (must stay <=3)
rpc chain_getHeader '[]' | grep -oE '"number":"0x[0-9a-f]+"'           # head
FH=$(rpc chain_getFinalizedHead '[]' | grep -oE '"result":"0x[0-9a-f]+"' | cut -d'"' -f4)
rpc chain_getHeader "[\"$FH\"]" | grep -oE '"number":"0x[0-9a-f]+"'    # finalised

# system_health (peers / sync)
rpc system_health '[]'
# Expect isSyncing=false; peers>=18 (rootchain), peers>=4 (typical leafchain).
```

---

## 6. Rollback procedures

**Phase 1 (binary swap, relay)** — fully reversible. `kubectl -n <namespace placeholder> rollout undo deploy/validator-N`. Old binary is wire-compatible with both old runtime and other v1.12.0 binaries.

**Phase 2 (rootchain runtime swap)** — **CAUTION**. Counter-setCode recovers the runtime CODE but does NOT undo migration's storage writes (`async_backing_params`, `node_features`, `AvailabilityCores=Free`, `ClaimQueue=killed`). Old runtime's decoder may panic on v1.12.0 `HostConfiguration` layout. **Phase-2 rollback is last resort**; prefer forward-fix (re-attempt with corrected artefact). If you must:

```bash
cd polkadot/scripts/forknet
SUDO_SEED="[OPERATOR FILL IN]" \
WS_ENDPOINT=wss://node.testnet.thxnet.org/archive-001/ws \
RUNTIME_WASM=/path/to/PREVIOUS/thxnet_testnet_runtime.compact.compressed.wasm \
LABEL=rootchain-rollback \
  bun run setcode-runtime-upgrade.ts
```

**Phase 2.1** — re-run `kubectl rollout restart` if the orchestration itself failed; cache pinning self-resolves at next session boundary regardless.

**Phase 3 (collator binary)** — reversible. `kubectl -n <namespace placeholder> rollout undo deploy/collator-<leaf>-N`.

**Phase 4 (leafchain runtime swap)** — same Phase-2 caveat. If a specific leafchain reveals a defect:

```bash
cd polkadot/scripts/forknet
SUDO_SEED="[OPERATOR FILL IN]" \
WS_ENDPOINT="<endpoint from §3 Phase 4 per-leaf table>" \
RUNTIME_WASM=/path/to/PREVIOUS/general_runtime.compact.compressed.wasm \
LABEL="<leaf>-testnet-rollback" \
  bun run setcode-parachain.ts
```

For Scenario B Phase-5 failure: downgrading spec 22 → 21 (cap=2 → cap=1) is **safe** — cap=1 is the Scenario A baseline that was independently verified. This is the recommended Scenario-B rollback. **Do NOT roll back the PR #38 binary** — it is operationally backward-compatible with cap=1.

---

## 7. Known gotchas

Distilled from `feedback_testing_traps.md`, validated by Path E.1/E.2/B in `REPORT-rehearsal-v5-2026-05-07.md`.

| Gotcha | Failure mode | Mitigation |
|---|---|---|
| **4-flag forknet boot rule** | Forknet/local boots of `Live`-chainType specs need `--discover-local --allow-private-ip --public-addr=/ip4/127.0.0.1/...` + static `--node-key`, otherwise validation peer-set never opens (symptom `Cluster has too many pending statements`, paras stuck). | **Production-irrelevant**: production validators advertise public addresses + use real authority-discovery DHT. The new container image must carry the existing production `args:` block unchanged — do not graft any forknet flags onto the production manifest. |
| **Phase 2.1 NON-OPTIONAL** | Validator processes hold stale `ActiveConfig` cache (~2 h on testnet) without restart → ghost `BlockedByBacking` post-migration. | Always run Phase 2.1. |
| **Cumulus 2-step setCode required** | Direct `system.setCode` on a leafchain → `1010: would exhaust block limits` (WASM ~1.3 MB > block budget). | Use `setcode-parachain.ts`: authorize (small) + enact (unsigned). |
| **24–36 s wait after `enactAuthorizedUpgrade`** | Cumulus needs next relay block + apply runtime before spec flips. Re-issuing causes confusion. | Wait 60 s before re-checking `state_getRuntimeVersion`. |
| **try-runtime CLI lowercases absolute paths** | Uppercase paths (`/mnt/HC_Volume_...`) silently lowercased → `No such file or directory` panic. | Always invoke try-runtime with relative paths (`cd` first). |
| **`bridge-hub-westend-runtime` workspace check** | Pre-existing CI noise; Westend not in THXNET. | Ignore. |
| **CI flake `Cargo check (thxnet crates)`** | Known cancel/fail @ ~1h on PR #36/#37/#38. Not a code defect. | Use a clean re-run if you need definitive CI green; otherwise the P0–P6.4 + try-runtime + chopsticks evidence is sufficient. |
| **Migration log may appear multiple times** | Validators that catch up across the upgrade block re-emit the line. | Informational; migration body is idempotent (`if cfg.x < y { x = y }` is no-op post-migration). |
| **Forknet helpers location moved (PR #36)** | Pre-PR-#36 muscle memory is `forknet/` top-level. | Canonical path is `polkadot/scripts/forknet/`. |

---

## 8. Risk register

| ID | Risk | P × I | Mitigation / Monitoring | Owner |
|---|---|---|---|---|
| **R-1** | Phase 2 migration body fails on testnet livenet state | LOW × HIGH (try-runtime live PASS multiple times in PR #37 evidence; 60 try-state PASS per chain; 4× idempotent re-runs identical) | Counter-setCode to previous runtime (Phase-2 caveats apply — §6); diagnose via try-runtime against post-failure live state. Watch finality dashboard + migration-log presence. | Sudo holder + engineering escalation |
| **R-2** | Phase 2.1 kubectl rollout restart fails | MED × MED (multi-pod orchestration tail risk; cache pinning self-recovers at next session ~2 h) | Manual `kubectl -n <namespace placeholder> delete pod <name>` per stuck validator; escalate if multiple. Watch pod-ready count + per-validator log tails. | Cluster operator |
| **R-3** | Leafchains freeze between Phase 2 and Phase 4 | **NEAR-ZERO × HIGH** — this is exactly what PR #37 prevents (`node_features[3]=true` set atomically with setCode → V2 receipts accepted) | Complete Phase 4 promptly. Multi-hour gap should still self-recover post-Phase-4. Watch per-leafchain height + collator logs for `BlockedByBacking`. | Cluster operator + sudo holder |
| **R-4** | (Scenario B) PR #4937 fragment-chain regression at production scale | V.LOW × HIGH (27/27 unit tests + hell-eagle-eye 8/8 forensic + 6-val cap=2 forknet sustained 6s; pre-existing orphan `fragment_tree/tests.rs` is dead code) | Runtime-only rollback to spec 21 → reverts to Scenario A regime (no freeze). **Do NOT roll back PR #38 binary; downgrade general_runtime only.** Watch Phase-5 gap distribution + relay log for `BlockedByBacking`. | Engineering escalation |
| **R-5** | (Scenario B) capacity=2 stalls in production topology | LOW × HIGH (forknet 6 vals + cap=2 = 93% 6s gaps; prod 19 vals = strictly more pipelining margin) | Same as R-4. | Engineering escalation |
| **R-6** | Sudo key holder unavailable mid-rollout | LOW × HIGH (operational discipline) | Pre-sign + queue Phase 2 + per-leaf Phase 4 txs on offline signer; verify signatures pre-rollout. | Sudo holder + cluster operator |
| **R-7** | Wrong WASM blob submitted (typo/artefact mix-up) | LOW × HIGH (blake2-256 verified pre-flight §2.1) | Hash double-check before signing; observer reads it back; tx event log shows `CodeUpdated`; check `state_getRuntimeVersion` immediately post-tx. | Sudo holder + observer |
| **R-8** | One leafchain's collator pool stuck on old image (Phase 3 incomplete) chokes Phase 4 | LOW × MED (Phase 3 verification catches this) | Re-roll Phase 3 for that leaf; verify all pods on new image before Phase 4. `kubectl -n <namespace placeholder> get pods` per leaf. | Cluster operator |

---

## 9. Telemetry & observability

**Validator-level (relay)** — `journalctl -u polkadot --since "10 minutes ago"`:
- Search for: `EnableAsyncBackingAndCoretime` (Phase 2 confirmation), `CodeUpdated`, `Imported #`, `Finalized #`, `panic`, `Cluster has too many pending statements`, `BlockedByBacking`.
- Finality gap (head − finalised) should stay ≤ 3 blocks.

**Para-level (leafchains)**:
- Poll `chain_getHeader` per leafchain every 30 s (active phases).
- Collator log indicators of trouble: `'no space left for the block in the unincluded segment'` (capacity exhaustion → cap=2 stall in Scenario B, or topology issue); `'set_validation_data inherent needs to be present in every block!'` (cumulus can't see new relay state → check Phase 2.1 cache flush).

**spec_version transitions**: rootchain `94000004 → 112000005` at Phase 2; each leafchain stays pre-rollout through Phases 1-3, then flips to `21` (A) or `22` (B) at its Phase 4 enact.

### Suggested dashboards / alerts

| Metric | Source | Alert |
|---|---|---|
| Rootchain finality gap | head minus finalised height, poll 12 s | > 6 blocks for 2 min |
| Per-leafchain height advance | `chain_getHeader.number` poll 30 s + deltas | No progress for 2 min |
| Per-leafchain spec_version | `state_getRuntimeVersion` poll 60 s | Notify on transition; page if reverts |
| Validator pod-ready count | `kubectl -n <namespace placeholder> get pods -l role=testnet-validator` | < 18 ready for 1 min |
| Per-leafchain collator pod-ready | `kubectl -n <namespace placeholder> get pods -l role=testnet-collator,leaf=<name>` | < 1 ready for 30 s |
| `BlockedByBacking` rate | grep on relay + collator logs | > 10/min sustained for 2 min |

### Expected-value baselines from rehearsal

From `REPORT-rehearsal-v5-2026-05-07.md`:
- Migration log line (testnet topology): `EnableAsyncBackingAndCoretime: num_cores=5, max_vals_per_core=Some(5), lookahead=1, async_backing=(depth=1, ancestry=2), node_features[0,1,3]=true, AvailabilityCores freed, ClaimQueue cleared, active_validators=19`
- spec_version flip: triggered exactly 1 block after `CodeUpdated` event
- Para block-time (Scenario A): 12–18 s/block, mean ~16 s
- Para block-time (Scenario B): 93% gaps at 6 s, ~7% at 12 s, zero ≥18 s gaps

---

## 10. Communication protocol

**Pre-rollout (T-2 h)** — announce to stakeholders. Template:

> THXNET testnet rollout of `release/v1.12.0` is scheduled `[start UTC]` → `[end UTC]`. Impact: rolling restarts of validators + collators. No expected downtime; finality gap may briefly widen at phase transitions. Sudo will perform 1 rootchain runtime upgrade and 5 leafchain runtime upgrades. Questions: `[OPERATOR FILL IN: contact channel]`.

**During rollout** — status updates in `[OPERATOR FILL IN: rollout channel]` every 30 min, even if "no issues." Post immediately at every phase boundary. After Phase 2: paste migration log line + `state_getRuntimeVersion` output. After each leafchain Phase 4: paste `parachainSystem.ValidationFunctionStored` event + `state_getRuntimeVersion`.

**Post-rollout** — 24 h soak with updates every 4 h business hours, once at start/end of overnight window. Soak-complete template:

> THXNET testnet rollout of `release/v1.12.0` COMPLETE. 24 h soak passed: all chains finalising, no degradation. spec_versions: rootchain `112000005`, leafchains `21` (A) / `22` (B).

**Incident** — on any rollback, switch to incident-response channel. Post a one-liner to rollout channel ("ROLLBACK INITIATED: [phase], updates in incident channel"). Engage engineering escalation immediately; do not wait for the next status interval.

---

## 11. Mainnet followup (NOT part of this rollout)

Mainnet rollout is the next major milestone after the testnet 24 h soak passes. Documented here so operators know what's next without re-engaging engineering.

**Status**:
- Mainnet rehearsal (forknet against mainnet seed DB) is **PENDING**. Testnet portion completed 2026-05-06; mainnet portion not started.
- Mainnet seed DB not yet acquired. Acquisition: `kubectl cp` from a mainnet validator or archive node → `/data/forknet-test/mainnet-seed/`. Size ~80–120 GB.
- Mainnet runtime in `release/v1.12.0` is `thxnet` spec `112_000_002` — byte-identical migration body to testnet's `112_000_005` (PR #37 invariant).
- try-runtime live evidence at PR #37 merge: `active_validators=16, num_cores=4, max_vals_per_core=Some(5), node_features[0,1,3]=true` — topology rule fires correctly.

**Pre-conditions before mainnet window opens**:
- Mainnet seed DB acquired and a Path-E.1-analogous forknet rehearsal is GREEN.
- Testnet rollout 24 h soak has passed.
- Additional 7-day testnet observation window with no incidents.
- Mainnet sudo key procedures reviewed and rehearsed.

**Deltas vs this handoff**: rootchain runtime `thxnet_runtime` (not `thxnet_testnet_runtime`); topology 16 vals × 4 cores; 4 leafchains (`avatect`, `lmt`, `ecq`, `thx` — all `-mainnet`); more conservative sudo key procedures. Separate mainnet handoff document will be produced.

---

## 12. Appendix: build artefact provenance

> CI run IDs and artefact hashes shift each rebuild. Verify each at pre-flight time; lock in actual values for the rollout window.

**`release/v1.12.0` HEAD**: branch `release/v1.12.0`, commit `6b7ee05aea` (PR #37 merge). Merge sequence: PR #36 `8c52edec40` (2026-05-04) → PR #37 `6b7ee05aea` (2026-05-05). Reference CI run `25221252584` (PR #35 tip post-merge); the post-PR-#37 successor run is `[OPERATOR FILL IN: post-merge CI run ID for 6b7ee05aea]`.

**Polkadot binary**:
- `polkadot --version` → `[OPERATOR FILL IN]`
- sha256 of `polkadot`, `polkadot-execute-worker`, `polkadot-prepare-worker` → `[OPERATOR FILL IN]`
- Built from `6b7ee05aea` (Scenario A) or `[OPERATOR FILL IN: PR #38 head SHA]` (Scenario B)

**`thxnet_testnet_runtime.compact.compressed.wasm`**: spec_version `112_000_005`, spec_name `thxnet-testnet`, built from `6b7ee05aea`, blake2-256 `[OPERATOR FILL IN]`. Migration body is unified `EnableAsyncBackingAndCoretime` (PR #37; byte-identical to mainnet).

**`general_runtime.compact.compressed.wasm`**:
- Scenario A: spec_version `21`, `UNINCLUDED_SEGMENT_CAPACITY=1`, `BLOCK_PROCESSING_VELOCITY=1`, built from `6b7ee05aea`, blake2-256 `[OPERATOR FILL IN]`.
- Scenario B: spec_version `22`, `UNINCLUDED_SEGMENT_CAPACITY=2`, built from `[OPERATOR FILL IN: PR #39 head SHA]`, blake2-256 `[OPERATOR FILL IN]`.

**Try-runtime feature WASMs** (for §2.2 re-verification): `thxnet-testnet-runtime-try-runtime.wasm` for the rootchain, `general-runtime-try-runtime.wasm` for each leafchain. Same source commits as the non-try-runtime WASMs above.

**Storage keys reference**:

| Key | Storage |
|---|---|
| `0x06de3d8a54d27e44a9d5ce189618f22db4b49d95320d9021994c850f25b8e385` | `parachains_configuration::ActiveConfig` (= `Configuration::ActiveConfig`) — async backing primitives + `node_features` |
| `0xb341e3a63e58a188839b242d17f8c9f87a50c904b368210021127f9238883a6e` | `parachains_shared::ActiveValidatorKeys` (= `ParasShared::ActiveValidatorKeys`) — entry count = `active_validators` for the topology rule |
| `0x3a636f6465` | `:code` (the runtime WASM blob) |
| `0x57f8dc2f5ab09467896f47300f0424385e0621c4869aa60c02be9adcc98a0d1d` | `Aura::Authorities` (leafchain authority set) |

**Rehearsal evidence references** (traceability; operators should not need these during execution):

| Document | Location |
|---|---|
| Path E.1 + E.2 + Path B + 6 s/block achievement (this repo) | `test-results/REPORT-rehearsal-v5-2026-05-07.md` |
| Earlier setCode-mechanics rehearsal (this repo) | `test-results/REPORT-rehearsal-2026-05-06.md` |
| Original P0–P6.4 verification | internal — request from rollout coordinator |
| PR #36 / PR #37 / PR #38 / PR #39 context | `gh pr view <N> --repo thxnet/thxnet-sdk` |
| Substrate gotchas, forknet topology, path catalogue, three-leafchain disambiguation | internal engineering notes — request from rollout coordinator |

---

**End of handoff.** Populate every `[OPERATOR FILL IN: ...]` from local CI artefact metadata before declaring the pre-flight checklist complete.
