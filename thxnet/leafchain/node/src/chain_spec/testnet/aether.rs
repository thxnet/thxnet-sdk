use general_runtime::{AccountId, AuraId, Balance, UNITS, WASM_BINARY};
use hex_literal::hex;
use sc_chain_spec::Properties;
use sc_service::ChainType;
use sp_core::crypto::UncheckedInto;

use crate::chain_spec::{
	testnet::testnet_genesis_patch, ChainSpec, Extensions, ROOTCHAIN_TESTNET_NAME,
};

const ROOT_STASH: Balance = 1_000_000_000 * UNITS;
const LEAFCHAIN_ID: u32 = 1004;
const COLLATOR_STASH: Balance = 200 * UNITS;

pub fn testnet_config() -> ChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "AETH".into());
	properties.insert("tokenDecimals".into(), 10.into());
	properties.insert("ss58Format".into(), 42.into());

	// 5CvzdjCjHQDaDnNGXYN3k2EDTcfbB35VCcMquaRsVFtSoLq3
	let root_key =
		AccountId::from(hex!["265abf0b6e9a925103d90b42c7127c16e10d04196f63608b32c757094d27d660"]);

	let invulnerables: Vec<(AccountId, AuraId)> = vec![
		// a
		(
			// 5D4x2qz8W7YwEthAv3aAKXAomFAU9KYhebFw6dC5uSgmYNET
			AccountId::from(hex![
				"2c6bf3ca165b86fe08b8f7905017ed3c7aef8c5b969a9643a735ece1268bf04a"
			]),
			// 5Hn2iazfiz1PkU3K8UX6JHs4zksxbVuxhtaGmrJWVSPTyX6D
			hex!["fcb1b82fc183cdce6805dc2b5deda69777498a27ed657d4276f40e1ecf93105f"]
				.unchecked_into(),
		),
		// b
		(
			// 5F1bqjaGNrXZoVne4f1tYCXex8pRJwLdegJWRzpVCRsDDpiL
			AccountId::from(hex![
				"8256b64eeaa20b33171601496c5cc49560f89d603d7983119df89dc725d7d31b"
			]),
			// 5EhCRvpNm5UHLpvhQMW4b7AwSDz7yqH6of5mKERT35BUJqvN
			hex!["744d7cfea6352a82a426dd925ac6a1d6d21fba715057f62245bdb71a6080f270"]
				.unchecked_into(),
		),
	];

	let wasm_binary = WASM_BINARY.expect("WASM binary was not built, please build it!");

	ChainSpec::builder(
		wasm_binary,
		Extensions { rootchain: ROOTCHAIN_TESTNET_NAME.to_string(), leafchain_id: LEAFCHAIN_ID },
	)
	.with_name("Aether")
	.with_id("aether_testnet")
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
