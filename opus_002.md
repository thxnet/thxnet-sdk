所有調查完成。情況比原先想像的更複雜。讓我呈現完整發現，然後問剩餘的關鍵問題。

---

調查結果總結

1. spec_version 94000004 的真相 (F6 答案)

94000004 包含了兩個 migrations：

- NominationPools::migration::v4::MigrateToV4 — 已從 94000001 開始存在並執行
- FixGrandpaFinalityDeadlock — 在 94000004 新增並已在 mainnet 執行

這表示：

- NominationPools 已經是 v4 ✅
- GRANDPA runtime migration 已在 mainnet 成功執行 ✅ — 不需要再跑
- 94000005 新增了 FinalityRescue pallet (index 135)，但從未上鏈

2. GRANDPA Fix 完整狀態

┌─────────────────────────────────────┬────────────────────┬───────────────────────────┬──────────────────────────────┐
│ 層級 │ rootchain │ thxnet-sdk │ 狀態 │
├─────────────────────────────────────┼────────────────────┼───────────────────────────┼──────────────────────────────┤
│ Node: hard fork entries │ ✅ 4 entries │ ✅ 已 port │ OK │
├─────────────────────────────────────┼────────────────────┼───────────────────────────┼──────────────────────────────┤
│ Node: reset_grandpa_state() │ ✅ │ ✅ 已 port │ OK │
├─────────────────────────────────────┼────────────────────┼───────────────────────────┼──────────────────────────────┤
│ Runtime: FixGrandpaFinalityDeadlock │ ✅ 94000004 已執行 │ ❌ 未包含 │ 但已是 noop (block > 14.25M) │
├─────────────────────────────────────┼────────────────────┼───────────────────────────┼──────────────────────────────┤
│ Runtime: FinalityRescue pallet │ ✅ 94000005 未上鏈 │ ❌ 未進 construct_runtime │ 需加入 │
└─────────────────────────────────────┴────────────────────┴───────────────────────────┴──────────────────────────────┘

3. 完整 Rootchain Migration Gap (v0.9.40 → v1.12.0)

原有 4 個 FATAL + 新發現 5 個缺口：

┌─────┬────────────────────────┬──────────┬──────────────────┬─────────────────────────────┬────────┐
│ # │ Pallet │ On-Chain │ Target (v1.12.0) │ Gap │ 嚴重性 │
├─────┼────────────────────────┼──────────┼──────────────────┼─────────────────────────────┼────────┤
│ F1 │ Configuration │ v4 │ v12 │ v4→v5→v6 遷移不存在 │ FATAL │
├─────┼────────────────────────┼──────────┼──────────────────┼─────────────────────────────┼────────┤
│ F2 │ NominationPools │ v4 │ v8 │ v4→v5 guard 壞 │ FATAL │
├─────┼────────────────────────┼──────────┼──────────────────┼─────────────────────────────┼────────┤
│ F4 │ Tips │ v4 │ 移除 │ 無 RemovePallet │ FATAL │
├─────┼────────────────────────┼──────────┼──────────────────┼─────────────────────────────┼────────┤
│ F9 │ Staking │ v13 │ v15 │ v14→v15 缺失 (v1.12.0 新增) │ FATAL │
├─────┼────────────────────────┼──────────┼──────────────────┼─────────────────────────────┼────────┤
│ F10 │ Scheduler (parachains) │ v0 │ v2 │ v0→v1 缺失 │ HIGH │
├─────┼────────────────────────┼──────────┼──────────────────┼─────────────────────────────┼────────┤
│ F11 │ Crowdloan │ v1 │ v2 │ v1→v2 缺失 │ HIGH │
├─────┼────────────────────────┼──────────┼──────────────────┼─────────────────────────────┼────────┤
│ F12 │ Offences │ v0 │ v1 │ v0→v1 缺失 │ HIGH │
├─────┼────────────────────────┼──────────┼──────────────────┼─────────────────────────────┼────────┤
│ F13 │ Bounties │ v0 │ v4 │ 版本 stamp 不匹配 │ MED │
└─────┴────────────────────────┴──────────┴──────────────────┴─────────────────────────────┴────────┘

好消息: F10/F11/F12 的 migration code 都存在於 codebase，只是沒被加進 migrations tuple。F1 的 v5.rs (23KB) 和 v6.rs (15KB) 在 ~/Works/polkadot/ 找到了。

4. Leafchain 全貌 — 比想像中複雜得多

9 條活鏈，3 種 runtime，2 種 StorageVersion 狀態：

┌──────────┬──────────────────────────────────────────────┬──────────────┬──────────────┬─────────────────────────────────────────────────────┐
│ 組別 │ 鏈 │ Runtime │ spec_version │ 用戶數據量 │
├──────────┼──────────────────────────────────────────────┼──────────────┼──────────────┼─────────────────────────────────────────────────────┤
│ Group A │ Mainnet THX, LMT; Testnet THX, LMT, Izutsuya │ general (v2) │ 2 │ THX: 5.3K accts, 90K NFTs; LMT: 13K accts, 79K NFTs │
├──────────┼──────────────────────────────────────────────┼──────────────┼──────────────┼─────────────────────────────────────────────────────┤
│ Group A\* │ Mainnet ECQ; Testnet ECQ │ general (v2) │ 2 │ 微量 (3 accounts) │
├──────────┼──────────────────────────────────────────────┼──────────────┼──────────────┼─────────────────────────────────────────────────────┤
│ Group B │ Mainnet Avatect │ avatect (v3) │ 3 │ Rwa=38, CF=21 │
├──────────┼──────────────────────────────────────────────┼──────────────┼──────────────┼─────────────────────────────────────────────────────┤
│ Group C │ Testnet Sand │ sand (v3) │ 3 │ TrustlessAgent=12 agents │
└──────────┴──────────────────────────────────────────────┴──────────────┴──────────────┴─────────────────────────────────────────────────────┘

Group A vs A\* 的關鍵差異: ECQ 的 DmpQueue 是 v1，其他都是 v2。ECQ 的 XcmpQueue 是 v2，其他是 v3。

新發現的 Leafchain 缺口：

┌─────┬─────────────────────────────────────────────────────────────┬────────────────┬────────┐
│ # │ 問題 │ 影響範圍 │ 嚴重性 │
├─────┼─────────────────────────────────────────────────────────────┼────────────────┼────────┤
│ F3 │ Crowdfunding MigrateToV3 skip (on-chain v0, guard==2) │ ALL 9 chains │ FATAL │
├─────┼─────────────────────────────────────────────────────────────┼────────────────┼────────┤
│ F5 │ CollatorSelection v1→v2 (v1.12.0 新增) │ ALL 9 chains │ HIGH │
├─────┼─────────────────────────────────────────────────────────────┼────────────────┼────────┤
│ F7 │ Treasury 在 on-chain 但移除了，無 RemovePallet │ ALL 9 chains │ FATAL │
├─────┼─────────────────────────────────────────────────────────────┼────────────────┼────────┤
│ F8 │ ECQ: DmpQueue v1, 無 v1→v2 migration │ 2 chains (ECQ) │ FATAL │
├─────┼─────────────────────────────────────────────────────────────┼────────────────┼────────┤
│ F14 │ 新 pallet (Rwa/CF/TA/Identity) on-chain v0 vs code v3/v5/v1 │ 7+ chains │ FATAL │
└─────┴─────────────────────────────────────────────────────────────┴────────────────┴────────┘

5. Branching 結構

424c0ec509 (merge base)
├── feat/production-port-v2 (HEAD: 140df064f5)
│ └── +2 commits: production port + CI conflict
│
└── origin/upgrade/1.12.0 (HEAD: 505618c8a7)
└── +5 commits: v1.11.0 merge → v1.12.0 merge → CollatorSelection v2 → conflict fixes

origin/upgrade/1.12.0 的問題：它的 migrations tuple 只有 v1.11.0→v1.12.0 的增量遷移，沒有 0.9.40→v1.10.0 的累積遷移。feat/production-port-v2 反過來，有累積但沒有 v1.12.0。

---

剩餘關鍵問題

Q1. leafchain "general" runtime spec_version 2 vs Avatect/Sand spec_version 3

老 leafchains repo 的 develop branch 是 spec_version 3，但 live 上 7 條 general chains 只有 spec_version 2。

這表示 spec_version 3 的 runtime upgrade 從未在這 7 條 general chains 上執行過。spec_version 3 加了什麼？是 Avatect 和 Sand 專有的 runtime change 嗎？ 如果是，那 general chains 的 StorageVersion 可能跟 Avatect/Sand 不同（已確認：ECQ 確實不同）。

你確認所有 9 條 leafchain 未來都會用同一個 thxnet-sdk general-runtime 嗎？包括 Avatect 和 Sand？

> ANSWER: 確認，下次的 thxnet-sdk leafchains general runtime spec version 應該就是 4 了，統一！

Q2. THX 和 LMT mainnet leafchains 的重要性

THX mainnet 有 5,315 accounts + 90,353 NFTs，LMT mainnet 有 13,226 accounts + 79,483 NFTs。這比 Avatect 的 21 campaigns 和 38 RWA assets 的資料量大很多。

這些 NFTs 是由 pallet-nfts 管理的嗎？pallet-nfts 在 v0.9.40→v1.12.0 之間有 StorageVersion 變化嗎？ 我目前只看到 on-chain Nfts=v1, code target=v1，似乎安全。但需要你確認這些 NFTs 對用戶的重要程度。

> ANSWER: "pallet-nfts 在 v0.9.40→v1.12.0 之間有 StorageVersion 變化嗎" 是一個超級令人頭痛的問題，需要你的 full-power 協助掃描.
> NFTs 是超級無敵重要的資料，不能壞掉, 否則我們會滅亡.

Q3. Leafchain Treasury RemovePallet

所有 9 條 general leafchain 的 on-chain metadata 裡有 Treasury pallet，但 thxnet-sdk 的 construct_runtime! 沒有 Treasury。

這是故意移除 Treasury 的嗎？如果是，我需要加 RemovePallet<TreasuryPalletName> 到 leafchain migrations。

> ANSWER: 需要 Treasury pallet in v1.12.0 . 是我們不小心沒加上的, do not remove Treasury Pallet.

Q4. ECQ 特殊狀態

ECQ (mainnet + testnet) 的 DmpQueue 在 v1 而非 v2，XcmpQueue 在 v2 而非 v3。但 thxnet-sdk 的 leafchain migrations 已經有 XcmpQueue v2→v3→v4 chain，這OK。

但 DmpQueue v1→v2 沒有標準 migration。在 Sand 的 feat/trustless-agent-fix branch 裡有 AlignCumulusStorageVersions 自訂遷移處理這個。ECQ 是否可以安全地用同樣的方法？還是 ECQ 的 DmpQueue 有實際不同格式的資料？

> ANSWER: 針對不同的 chain , 我們在 runtime 需要 dmpqueue 的正確 migrations (for all kinds of our leafchains)
> 你能幫我去確認嗎？ about mainnet and testnet leafchain ecq dmpqueues, using ws rpc endpoints i provided.

Q5. 合併策略確認

基於我的分析，建議的方式是：

1. 從 origin/upgrade/1.12.0 (505618c8) checkout 一個新 branch
2. Cherry-pick 或 merge feat/production-port-v2 的 thxnet custom code
3. 重建完整的 cumulative migrations tuple (0.9.40 → 1.12.0)

而不是把 origin/upgrade/1.12.0 merge 進 feat/production-port-v2，因為 origin/upgrade/1.12.0 已經有正確的 v1.12.0 upstream code base。

你同意這個方向嗎？

> ANSWER: 那 commit b4016902ac7fc1d885eae236a2f71ddc58abc2f9 當下的時空背景也可以被納入到 "1. 的 一個新 branch" ?
> 如果可以的話，那我覺得沒問題

Q6. forked-testnet 的技術細節

你提到要用 forked testnet 測試。你打算用什麼工具 fork？ try-runtime? chopsticks? 直接用 --chain flag 載入 testnet chain spec 配合本地 state snapshot？

這影響我怎麼設計 migration 測試策略。

> ANSWER: forked testnet 是完全無關於任何工具 fork, 我是用 k8s 完整 reproduce 環境，但只是 network 被隔開，他們都還以為他們是原本的 testnet, 直到我一個個重新啟動 (using new blockchain node clients versions), 然後一步步 upgrade runtime 之後 test existing pallets things 等等.
> 當然，你可以幫助我們實現的，像是 try-runtime and chopsticks , 也可以.
> 你們需要 live net states 的話也可以去參考剛剛提供的 ws rpc endpoints
