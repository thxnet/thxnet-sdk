請仔細聽我說，現在我們在 endgame, 很多事情可以一次解決，但會是一條很驚險的道路。

first, @TMP_PLAN.md 會是你要開始基礎閱讀的部分，我們打算要把 ~/Works/rootchain/ and ~/Works/leafchains/ 中的 custom pallets, custom grandpa fix for thxnet_mainnet (or mainnet_thxnet or thxnet_main, 總之就是 mainnet rootchain) and biz logic (such as, tx fee = 0) 的東西帶來 thxnet-sdk 這個專案 (他立基於 polkadot-sdk repo)

但 @TMP_PLAN.md 遠遠不夠.

但是呢，由於各種奇奇怪怪的趕工，我不覺得現在的版本很安全，例如我們想要把 ~/Works/rootchain/ and ~/Works/leafchains/ (0.9.40 substrate blockchain node client and runtime) 要一次升級到 v1.12.0 polkadot-sdk (thxnet-sdk 也將會叫做 1.12.0)
你去看 commit b4016902ac7fc1d885eae236a2f71ddc58abc2f9 的話，就是 polkadot-sdk 當年 1.12.0 的樣子, 我們想要把我們需要的各種機能與 livenet (mainnet and testnet, 等一下會補充) 的狀態整個套在 1.12.0 上.

然後你再去閱讀 @CRITICAL\_\*.md, 裡面有很多可怕的細節被抓出，你要明白，現在目前的 thxnet-sdk 中的測試都是很不及格的.

live mainnet and live testnet 在 (source codes: ~/Works/rootchain/ is the rootchain repo, ~/Works/leafchains/ is the leafchains' repo):

- mainnet rootchain (relaychain): `wss://node.mainnet.thxnet.org/archive-001/ws`
- one of the testnet leafchains, mainnet leafchan avatect (parachain) (it has rwa and crowdfunding pallets things): `wss://node.avatect.mainnet.thxnet.org/archive-001/ws`
- testnet rootchain (relaychain): `wss://node.testnet.thxnet.org/archive-001/ws`
- one of the tesnet leafchains, testnet leafchain sand (parachain) (it has trustless-agent pallets things): `wss://node.sand.testnet.thxnet.org/archive-001/ws`

假設我們得到了一個 end-user, exsiting data , existing assets 都不會受到影響的 nice v1.12.0 thxnet-sdk (實際程式碼會是 based on commit b4016902ac7fc1d885eae236a2f71ddc58abc2f9 然後加上一堆我們需要的調整 , commit b4016902ac7fc1d885eae236a2f71ddc58abc2f9 就是當年的 polkadot-sdk's 1.12.0) 了之後，
我們有認知到 thxnet mainnet/testnet rootchain blockchain node clients 可以先更新之後 (因為它涵蓋了所有 0.9.40 ~ 1.12.0 的 old to new host functions and 各種 special fixes, including grandpa things in ~/Works/rootchain/, ~/Works/leafchains/ 其實本來要用 ~/Works/rootchain/ 的 blockchain node client 作為 embed relaychain , 但我們反正都要用 polkadot-sdk 了，也就是 thxnet-sdk).
然後我們可以再升級一個超級完整的 runtime wasm 是包含所有所有 pallet migrations (一路從 0.9.40 -> ~/Works/rootchain/ runtime 有的 + ~/Works/leafchains/ runtime 有的 -> 1.x.y -> 1.12.0)

然後再處理 mainnet/testnet leafchains blockchain node clients? and then leafchain runtime wasm ?

你就知道, 假定我們做了一個超強絕地大反攻成功之後，thxnet-sdk repo 將會有一個 tag 叫做 thxnet-sdk-v1.12.0, 那他就是等於基於 commit b4016902ac7fc1d885eae236a2f71ddc58abc2f9 + 各種 thxnet mainnet rootchain,leafchains + thxnet testnet rootchain,leafchains 需要的所有東西

你必須要好好 ultrathink, ultra-triangulate, ultra-scrutinize. 徹底搞清楚狀況後，問我問題，愈細節愈好，我會詳細地回答你。

請仔細聽我說，現在我們在 endgame, 很多事情可以一次解決，但會是一條很驚險的道路。

first, @TMP_PLAN.md 會是你要開始基礎閱讀的部分，我們打算要把 ~/Works/rootchain/ and ~/Works/leafchains/ 中的 custom pallets, custom grandpa fix for thxnet_mainnet (or mainnet_thxnet or thxnet_main, 總之就是 mainnet rootchain) and biz logic (such as, tx fee = 0) 的東西帶來 thxnet-sdk 這個專案 (他立基於 polkadot-sdk repo)

但 @TMP_PLAN.md 遠遠不夠.

但是呢，由於各種奇奇怪怪的趕工，我不覺得現在的版本很安全，例如我們想要把 ~/Works/rootchain/ and ~/Works/leafchains/ (0.9.40 substrate blockchain node client and runtime) 要一次升級到 v1.12.0 polkadot-sdk (thxnet-sdk 也將會叫做 1.12.0)
你去看 commit b4016902ac7fc1d885eae236a2f71ddc58abc2f9 的話，就是 polkadot-sdk 當年 1.12.0 的樣子, 我們想要把我們需要的各種機能與 livenet (mainnet and testnet, 等一下會補充) 的狀態整個套在 1.12.0 上.

然後你再去閱讀 @CRITICAL\_\*.md, 裡面有很多可怕的細節被抓出，你要明白，現在目前的 thxnet-sdk 中的測試都是很不及格的.

live mainnet and live testnet 在 (source codes: ~/Works/rootchain/ is the rootchain repo, ~/Works/leafchains/ is the leafchains' repo):

- mainnet rootchain (relaychain): `wss://node.mainnet.thxnet.org/archive-001/ws`
- one of the testnet leafchains, mainnet leafchan avatect (parachain) (it has rwa and crowdfunding pallets things): `wss://node.avatect.mainnet.thxnet.org/archive-001/ws`
- testnet rootchain (relaychain): `wss://node.testnet.thxnet.org/archive-001/ws`
- one of the tesnet leafchains, testnet leafchain sand (parachain) (it has trustless-agent pallets things): `wss://node.sand.testnet.thxnet.org/archive-001/ws`

假設我們得到了一個 end-user, exsiting data , existing assets 都不會受到影響的 nice v1.12.0 thxnet-sdk (實際程式碼會是 based on commit b4016902ac7fc1d885eae236a2f71ddc58abc2f9 然後加上一堆我們需要的調整 , commit b4016902ac7fc1d885eae236a2f71ddc58abc2f9 就是當年的 polkadot-sdk's 1.12.0) 了之後，
我們有認知到 thxnet mainnet/testnet rootchain blockchain node clients 可以先更新之後 (因為它涵蓋了所有 0.9.40 ~ 1.12.0 的 old to new host functions and 各種 special fixes, including grandpa things in ~/Works/rootchain/, ~/Works/leafchains/ 其實本來要用 ~/Works/rootchain/ 的 blockchain node client 作為 embed relaychain , 但我們反正都要用 polkadot-sdk 了，也就是 thxnet-sdk).
然後我們可以再升級一個超級完整的 runtime wasm 是包含所有所有 pallet migrations (一路從 0.9.40 -> ~/Works/rootchain/ runtime 有的 + ~/Works/leafchains/ runtime 有的 -> 1.x.y -> 1.12.0)

然後再處理 mainnet/testnet leafchains blockchain node clients? and then leafchain runtime wasm ?

你就知道, 假定我們做了一個超強絕地大反攻成功之後，thxnet-sdk repo 將會有一個 tag 叫做 thxnet-sdk-v1.12.0, 那他就是等於基於 commit b4016902ac7fc1d885eae236a2f71ddc58abc2f9 + 各種 thxnet mainnet rootchain,leafchains + thxnet testnet rootchain,leafchains 需要的所有東西

你必須要好好 ultrathink, ultra-triangulate, ultra-scrutinize. 徹底搞清楚狀況後，問我問題，愈細節愈好，我會詳細地回答你。
