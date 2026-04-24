pub mod aether;
pub mod ecq;
pub mod izutsuya;
pub mod lmt;
pub mod mirrored_body;
pub mod sand;
pub mod thx;
pub mod txd;

use cumulus_primitives_core::ParaId;
use general_runtime::{AccountId, AuraId, Balance, UNITS};

use crate::chain_spec::SAFE_XCM_VERSION;

fn testnet_genesis_patch(
	root_key: Option<AccountId>,
	endowed_accounts: Vec<(AccountId, Balance)>,
	invulnerables: Vec<(AccountId, Balance, AuraId)>,
	id: ParaId,
) -> serde_json::Value {
	let balances: Vec<(AccountId, Balance)> = endowed_accounts
		.iter()
		.map(|x| (x.0.clone(), x.1))
		.chain(invulnerables.iter().clone().map(|k| (k.0.clone(), k.1)))
		.collect();

	let session_keys: Vec<_> = invulnerables
		.into_iter()
		.map(|(acc, _, aura)| {
			(
				acc.clone(),                           // account id
				acc,                                   // validator id
				general_runtime::SessionKeys { aura }, // session keys
			)
		})
		.collect();

	serde_json::json!({
		"balances": {
			"balances": balances,
		},
		"parachainInfo": {
			"parachainId": id,
		},
		"collatorSelection": {
			"invulnerables": session_keys.iter().map(|(acc, _, _)| acc).collect::<Vec<_>>(),
			"candidacyBond": 100 * UNITS,
		},
		"session": {
			"keys": session_keys,
		},
		"polkadotXcm": {
			"safeXcmVersion": Some(SAFE_XCM_VERSION),
		},
		"sudo": {
			"key": root_key,
		},
	})
}
