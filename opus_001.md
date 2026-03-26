好，我已經徹底消化了所有資料。讓我把整個局勢的心智模型整理清楚，然後問你精準的問題。

---

## 我理解的全貌

**現狀**: `feat/production-port-v2` 基於 polkadot-sdk v1.10.0，已經把 rootchain + leafchains (v0.9.40) 的 custom code port 過來了。但測試不及格，CRITICAL 分析揭露了 4 個 FATAL 級 migration gap。

**目標**: 基於 `b4016902ac` (polkadot-sdk v1.12.0) 重建一切，最終 tag 為 `thxnet-sdk-v1.12.0`。repo 裡已經有 v1.12.0 的 merge commit (`7a04d59605`)。

**部署順序**: rootchain binary → rootchain runtime WASM → leafchain binary → leafchain runtime WASM

**4 條活鏈**:
| 鏈 | spec_version | 關鍵狀態 |
|---|---|---|
| Mainnet Relay | 94000004 | 10 validators, ~14.68M blocks, Dao=40 |
| Testnet Relay | 94000004 | ~15.38M blocks, Dao=8538 |
| Mainnet Leaf (Avatect) | 3 | Rwa=38, CF=21, 無 TrustlessAgent |
| Testnet Leaf (Sand) | 3 | TrustlessAgent=12 agents, 無 Rwa/CF |

---

## 我的精準問題（按 criticality 排序）

### A. 版本跳躍 v1.10.0 → v1.12.0 的額外代價

**A1.** v1.12.0 merge message 提到新增了 `pallet_staking v14→v15` migration 和 `CollatorSelection v1→v2` migration。這意味著：

- Staking 的 migration chain 從原本的 `v13→v14` 變成 `v13→v14→v15`
- Leafchain CollatorSelection 從 `v0→v1` 變成 `v0→v1→v2`
- 可能還有 Configuration v12 以上的新版本

**你是否已經確認 v1.12.0 引入的所有新 pallet storage version 變化？還是需要我去 diff v1.10.0 和 v1.12.0 之間的所有 STORAGE_VERSION 變更？**

> ANSWER: you should diff all pallet storage version changes from 0.9.40 ~ 0.9.43 ~ 1.0.0 ~ all the way to 1.12 ; (0.9.40 ~ 1.0.0 的部分你可以參考 ~/Works/rootchain/ and ~/Works/polkadot/).
> 老實說, v1.12.0 應該要網羅所有必需的、累積的 migrations.

### B. GRANDPA Finality Fix

**B2.** rootchain 的 `FixGrandpaFinalityDeadlock` runtime migration 是寫在 spec_version `94000005` 裡的，而 live mainnet 在 `94000004`。這表示 **GRANDPA fix migration 從未在 on-chain 執行過**。

但 node-side 的 `thxnet_grandpa_recovery::reset_grandpa_state()` 和 4 個 hard fork entries 是在 node binary 層面的。

**問題**:

- 現在 mainnet finality 是正常的嗎？是靠 node-side fix 撐住的嗎？
- 當我們部署 thxnet-sdk v1.12.0 的 node binary 時，這些 hard fork entries 和 `reset_grandpa_state()` 需要帶過來嗎？（我看到 thxnet-sdk 已有 `thxnet_grandpa_recovery.rs` 檔案）
- `FixGrandpaFinalityDeadlock` runtime migration 還需要包含在新 runtime WASM 裡嗎？還是因為 node-side fix 已經解決了，runtime-level 的 migration 可以跳過？

> ANSWER: 94000005 的那個應該是有包含到 pallet finality-rescue 的那個 new pallet , 其實 thxnet mainnet rootchain 是透過 blockchain node client 層次來解決了 grandpa authority set 的問題的，現在 live mainnet rootchain 活得很好，從 0 開始 sync 都不會有問題.
> hard fork entries 和 `reset_grandpa_state()` and `FixGrandpaFinalityDeadlock` runtime migration 需要帶過來, 或者換句話說，總之就是 "commit acf5cbd28875d09240cea16bcb0847d543589cbb ~ 682594173b80983c581fb2829629bc3f9ae3dd72 在 ~/Works/rootchain/ repo 中的 commits" 的「效果」要帶過來

### C. 多條 Leafchain 的問題

**C3.** chain spec 目錄顯示 mainnet 有 6 條 leafchains (activa, avatect, ecq, lmt, mirrored_body, thx)，testnet 有 9 條。

**除了 Avatect (mainnet) 和 Sand (testnet) 之外，還有其他 leafchain 是 live 的、有真實用戶資料的嗎？還是只有這兩條是 live 的？**

> ANSWER:
> live mainnet/testnet's leafchains (parachains) 的還有 mainnet leafchain thx, lmt, ecq ; testnet leafchain thx, lmt, izutsuya, ecq
>
> - wss://node.thx.mainnet.thxnet.org/archive-001/ws
> - wss://node.lmt.mainnet.thxnet.org/archive-001/ws
> - wss://node.ecq.mainnet.thxnet.org/archive-001/ws
> - wss://node.thx.testnet.thxnet.org/archive-001/ws
> - wss://node.lmt.testnet.thxnet.org/archive-001/ws
> - wss://node.izutsuya.testnet.thxnet.org/archive-001/ws
> - wss://node.ecq.testnet.thxnet.org/archive-001/ws

### D. 統一 Leafchain Runtime 的風險

**D4.** thxnet-sdk 用一個 `general-runtime` 涵蓋所有 leafchain。但：

- Avatect (mainnet) 有 Rwa(40) + Crowdfunding(41)，**沒有** TrustlessAgent、Identity
- Sand (testnet) 有 TrustlessAgent(27) + Identity(28)，**沒有** Rwa、Crowdfunding

統一 runtime 包含了 **所有** pallets。這代表：

- Avatect 升級後會多出 TrustlessAgent(27) 和 Identity(28) — 空 storage，OK
- Sand 升級後會多出 Rwa(40) 和 Crowdfunding(41) — 空 storage

**但 Crowdfunding 的 MigrateToV3 會在 Sand 上觸發嗎？Sand 的 Crowdfunding on-chain 是 v0（因為 pallet 不存在 = StorageVersion reads 0），migration guard 是 `==2`，所以會 skip。但 in-code 是 v3。try-runtime 會 fail 嗎？**

也就是說：**對於「從未存在過」的 pallet，升級後 on-chain StorageVersion 是 0，in-code 是 v3/v5 — 這個 mismatch 要怎麼處理？**

> ANSWER:
> 我希望 leafchains general runtime 將會有 pallet rwa,crowdfunding,trustlessAgent,Identity
> 並且在所有所有 thxnet mainnet/testnet leafchains 上面可以正常運作 (也就是 migrations 都要得能支援各種 mainnet/testnet leafchains).
> 我們反正會用 sudo.sudoUncheckedWeight(system.setCode()) for leafchains runtime upgrade.s

### E. 部署戰略

**E5.** 你提到「先 binary 再 runtime」的順序。但我想確認：

- **testnet 可以先做嗎？** 先在 testnet relay + testnet Sand 上跑完整個升級流程，驗證所有 migration，然後再打 mainnet？
- **如果 testnet 先做，testnet 的 Dao 有 8538 entries（vs mainnet 只有 40）— 這反而是更嚴格的測試，因為資料量更大？**

> ANSWER: testnet 會是我們希望先做的，而且我們會準備 forked-testnet (live testnet 完全不受影響), 來測試這一切 (0.9.40 ->>>>>> 1.12.0)

### F. spec_version 94000004 已有的 Migrations

**F6.** Live 鏈在 94000004。rootchain repo 的 94000005 migrations 包含：

- `NominationPools::migration::v4::MigrateToV4`
- `FixGrandpaFinalityDeadlock` (mainnet only)

**那 94000004 runtime 裡面的 migrations 是什麼？是否 NominationPools v4 migration 已經在 94000004 run 過了？（因為 CRITICAL_001 顯示 on-chain NominationPools 是 v4，如果 v4 migration 在 94000004 run 過，那就是 v4 migration 成功了）**

> ANSWER: 我不知道，你去查驗, 這真的需要你的幫忙.

### G. FinalityRescue Pallet

**G7.** thxnet-sdk 有 `pallet-finality-rescue` 在 `thxnet/pallets/finality-rescue/` 但 **沒有** 放進 `construct_runtime!`。

- 這是故意的嗎？未來需要放回去嗎？
- 老 rootchain 的 `construct_runtime!` 有 `FinalityRescue = 135`，但因為 94000005 從未上鏈，所以 on-chain 沒有 FinalityRescue 的 storage prefix

> ANSWER: 我不知道，你去查驗, 這真的需要你的幫忙. 我猜很有可能是失誤，但我們還沒有 setCode 94000005, 所以應該可以把 pallet finality-rescue 當作全新 pallet 放進 1.12.0 (當然，可能裡面的邏輯要大改, i don't know)
> finality-rescue pallet 當然是 for mainnet rootchain and testnet rootchain.
> 但是特殊的 blockchain node client level 的 grandpa fix 只有專屬給 mainnet rootchain.

### H. v1.12.0 Branching 策略

**H8.** Repo 裡已經有 v1.12.0 merge (`7a04d59605`)。你的計畫是：

- (a) 從 `7a04d59605` (v1.12.0 merge) 開一個新 branch，重新 port thxnet custom code？
- (b) 把 `7a04d59605` merge 進 `feat/production-port-v2`？
- (c) 從 `b4016902ac` (upstream v1.12.0) 開一個全新的 branch，從頭 port？
- (d) 其他？

**你偏好哪種？考慮到 feat/production-port-v2 已有的 port 工作（46,409 行新增），我需要知道你想保留多少。**

> ANSWER: Raj 做了很多工作，我不想浪費他的意志，我覺得應該要同時參考 commit 505618c8a767a182a6a8cde76e4fa49f7ae68dbb and b4016902ac7fc1d885eae236a2f71ddc58abc2f9,  
> 有點像是我們的 branch feat/production-port-v2 要獲得來自 branch origin/upgrade/1.12.0 and commit b4016902ac7fc1d885eae236a2f71ddc58abc2f9 的注入 (origin/upgrade/1.12.0 應該有多不少 codes, 所以你要想一下 merge 的順序).
> 因為我相信 branch feat/production-port-v2 目前應該是殘破不堪的
