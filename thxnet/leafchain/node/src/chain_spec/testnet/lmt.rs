use general_runtime::{AccountId, AuraId, Balance, UNITS, WASM_BINARY};
use hex_literal::hex;
use sc_chain_spec::Properties;
use sc_service::ChainType;
use sp_core::crypto::UncheckedInto;

use crate::chain_spec::{
	testnet::testnet_genesis_patch, ChainSpec, Extensions, ROOTCHAIN_TESTNET_NAME,
};

const ROOT_STASH: Balance = 72_000_000_000 * UNITS;
const LEAFCHAIN_ID: u32 = 1001;
const COLLATOR_STASH: Balance = 200 * UNITS;

pub fn testnet_config() -> ChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "DEVLMT".into());
	properties.insert("tokenDecimals".into(), 10.into());
	properties.insert("ss58Format".into(), 42.into());

	// 5GcBPgD5CjoRdzaCZUqDYLMUqWz62qZhjzdwZi1543mk9sid
	let root_key =
		AccountId::from(hex!["c8f23b2c6ee09018ac747b790101e15cc69177a4db9f7f171966bb53ad2e651c"]);

	let invulnerables: Vec<(AccountId, AuraId)> = vec![
		// a
		(
			// 5EyCvP9TAzVVfydqiToqJ8U3kd7QcZ6YCWRUBb2C98vGNtxB
			AccountId::from(hex![
				"808310f1ad771f05ccf47ee9999ef5950f870d53deab369db13576d9a5375f65"
			]),
			// 5Gn4PzmKfUz7MQ9Kkk17rUQ73C2Hc3HQ754p1GUaxnwXCC24
			hex!["d07b23c0a999f4a15c72bc76dcfcfda6ad27b55755d17606891e3127b8771c32"]
				.unchecked_into(),
		),
		// b
		(
			// 5G3tnugL6GcMdFby88pCCAbgDzPWaXEuGr5raTJvqhNsKgnJ
			AccountId::from(hex![
				"b0527fdf0b795b7cc77eb7b5230c8b3d3d479fcb0c4f42a7dda84517a319170b"
			]),
			// 5E4XLBvG6TyAa8u8NaEoE8UKxxa33KfWs2xpuQkJhKakt9Kq
			hex!["585508ed89a7990205aa7a1ce6c3840407ffa68d509a976a5090f5f346ea8a37"]
				.unchecked_into(),
		),
	];

	let wasm_binary = WASM_BINARY.expect("WASM binary was not built, please build it!");

	ChainSpec::builder(
		wasm_binary,
		Extensions { rootchain: ROOTCHAIN_TESTNET_NAME.to_string(), leafchain_id: LEAFCHAIN_ID },
	)
	.with_name("LimiteT Testnet")
	.with_id("lmt_testnet")
	.with_chain_type(ChainType::Live)
	.with_genesis_config_patch(testnet_genesis_patch(
		Some(root_key.clone()),
		vec![(root_key, ROOT_STASH - (invulnerables.len() as u128) * COLLATOR_STASH)],
		invulnerables
			.iter()
			.map(|x| (x.0.clone(), COLLATOR_STASH, x.1.clone()))
			.collect(),
		LEAFCHAIN_ID.into(),
	))
	.with_properties(properties)
	.build()
}
