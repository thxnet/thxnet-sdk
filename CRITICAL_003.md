結論：發現並修復了 porting bug

問題所在

thxnet-sdk 的 rootchain runtime 兩個檔案在 porting 時沒有套用 thxnet 的 zero-fee 客製化，而是沿用了 upstream Polkadot 的有費用值：

┌──────────────────────────┬────────────────────────────────┬─────────────────────────────────────────────────────────────────┬──────────────────────────────────┐
│ │ rootchain（源頭） │ thxnet-sdk（修復前） │ thxnet-sdk（修復後） │
├──────────────────────────┼────────────────────────────────┼─────────────────────────────────────────────────────────────────┼──────────────────────────────────┤
│ TransactionByteFee │ TRANSACTION_BYTE_FEE = 0 │ 10 \* MILLICENTS ❌ │ TRANSACTION_BYTE_FEE = 0 ✓ │
├──────────────────────────┼────────────────────────────────┼─────────────────────────────────────────────────────────────────┼──────────────────────────────────┤
│ OperationalFeeMultiplier │ OPERATIONAL_FEE_MULTIPLIER = 0 │ 5 ❌ │ OPERATIONAL_FEE_MULTIPLIER = 0 ✓ │
├──────────────────────────┼────────────────────────────────┼─────────────────────────────────────────────────────────────────┼──────────────────────────────────┤
│ LengthToFee │ WeightToFee（= 0） │ ConstantMultiplier<Balance, TransactionByteFee>（= non-zero）❌ │ WeightToFee ✓ │
└──────────────────────────┴────────────────────────────────┴─────────────────────────────────────────────────────────────────┴──────────────────────────────────┘

修復的檔案

1. thxnet/runtime/thxnet-testnet/src/lib.rs
2. thxnet/runtime/thxnet/src/lib.rs

leafchain 狀況

thxnet/leafchain/runtime/general/src/lib.rs 雖然用 ConstantMultiplier<Balance, TransactionByteFee>（不是 WeightToFee），但因為 TransactionByteFee = TRANSACTION_BYTE_FEE = 0u128，計算結果仍然是零，所以 leafchain 沒有問題。
