use general_runtime::{AccountId, AuraId, Balance, UNITS, WASM_BINARY};
use hex_literal::hex;
use sc_chain_spec::Properties;
use sc_service::ChainType;
use sp_core::crypto::UncheckedInto;

use crate::chain_spec::{
	testnet::testnet_genesis_patch, ChainSpec, Extensions, ROOTCHAIN_TESTNET_NAME,
};

const ROOT_STASH: Balance = 10_000_000_000 * UNITS;
const LEAFCHAIN_ID: u32 = 1003;
const COLLATOR_STASH: Balance = 200 * UNITS;

pub fn testnet_config() -> ChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "SAND".into());
	properties.insert("tokenDecimals".into(), 10.into());
	properties.insert("ss58Format".into(), 42.into());

	// 5DevG7WYZjzG7e2ps5EdZn8jCuJfKptPXRDmi6sQZrDMRkWa
	let root_key =
		AccountId::from(hex!["4654556053222de86e826a7e6085fba5c8a23590ecea55c3971b73eeff22207b"]);

	let invulnerables: Vec<(AccountId, AuraId)> = vec![
		// a
		(
			// 5CGRnSvBHqaYZgCwUNJ6TZ6eRosrAWSY4i84zL6d6VH6QXVq
			AccountId::from(hex![
				"08f0d6cca8f427954d7bb85cac42f3d29de4b90a59d3cb1a1dd6b6732945200b"
			]),
			// 5EypmffvLe3rk2e28ydxi1USQSa6U5V9tphxZW1XZZhJYxoa
			hex!["80fbbf468a88629f81423b1a249d37199604a1e3d0b891958c3d172138bc4f41"]
				.unchecked_into(),
		),
		// b
		(
			// 5CnqeN2fYC7L9eH6s1kb47Mfn5JwkHdZCuMSnobr3q6CfB8q
			AccountId::from(hex![
				"202285ef6785e4597abee5e98ba7fd92521c93a5cbb48a43b8080b51a9f0be15"
			]),
			// 5CoGQqph2cEKoXh4yPQHyPfZ4mZ7JWkYWRrapNSJ5qej8N2i
			hex!["2075e55c8dcb72ebbbefa97d858c3b2c7669fde96947972b703c2e227ebce834"]
				.unchecked_into(),
		),
	];

	let wasm_binary = WASM_BINARY.expect("WASM binary was not built, please build it!");

	ChainSpec::builder(
		wasm_binary,
		Extensions { rootchain: ROOTCHAIN_TESTNET_NAME.to_string(), leafchain_id: LEAFCHAIN_ID },
	)
	.with_name("Sandbox")
	.with_id("sand_testnet")
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
