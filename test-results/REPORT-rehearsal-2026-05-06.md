# Mini-fork Rehearsal Report — testnet rootchain (post PR #36 + #37)

**Date**: 2026-05-06
**Worktree**: `/mnt/HC_Volume_105402799/worktrees/thxnet-rehearsal`
**Branch**: `review/v1.12.0-post-pr37` (tracks `origin/release/v1.12.0`)
**HEAD**: `6b7ee05aeaabdce1417e2b4f0f50405ce9a5ac9f` (PR #37 merge — unified `EnableAsyncBackingAndCoretime`)
**Scope**: testnet rootchain forknet rehearsal only (mainnet rehearsal explicitly deferred this session — mainnet seed DB not locally available)

## Headline result

**PASS** — cumulus 2-step setCode flow on a 3-validator + 2-collator forknet booted from PR #37-tip remains green:

| Gate | Result |
|---|---|
| Build polkadot binary (`cargo build --release -p polkadot -p thxnet-leafchain`) | **PASS** — 22m 08s, exit 0 |
| `polkadot fork-genesis --help` | PRESENT (PR #36 port verified end-to-end) |
| fork-genesis from `/data/forknet-test/rootchain-seed` | PASS — 21.7 MB output, paraId 2000 registered |
| Relay finalize first block (3 validators) | PASS — finalized within 30 s |
| Para advance past #1 | PASS — para reached #5 within ~85 s of collator boot |
| Cumulus 2-step setCode `[1/2] authorizeUpgrade` | PASS — InBlock, no dispatch error |
| Cumulus 2-step setCode `[2/2] enactAuthorizedUpgrade` | PASS — InBlock at 12.1 s, no dispatch error |
| `parachainSystem.ValidationFunctionStored` event | PRESENT at block #7 |
| Para sustained advance post-setCode | PASS — +3 blocks in 36 s, +1 block in 8 s further |
| W1 / W2 / W4 drift | PASS — zero drift throughout (sha256 unchanged) |

Two operational notes — neither indicates a regression:

1. **`spec_version stayed at 21`** — by design (idempotent test, identical v1.12.0 WASM injected as both genesis `:code` and setCode payload). Matches prior P6.4 result on 2026-05-03.
2. **No `EnableAsyncBackingAndCoretime` log line in relay boot** — expected, because v1.12.0 polkadot's `fork-genesis` builds genesis storage directly with the v1.12.0 runtime metadata. There is no spec_version transition at block #1, so `on_runtime_upgrade()` is a no-op. The migration body itself was already validated by try-runtime live runs against mainnet + testnet during PR #37 (`active_validators=16/19`, 60 try-state checks PASS).

## Build artefacts

```
/mnt/HC_Volume_105402799/worktrees/thxnet-rehearsal/
├── target/release/
│   ├── polkadot                    148 MB  (polkadot 1.12.0-6b7ee05aeaa, fork-genesis baked in)
│   ├── thxnet-leafchain            180 MB  (thxnet-leafchain 0.5.0-6b7ee05aeaa)
│   └── wbuild/
│       ├── general-runtime/general_runtime.compact.compressed.wasm                  1.34 MB
│       └── thxnet-testnet-runtime/thxnet_testnet_runtime.compact.compressed.wasm    2.07 MB
└── ci-artefacts/
    ├── binaries/thxnet-leafchain                                                    (copy of above)
    └── wasm-runtimes/{general-runtime,thxnet-testnet-runtime}/                      (copies of above)
```

Build env: `CC=clang-14 CXX=clang++-14 LIBCLANG_PATH=/usr/lib/llvm-14/lib CXXFLAGS="-include cstdint"`.
Toolchain: rustc 1.95.0 + nightly-2024-04-10 (workspace-pinned).

## Spec JSONs regenerated

W3 deletion took the prior spec JSONs with it. We regenerated:

| File | Source | Size | Purpose |
|---|---|---|---|
| `forknet/forked-thxnet-testnet-baseline.json` | old `polkadot 0.9.40-6425faa1deb` `build-spec --chain=thxnet-testnet --raw` | 3.2 MB | INPUT_SPEC for fork-genesis (id=`thxnet_testnet`) |
| `forknet/w6-t3-verify.json` | old `thxnet-leafchain 0.3.3-2de2fcfc198` `build-spec --chain=sand-testnet --raw --disable-default-bootnode` | 1.8 MB | (initially intended para spec — UNUSABLE, see below) |
| `forknet/w6-t3-verify-v1.12.0.json` | inject v1.12.0 general-runtime WASM into the above as `:code` | 2.7 MB | (initially intended PARA_JSON — UNUSABLE) |
| `forknet/w6-t3-verify-v1.12.0-dev.json` | new `thxnet-leafchain 0.5.0` `build-spec --chain=dev --raw` | 2.7 MB | **actual PARA_JSON** (id=`leafchain_dev`, paraId=2000, //Alice as sole Aura authority, v1.12.0 `:code` already baked in) |

## The Aura authority pivot (key gotcha discovered this session)

First boot attempt used `w6-t3-verify-v1.12.0.json` (sand-testnet baseline + v1.12.0 :code injected). It booted relay clean but **para stuck at #0**. Diagnosis:

- `Aura::Authorities` storage in sand-testnet's `Live`-chainType genesis = production sr25519 keys: `0x80fbbf...4f41` and `0x2075e5...e834`
- Our keystore inserted `//Alice` (`0xd43593...da27d`) and `//Bob` (`0x8eaf04...26a48`)
- No matching pub key → no slot owner produces collation → relay log: `parachain::collation-generation: collator returned no collation on collate para_id=Id(1003)`
- Collator log: zero Aura/slot/authority/Imported messages — Aura silently discovers no usable key and idles

**Fix**: switch para spec to `--chain=dev` (paraId 2000, `leafchain_dev` chain id, `Aura::Authorities = [//Alice]` baked in by genesis builder). Required script substitutions (sed onto the boot wrapper, source script untouched):

- `--register-leafchain="1003:..."` → `--register-leafchain="2000:..."`
- keystore dir `chains/sand_testnet/` → `chains/leafchain_dev/`
- `PARA_JSON` value → `forknet/w6-t3-verify-v1.12.0-dev.json`

After pivot: para advanced #0 → #5 within 85 s of collator boot.

**Memo for next time**: any "fresh-state" mini-fork rehearsal should start from a leafchain `--chain=dev` spec rather than the sand-testnet `Live` spec. The original P6.4 work pre-2026-05-04 used a livenet-state-merged sand-testnet para spec (where the Aura authorities had been substituted by the operator who created the spec); this session's W3-rebuilt baseline didn't carry that substitution.

## Topology

```
RELAY (3-validator forknet, fork-genesis from /data/forknet-test/rootchain-seed)
  Alice    --alice    p2p=40331  rpc=9931  bootnode for Bob+Charlie
  Bob      --bob      p2p=40332  rpc=9932
  Charlie  --charlie  p2p=40333  rpc=9933

PARA (paraId=2000, leafchain_dev, v1.12.0 :code, //Alice as Aura authority)
  sand-Alice --collator --alice  p2p=40334  rpc=9934  embedded relay client p2p=40335 rpc=9935
  sand-Bob   --collator --bob    p2p=40336  rpc=9936  embedded relay client p2p=40337 rpc=9937
```

Required boot flags (verified again this session — drop any one and backing peer-set never opens between group peers):
- `--discover-local` (flips `allow_non_globals_in_dht=true`)
- `--allow-private-ip` (transport allows dialing private IPs)
- `--public-addr=/ip4/127.0.0.1/tcp/<port>/p2p/<peer_id>` (full multiaddr including `/p2p/...`)
- Static `--node-key=<hex>` per validator (so authority-discovery cache stays valid across boots)

## Timing summary

| Phase | Elapsed |
|---|---|
| `cargo build` (cold cache, 8-core AMD EPYC-Genoa) | 22 m 08 s |
| Spec regeneration (baseline + para) | < 30 s |
| fork-genesis (relay spec from rootchain-seed) | 3 s |
| Relay 3-validator boot → finalized #1 | ~30 s |
| 60 s grace + collator boot | ~70 s |
| Para advance past #1 (#0 → #5) | ~85 s |
| `[1/2] authorizeUpgrade` InBlock | sub-second after submit |
| `[2/2] enactAuthorizedUpgrade` InBlock | 12.1 s |
| Post-setCode +3 blocks (#5 → #8) | 36 s |
| Total wall-clock (build excluded) | ~5 min |

## Drift baseline (W1 / W2 / W4)

| Worktree | Path | sha256 | Verdict |
|---|---|---|---|
| W1 | `/root/Works/thxnet-sdk` | `a6014d908d4a130c40b15a93d05bed0d83bb0b777d732171210d414a6f9cf37c` | unchanged |
| W2 | `/mnt/HC_Volume_105402799/worktrees/thxnet-release-v1.12` | `71bbb56562fc20df4fc03498efc0351959d701244723097a6fc6b12d9dbcf42d` | unchanged |
| W4 | `/mnt/HC_Volume_105402799/worktrees/thxnet-upgrade-v1.12` | `4d1b15ed4357f44b6017d0b7941996581f7c7b7ada550f6ce4d482396157740c` | unchanged |

## Conclusion

PR #36 (fork-genesis CLI port) and PR #37 (unified async-backing migration) on `release/v1.12.0` (`6b7ee05aea`) **remain green for the testnet mini-fork rehearsal flow**. The cumulus 2-step setCode path is mechanically sound; the para sustains advance under the merged stack. No regression vs the 2026-05-03 P6.4 PASS.

Mainnet rehearsal remains pending — needs a mainnet rootchain seed DB acquired from prod (kubectl cp from a mainnet validator/archive node) before it can be exercised end-to-end against livenet state.
