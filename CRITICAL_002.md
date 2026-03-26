polkadot-sdk polkadot-v1.11.0 vs thxnet-sdk PR #13 (upgrade/1.11.0)

總覽

┌───────────────┬─────────┐
│ 指標 │ 數值 │
├───────────────┼─────────┤
│ Files changed │ 264 │
├───────────────┼─────────┤
│ Insertions │ +48,664 │
├───────────────┼─────────┤
│ Deletions │ -2,240 │
└───────────────┴─────────┘

變更分類

┌────────────────────────┬────────┬──────────────────────────────────────────────────────────────┐
│ 類別 │ 檔案數 │ 說明 │
├────────────────────────┼────────┼──────────────────────────────────────────────────────────────┤
│ thxnet/ (新增) │ 194 │ THXNet 全部自訂程式碼（+46,409 行，純新增） │
├────────────────────────┼────────┼──────────────────────────────────────────────────────────────┤
│ .github/workflows/ │ 32 │ 刪除上游 CI，替換成 THXNet 自有 CI │
├────────────────────────┼────────┼──────────────────────────────────────────────────────────────┤
│ polkadot/node/service/ │ 4 │ 注入 thxnet-native feature、chain spec、Chain enum │
├────────────────────────┼────────┼──────────────────────────────────────────────────────────────┤
│ polkadot/cli/ │ 2 │ 加入 thxnet-dev/thxnet-local 命令 │
├────────────────────────┼────────┼──────────────────────────────────────────────────────────────┤
│ cumulus/ │ ~8 │ 上游 patch（可見性、weight 修改、XCM test 修復） │
├────────────────────────┼────────┼──────────────────────────────────────────────────────────────┤
│ substrate/ │ 5 │ 格式化 diff + execute_blob/send_blob revert 適配 │
├────────────────────────┼────────┼──────────────────────────────────────────────────────────────┤
│ Cargo.lock/toml │ 2 │ workspace 新增 thxnet crates │
├────────────────────────┼────────┼──────────────────────────────────────────────────────────────┤
│ 其他 │ ~17 │ .cargo/config.toml、.gitignore、rust-toolchain.toml、scripts │
└────────────────────────┴────────┴──────────────────────────────────────────────────────────────┘

---

1. THXNet 自訂程式碼（thxnet/ 目錄，194 檔，+46,409 行）

這是 fork 的核心價值，全部是 新增，上游不存在：

- Rootchain runtime (thxnet/runtime/thxnet/, thxnet/runtime/thxnet-testnet/) — 完整的 relay chain runtime，含 weights、xcm_config
- Leafchain (thxnet/leafchain/) — parachain node + runtime（general-runtime），含：
  - pallet-trustless-agent（2,499 行 lib + 1,155 行 tests）
  - XCM emulator + integration tests (dmp/ump/xcmp)
  - 多條 mainnet/testnet chain specs（activa, avatect, lmt, mirrored_body, thx, aether, izutsuya, sand, txd）
- Docker 構建檔案
- Runtime constants (thxnet/runtime/thxnet/constants/)

2. 上游程式碼的修改（Upstream Patches）

實質性修改（需要關注）：

polkadot/node/service/ — 注入 THXNet 作為新的 native runtime：

- Chain::Thxnet enum variant
- is_thxnet() 識別方法
- thxnet-native feature flag 貫穿 Cargo.toml
- chain_spec.rs 加入 +144 行的 THXNet genesis config

cumulus/pallets/aura-ext/src/lib.rs — SlotInfo 從 pub(crate) 改為 pub（leafchain 需要跨 crate 存取）

substrate/frame/contracts/src/wasm/runtime.rs — 適配 execute_blob/send_blob（因為上游 v1.11.0 revert 了 execute_blob/send_blob，THXNet 這裡做了反向適配）

polkadot/xcm/src/v4/mod.rs — XCM decode test 擴充（加入 MAX_ITEMS_IN_ASSETS 測試）

格式化/cosmetic diff：

- substrate/frame/nis/src/lib.rs — 純 rustfmt 重排
- substrate/primitives/state-machine/src/trie_backend.rs — GAT where clause 格式化（nightly-2024-04-10 的新 rustfmt 規則）
- substrate/frame/bags-list/src/list/tests.rs — 註解換行

工具鏈：

- 新增 rust-toolchain.toml（nightly-2024-04-10）
- .cargo/config.toml 加入 curve25519_dalek_backend="serial" rustflag

---

3. CI 替換（.github/workflows/）

刪除了上游全部 22 個 workflows（check-features、check-licenses、check-prdoc、srtool、subsystem-benchmarks、sync-templates 等），替換為 4 個 THXNet 自有 workflows：

┌───────────────┬──────────────────────────────────────────────────────┐
│ Workflow │ 內容 │
├───────────────┼──────────────────────────────────────────────────────┤
│ ci.yml │ Build + Docker + try-runtime + zombienet + XCM tests │
├───────────────┼──────────────────────────────────────────────────────┤
│ rust.yml │ cargo check (只 check，不 test) │
├───────────────┼──────────────────────────────────────────────────────┤
│ fmt-check.yml │ cargo fmt --check │
├───────────────┼──────────────────────────────────────────────────────┤
│ release.yml │ Release 流程 │
└───────────────┴──────────────────────────────────────────────────────┘

---

總結與風險評估

THXNet 的 fork 策略是最小侵入式的 — 上游程式碼的修改非常少（約 70 檔是上游 patch，其中大半是 Cargo.lock/格式化），主要是注入 thxnet-native feature 和新增 thxnet/ 目錄。

但如我之前分析的，CI 是最大的盲點 — 上游的 22 個 quality gate workflows 全被刪掉了，取代的 4 個 workflows 缺少 cargo test、clippy、benchmark 驗證。Raj 看到的「all tests passed」其實只是「能編譯 + 能出 block + 1 個 XCM test package 通過 + try-runtime 不
panic（但跳過 mbm checks）」。
