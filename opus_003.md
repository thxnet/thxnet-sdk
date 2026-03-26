好，現在呈現完整的 Endgame Master Plan。

---

ENDGAME MASTER PLAN: thxnet-sdk v1.12.0

階段 0: Branching (前置作業)

origin/upgrade/1.12.0 (505618c8) ← 已包含 b4016902ac (upstream v1.12.0)
│
└──► checkout 新 branch: feat/endgame-v1.12.0
│
└──► merge feat/production-port-v2 的 thxnet/ 自訂程式碼
(resolve conflicts, 以 v1.12.0 API 為準)

origin/upgrade/1.12.0 的 merge history 已包含 b4016902ac，所以新 branch 自動擁有完整的 v1.12.0 upstream base。

---

階段 1: Rootchain Runtime Migrations (0.9.40 → 1.12.0)

完整的 Migrations Tuple — 按執行順序排列：

MigrationsEarly = (
// ═══ 0.9.40 → 0.9.43 (從 ~/Works/polkadot/ 移植) ═══
① parachains_configuration::migration::v5::MigrateToV5<Runtime>, // v4→v5 ★PORT
② parachains_configuration::migration::v6::MigrateToV6<Runtime>, // v5→v6 ★PORT
③ custom::NominationPoolsV4ToV5<Runtime>, // v4→v5 ★WRITE (VersionedMigration wrapper)
④ pallet_offences::migration::v1::MigrateToV1<Runtime>, // v0→v1 ★ADD

      // ═══ 0.9.43 → v1.1.0 ═══
      ⑤ parachains_configuration::migration::v7::MigrateToV7<Runtime>,    // v6→v7 ✅ exists
      ⑥ parachains_configuration::migration::v8::MigrateToV8<Runtime>,    // v7→v8 ✅ exists
      ⑦ parachains_configuration::migration::v9::MigrateToV9<Runtime>,    // v8→v9 ✅ exists
      ⑧ paras_registrar::migration::MigrateToV1<Runtime, ...>,            // v0→v1 ✅ exists

      // ═══ v1.2.0 → v1.3.0 ═══
      ⑨ pallet_nomination_pools::migration::versioned::V5toV6<Runtime>,   // v5→v6 ✅ exists
      ⑩ pallet_nomination_pools::migration::versioned::V6ToV7<Runtime>,   // v6→v7 ✅ exists

      // ═══ v1.3.0 → v1.4.0 ═══
      ⑪ custom::StakingV12ObsoleteToV14<Runtime>,                         // v13→v14 bridge ★WRITE
      ⑫ pallet_staking::migrations::v14::MigrateToV14<Runtime>,           // v14 stamp ✅ exists
      ⑬ pallet_grandpa::migrations::MigrateV4ToV5<Runtime>,               // v4→v5 ✅ exists
      ⑭ parachains_configuration::migration::v10::MigrateToV10<Runtime>,  // v9→v10 ✅ exists

      // ═══ v1.4.0 → v1.5.0 ═══
      ⑮ pallet_nomination_pools::migration::versioned::V7ToV8<Runtime>,   // v7→v8 ✅ exists
      ⑯ UpgradeSessionKeys,                                               // ✅ exists
      ⑰ RemovePallet<ImOnlinePalletName, ...>,                            // ✅ exists
      ⑱ RemovePallet<TipsPalletName, ...>,                                // ★ADD

      // ═══ v1.5.0 → v1.6.0 ═══
      ⑲ parachains_scheduler::migration::MigrateV0ToV1<Runtime>,          // v0→v1 ★ADD
      ⑳ parachains_scheduler::migration::MigrateV1ToV2<Runtime>,          // v1→v2 ✅ exists
      ㉑ pallet_identity::migration::versioned::V0ToV1<Runtime, ...>,     // v0→v1 ✅ exists
      ㉒ parachains_configuration::migration::v11::MigrateToV11<Runtime>,  // v10→v11 ✅ exists

);

MigrationsLate = (
// ═══ v1.6.0 → v1.7.0 ═══
㉓ pallet_xcm::migration::MigrateToLatestXcmVersion<Runtime>, // ✅ exists

      // ═══ v1.7.0 → v1.8.0 ═══
      ㉔ pallet_nomination_pools::migration::unversioned::TotalValueLockedSync<Runtime>, // ✅ exists

      // ═══ v1.8.0 → v1.9.0 ═══
      ㉕ parachains_configuration::migration::v12::MigrateToV12<Runtime>,  // v11→v12 ✅ exists

      // ═══ v1.9.0 → v1.10.0 ═══
      ㉖ parachains_inclusion::migration::MigrateToV1<Runtime>,            // v0→v1 ✅ exists
      ㉗ crowdloan::migration::MigrateToTrackInactiveV2<Runtime>,          // v1→v2 ★ADD

      // ═══ v1.10.0 → v1.12.0 (NEW) ═══
      ㉘ pallet_staking::migrations::v15::MigrateV14ToV15<Runtime>,        // v14→v15 ★ADD

      // ═══ GRANDPA finality fix (noop if block > 14.25M, but safe to include) ═══
      ㉙ custom::FixGrandpaFinalityDeadlock,                               // ★PORT from rootchain

);

★ 標記 = 需要寫或移植的 (9 項)：

┌─────┬────────────────────────┬────────────────────────────────┬──────────────────────────────────┐
│ # │ 工作 │ 來源 │ 複雜度 │
├─────┼────────────────────────┼────────────────────────────────┼──────────────────────────────────┤
│ ① │ Configuration v5 │ ~/Works/polkadot/ v5.rs (23KB) │ HIGH — 需改 imports 適配 v1.12.0 │
├─────┼────────────────────────┼────────────────────────────────┼──────────────────────────────────┤
│ ② │ Configuration v6 │ ~/Works/polkadot/ v6.rs (15KB) │ HIGH — 同上 │
├─────┼────────────────────────┼────────────────────────────────┼──────────────────────────────────┤
│ ③ │ NominationPools v4→v5 │ 提取 v5::MigrateToV5 邏輯 │ MED — VersionedMigration wrapper │
├─────┼────────────────────────┼────────────────────────────────┼──────────────────────────────────┤
│ ④ │ Offences v0→v1 │ 已在 codebase │ LOW — 加入 tuple │
├─────┼────────────────────────┼────────────────────────────────┼──────────────────────────────────┤
│ ⑪ │ Staking bridge v13→v14 │ TMP_PLAN 設計 │ MED — custom migration │
├─────┼────────────────────────┼────────────────────────────────┼──────────────────────────────────┤
│ ⑱ │ Tips RemovePallet │ 一行 │ LOW │
├─────┼────────────────────────┼────────────────────────────────┼──────────────────────────────────┤
│ ⑲ │ Scheduler v0→v1 │ 已在 codebase │ LOW — 加入 tuple │
├─────┼────────────────────────┼────────────────────────────────┼──────────────────────────────────┤
│ ㉗ │ Crowdloan v1→v2 │ 已在 codebase │ LOW — 加入 tuple │
├─────┼────────────────────────┼────────────────────────────────┼──────────────────────────────────┤
│ ㉘ │ Staking v14→v15 │ 已在 codebase │ LOW — 加入 tuple │
├─────┼────────────────────────┼────────────────────────────────┼──────────────────────────────────┤
│ ㉙ │ GRANDPA fix │ ~/Works/rootchain/ │ MED — port + adapt │
└─────┴────────────────────────┴────────────────────────────────┴──────────────────────────────────┘

Rootchain construct_runtime! 變更：

- 加入 FinalityRescue: pallet_finality_rescue = 135

---

階段 2: Leafchain Runtime Migrations (spec v2/v3 → v4, to v1.12.0)

3 種起始狀態，1 個統一 runtime：

┌──────────────────┬──────────────────────┬───────────┬───────────┬──────────┬──────────┬──────────┬──────────┐
│ 狀態 │ 鏈 │ XcmpQueue │ DmpQueue │ Rwa │ CF │ TA │ Identity │
├──────────────────┼──────────────────────┼───────────┼───────────┼──────────┼──────────┼──────────┼──────────┤
│ General v2 │ THX/LMT/Izutsuya (5) │ null(≈v3) │ null(≈v0) │ 不存在 │ 不存在 │ 不存在 │ 不存在 │
├──────────────────┼──────────────────────┼───────────┼───────────┼──────────┼──────────┼──────────┼──────────┤
│ General v2 (ECQ) │ ECQ (2) │ null(≈v2) │ null(≈v0) │ 不存在 │ 不存在 │ 不存在 │ 不存在 │
├──────────────────┼──────────────────────┼───────────┼───────────┼──────────┼──────────┼──────────┼──────────┤
│ Avatect v3 │ Avatect (1) │ v3 │ v2 │ v0(38件) │ v0(21件) │ 不存在 │ 不存在 │
├──────────────────┼──────────────────────┼───────────┼───────────┼──────────┼──────────┼──────────┼──────────┤
│ Sand v3 │ Sand (1) │ v3 │ v2 │ 不存在 │ 不存在 │ v1(12件) │ null │
└──────────────────┴──────────────────────┴───────────┴───────────┴──────────┴──────────┴──────────┴──────────┘

完整的 Leafchain Migrations Tuple：

pub type Migrations = (
// ═══ Cumulus framework migrations ═══
① pallet_collator_selection::migration::v1::MigrateToV1<Runtime>, // v0→v1 ✅
② cumulus_pallet_xcmp_queue::migration::v2::MigrationToV2<Runtime>, // for ECQ v2→v3
③ cumulus_pallet_xcmp_queue::migration::v3::MigrationToV3<Runtime>, // v2→v3 ✅
④ cumulus_pallet_xcmp_queue::migration::v4::MigrationToV4<Runtime>, // v3→v4 ✅
⑤ pallet_collator_selection::migration::v2::MigrationToV2<Runtime>, // v1→v2 ★ADD (v1.12.0)
⑥ pallet_xcm::migration::MigrateToLatestXcmVersion<Runtime>, // ✅

      // ═══ DmpQueue: force stamp v0→v2 (zero data on all chains) ═══
      ⑦ custom::InitDmpQueueStorageVersion,                                 // ★WRITE

      // ═══ Custom pallet migrations ═══
      ⑧ pallet_rwa::migrations::v5::MigrateToV5<Runtime>,                  // v0→v5 stamp ✅
      ⑨ custom::CrowdfundingStampOrMigrateToV3<Runtime>,                   // ★WRITE (handles v0→v3)
      ⑩ pallet_trustless_agent::migrations::Migrations<Runtime>,            // v0→v1 ✅
      // Identity: new pallet, starts at v0, needs stamp if code > v0       // CHECK v1.12.0

      // ═══ Treasury: ADD BACK to construct_runtime, no migration needed ═══
      // (on-chain has 0 data, pallet initializes cleanly)

);

★ 標記 = 需要寫的 (3 項)：

┌─────┬──────────────────────────────────┬───────────────────────────────────────────────────────────────────────────────┐
│ # │ 工作 │ 複雜度 │
├─────┼──────────────────────────────────┼───────────────────────────────────────────────────────────────────────────────┤
│ ⑦ │ DmpQueue force stamp v0→v2 │ LOW — 直接 put StorageVersion(2) │
├─────┼──────────────────────────────────┼───────────────────────────────────────────────────────────────────────────────┤
│ ⑨ │ Crowdfunding v0→v3 stamp/migrate │ MED — on-chain v0 有兩種情況：有資料(Avatect)直接 stamp v3, 無資料也 stamp v3 │
├─────┼──────────────────────────────────┼───────────────────────────────────────────────────────────────────────────────┤
│ │ Treasury 加回 construct_runtime │ MED — 需要完整 Config trait impl │
└─────┴──────────────────────────────────┴───────────────────────────────────────────────────────────────────────────────┘

Leafchain construct_runtime! 變更：

- 加回 Treasury: pallet_treasury = 19（附 Config trait 實作）
- 確認所有 pallet indices 不衝突

---

階段 3: Node Binary 變更

Rootchain binary 必帶：

- GRANDPA hard fork entries (4 entries, set_id 987-990) — 已 ported ✅
- reset_grandpa_state() stuck-node recovery — 已 ported ✅
- is_thxnet_mainnet() detection — 已 ported ✅
- FinalityRescue pallet（compile into native runtime）

Leafchain binary：

- 標準 cumulus parachain node，無特殊修改

---

階段 4: 部署順序

Phase A: Forked Testnet 驗證
├─ 1. k8s 隔離環境復刻 testnet
├─ 2. 部署新 rootchain binary (10 validators rolling restart)
├─ 3. sudo.setCode() rootchain runtime WASM
├─ 4. try-runtime on-runtime-upgrade 驗證
├─ 5. 部署新 leafchain binary (all collators)
├─ 6. sudo.sudoUncheckedWeight(system.setCode()) per leafchain
└─ 7. 驗證所有 pallets 功能正常

Phase B: Live Testnet
└─ 同 Phase A，但在真實 testnet 上

Phase C: Live Mainnet
└─ 同 Phase A，但在 mainnet 上

---

階段 5: 驗證清單

┌─────────────────────────────┬─────────────────┬──────────────────────────┐
│ 驗證項目 │ 工具 │ 目標 │
├─────────────────────────────┼─────────────────┼──────────────────────────┤
│ 所有 migration 正確觸發 │ try-runtime │ 0 個 FATAL/ERROR │
├─────────────────────────────┼─────────────────┼──────────────────────────┤
│ NFTs 完整性 (169K+) │ RPC query count │ 數量一致 │
├─────────────────────────────┼─────────────────┼──────────────────────────┤
│ Staking validators 正常 │ polkadot.js │ 10 validators active │
├─────────────────────────────┼─────────────────┼──────────────────────────┤
│ Crowdfunding campaigns 完整 │ RPC query │ 21 campaigns on Avatect │
├─────────────────────────────┼─────────────────┼──────────────────────────┤
│ TrustlessAgent 完整 │ RPC query │ 12 agents on Sand │
├─────────────────────────────┼─────────────────┼──────────────────────────┤
│ Zero-fee 正常 │ 送交易 │ fee = 0 │
├─────────────────────────────┼─────────────────┼──────────────────────────┤
│ Finality 正常 │ 觀察 │ blocks finalize < 12s │
├─────────────────────────────┼─────────────────┼──────────────────────────┤
│ XCM 正常 │ zombienet test │ messages relay correctly │
└─────────────────────────────┴─────────────────┴──────────────────────────┘

---

風險矩陣

┌─────────────────────────────────────────┬───────────────────┬───────────┬───────────────────────────────────────────────┐
│ 風險 │ 影響 │ 機率 │ 緩解 │
├─────────────────────────────────────────┼───────────────────┼───────────┼───────────────────────────────────────────────┤
│ Configuration v5/v6 移植 imports 不匹配 │ 編譯失敗 │ HIGH │ 需要手動適配 types │
├─────────────────────────────────────────┼───────────────────┼───────────┼───────────────────────────────────────────────┤
│ Crowdfunding stamp 邏輯錯誤 │ 21 campaigns 損壞 │ LOW │ try-runtime pre/post checks │
├─────────────────────────────────────────┼───────────────────┼───────────┼───────────────────────────────────────────────┤
│ NFTs storage 不兼容 │ 169K NFTs 損毀 │ NEAR ZERO │ 已確認 v1→v1 無變化 │
├─────────────────────────────────────────┼───────────────────┼───────────┼───────────────────────────────────────────────┤
│ GRANDPA finality 問題 │ 鏈停 │ LOW │ Node-level fix 已 ported + runtime fix 已執行 │
├─────────────────────────────────────────┼───────────────────┼───────────┼───────────────────────────────────────────────┤
│ ECQ DmpQueue 特殊狀態 │ ECQ 壞 │ LOW │ 已確認 0 data，stamp 安全 │
└─────────────────────────────────────────┴───────────────────┴───────────┴───────────────────────────────────────────────┘

---

現在告訴我：你想從哪裡開始？ 建議順序：

1. 先建立 branch (階段 0) — 從 origin/upgrade/1.12.0 checkout + merge custom code
2. 先修最簡單的 LOW 項 — Tips RemovePallet, Scheduler v0→v1, Crowdloan v1→v2, Staking v15, Offences v1 (全部是「加入 tuple」)
3. 再修 MEDIUM 項 — Staking bridge, DmpQueue stamp, Crowdfunding stamp, Treasury 加回
4. 最後 HIGH 項 — Configuration v5/v6 移植, NominationPools v4→v5 wrapper
