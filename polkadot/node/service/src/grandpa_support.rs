// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

//! Polkadot-specific GRANDPA integration utilities.

use sp_runtime::traits::{Block as BlockT, Header as _, NumberFor};

use crate::HeaderProvider;

#[cfg(feature = "full-node")]
use polkadot_primitives::{Block, Hash};

/// Returns the block hash of the block at the given `target_number` by walking
/// backwards from the given `current_header`.
pub(super) fn walk_backwards_to_target_block<Block, HP>(
	backend: &HP,
	target_number: NumberFor<Block>,
	current_header: &Block::Header,
) -> Result<(Block::Hash, NumberFor<Block>), sp_blockchain::Error>
where
	Block: BlockT,
	HP: HeaderProvider<Block>,
{
	let mut target_hash = current_header.hash();
	let mut target_header = current_header.clone();

	loop {
		if *target_header.number() < target_number {
			unreachable!(
				"we are traversing backwards from a known block; \
				 blocks are stored contiguously; \
				 qed"
			);
		}

		if *target_header.number() == target_number {
			return Ok((target_hash, target_number))
		}

		target_hash = *target_header.parent_hash();
		target_header = backend
			.header(target_hash)?
			.expect("Header known to exist due to the existence of one of its descendants; qed");
	}
}

/// GRANDPA hard forks due to borked migration of session keys after a runtime
/// upgrade (at #1491596), the signaled authority set changes were invalid
/// (blank keys) and were impossible to finalize. The authorities for these
/// intermediary pending changes are replaced with a static list comprised of
/// w3f validators and randomly selected validators from the latest session (at
/// #1500988).
#[cfg(feature = "full-node")]
pub(crate) fn kusama_hard_forks() -> Vec<sc_consensus_grandpa::AuthoritySetHardFork<Block>> {
	use sp_core::crypto::Ss58Codec;
	use std::str::FromStr;

	let forks = vec![
		(623, "01e94e1e7e9cf07b3b0bf4e1717fce7448e5563901c2ef2e3b8e9ecaeba088b1", 1492283),
		(624, "ddc4323c5e8966844dfaa87e0c2f74ef6b43115f17bf8e4ff38845a62d02b9a9", 1492436),
		(625, "38ba115b296663e424e32d7b1655cd795719cef4fd7d579271a6d01086cf1628", 1492586),
		(626, "f3172b6b8497c10fc772f5dada4eeb1f4c4919c97de9de2e1a439444d5a057ff", 1492955),
		(627, "b26526aea299e9d24af29fdacd5cf4751a663d24894e3d0a37833aa14c58424a", 1493338),
		(628, "3980d024327d53b8d01ef0d198a052cd058dd579508d8ed6283fe3614e0a3694", 1493913),
		(629, "31f22997a786c25ee677786373368cae6fd501fd1bc4b212b8e267235c88179d", 1495083),
		(630, "1c65eb250cf54b466c64f1a4003d1415a7ee275e49615450c0e0525179857eef", 1497404),
		(631, "9e44116467cc9d7e224e36487bf2cf571698cae16b25f54a7430f1278331fdd8", 1498598),
	];

	let authorities = vec![
		"CwjLJ1zPWK5Ao9WChAFp7rWGEgN3AyXXjTRPrqgm5WwBpoS",
		"Dp8FHpZTzvoKXztkfrUAkF6xNf6sjVU5ZLZ29NEGUazouou",
		"DtK7YfkhNWU6wEPF1dShsFdhtosVAuJPLkoGhKhG1r5LjKq",
		"FLnHYBuoyThzqJ45tdb8P6yMLdocM7ir27Pg1AnpYoygm1K",
		"FWEfJ5UMghr52UopgYjawAg6hQg3ztbQek75pfeRtLVi8pB",
		"ECoLHAu7HKWGTB9od82HAtequYj6hvNHigkGSB9g3ApxAwB",
		"GL1Tg3Uppo8GYL9NjKj4dWKcS6tW98REop9G5hpu7HgFwTa",
		"ExnjU5LZMktrgtQBE3An6FsQfvaKG1ukxPqwhJydgdgarmY",
		"CagLpgCBu5qJqYF2tpFX6BnU4yHvMGSjc7r3Ed1jY3tMbQt",
		"DsrtmMsD4ijh3n4uodxPoiW9NZ7v7no5wVvPVj8fL1dfrWB",
		"HQB4EctrVR68ozZDyBiRJzLRAEGh1YKgCkAsFjJcegL9RQA",
		"H2YTYbXTFkDY1cGnv164ecnDT3hsD2bQXtyiDbcQuXcQZUV",
		"H5WL8jXmbkCoEcLfvqJkbLUeGrDFsJiMXkhhRWn3joct1tE",
		"DpB37GDrJDYcmg2df2eqsrPKMay1u8hyZ6sQi2FuUiUeNLu",
		"FR8yjKRA9MTjvFGK8kfzrdC23Fr6xd7rfBvZXSjAsmuxURE",
		"DxHPty3B9fpj3duu6Gc6gCSCAvsydJHJEY5G3oVYT8S5BYJ",
		"DbVKC8ZJjevrhqSnZyJMMvmPL7oPPL4ed1roxawYnHVgyin",
		"DVJV81kab2J6oTyRJ9T3NCwW2DSrysbWCssvMcE6cwZHnAd",
		"Fg4rDAyzoVzf39Zo8JFPo4W314ntNWNwm3shr4xKe8M1fJg",
		"GUaNcnAruMVxHGTs7gGpSUpigRJboQYQBBQyPohkFcP6NMH",
		"J4BMGF4W9yWiJz4pkhQW73X6QMGpKUzmPppVnqzBCqw5dQq",
		"E1cR61L1tdDEop4WdWVqcq1H1x6VqsDpSHvFyUeC41uruVJ",
		"GoWLzBsj1f23YtdDpyntnvN1LwXKhF5TEeZvBeTVxofgWGR",
		"CwHwmbogSwtRbrkajVBNubPvWmHBGU4bhMido54M9CjuKZD",
		"FLT63y9oVXJnyiWMAL4RvWxsQx21Vymw9961Z7NRFmSG7rw",
		"FoQ2y6JuHuHTG4rHFL3f2hCxfJMvtrq8wwPWdv8tsdkcyA8",
		"D7QQKqqs8ocGorRA12h4QoBSHDia1DkHeXT4eMfjWQ483QH",
		"J6z7FP35F9DiiU985bhkDTS3WxyeTBeoo9MtLdLoD3GiWPj",
		"EjapydCK25AagodRbDECavHAy8yQY1tmeRhwUXhVWx4cFPv",
		"H8admATcRkGCrF1dTDDBCjQDsYjMkuPaN9YwR2mSCj4DWMQ",
		"FtHMRU1fxsoswJjBvyCGvECepC7gP2X77QbNpyikYSqqR6k",
		"DzY5gwr45GVRUFzRMmeg8iffpqYF47nm3XbJhmjG97FijaE",
		"D3HKWAihSUmg8HrfeFrftSwNK7no261yA9RNr3LUUdsuzuJ",
		"D82DwwGJGTcSvtB3SmNrZejnSertbPzpkYvDUp3ibScL3ne",
		"FTPxLXLQvMDQYFA6VqNLGwWPKhemMYP791XVj8TmDpFuV3b",
		"FzGfKmS7N8Z1tvCBU5JH1eBXZQ9pCtRNoMUnNVv38wZNq72",
		"GDfm1MyLAQ7Rh8YPtF6FtMweV4hz91zzeDy2sSABNNqAbmg",
		"DiVQbq7sozeKp7PXPM1HLFc2m7ih8oepKLRK99oBY3QZak1",
		"HErWh7D2RzrjWWB2fTJfcAejD9MJpadeWWZM2Wnk7LiNWfG",
		"Es4DbDauYZYyRJbr6VxrhdcM1iufP9GtdBYf3YtSEvdwNyb",
		"EBgXT6FaVo4WsN2LmfnB2jnpDFf4zay3E492RGSn6v1tY99",
		"Dr9Zg4fxZurexParztL9SezFeHsPwdP8uGgULeRMbk8DDHJ",
		"JEnSTZJpLh91cSryptj57RtFxq9xXqf4U5wBH3qoP91ZZhN",
		"DqtRkrmtPANa8wrYR7Ce2LxJxk2iNFtiCxv1cXbx54uqdTN",
		"GaxmF53xbuTFKopVEseWiaCTa8fC6f99n4YfW8MGPSPYX3s",
		"EiCesgkAaighBKMpwFSAUdvwE4mRjBjNmmd5fP6d4FG8DAx",
		"HVbwWGUx7kCgUGap1Mfcs37g6JAZ5qsfsM7TsDRcSqvfxmd",
		"G45bc8Ajrd6YSXav77gQwjjGoAsR2qiGd1aLzkMy7o1RLwd",
		"Cqix2rD93Mdf7ytg8tBavAig2TvhXPgPZ2mejQvkq7qgRPq",
		"GpodE2S5dPeVjzHB4Drm8R9rEwcQPtwAspXqCVz1ooFWf5K",
		"CwfmfRmzPKLj3ntSCejuVwYmQ1F9iZWY4meQrAVoJ2G8Kce",
		"Fhp5NPvutRCJ4Gx3G8vCYGaveGcU3KgTwfrn5Zr8sLSgwVx",
		"GeYRRPkyi23wSF3cJGjq82117fKJZUbWsAGimUnzb5RPbB1",
		"DzCJ4y5oT611dfKQwbBDVbtCfENTdMCjb4KGMU3Mq6nyUMu",
	];

	let authorities = authorities
		.into_iter()
		.map(|address| {
			(
				sp_consensus_grandpa::AuthorityId::from_ss58check(address)
					.expect("hard fork authority addresses are static and they should be carefully defined; qed."),
				1,
			)
		})
		.collect::<Vec<_>>();

	forks
		.into_iter()
		.map(|(set_id, hash, number)| {
			let hash = Hash::from_str(hash)
				.expect("hard fork hashes are static and they should be carefully defined; qed.");

			sc_consensus_grandpa::AuthoritySetHardFork {
				set_id,
				block: (hash, number),
				authorities: authorities.clone(),
				last_finalized: None,
			}
		})
		.collect()
}

/// Post-incident GRANDPA authorities for THX Network mainnet.
/// These are the 10 validators active after the finality deadlock incident area
/// (blocks 14,205,952 through 14,206,626).
#[cfg(feature = "full-node")]
pub(crate) fn thxnet_post_incident_authorities() -> Vec<(sp_consensus_grandpa::AuthorityId, u64)> {
	use sp_core::crypto::Ss58Codec;

	let addresses: Vec<&str> = vec![
		"5DQjEK2cWN2Qnp5sFdJQAoQ5RLaveyCxYpCbc8kWK2mbkrHi",
		"5CLCUaSjUhmukZEsp9bTgWi6gBDCMEVLXebN79U46q68Qzh1",
		"5FMYd9YVje234kxfCwZ5UmWoEQ6Zjz78GjjN3hQLM7SH3wDi",
		"5CNfCS5SZ6zEu9YtW1HKeyBxWibrwedgd6by4y9W1D2R1NbA",
		"5CKRFQnViKUtpyEmETsG2TxmzbWHDpGt9n9r1NWEVh9CU4RY",
		"5FW1LVeZKtrJB8RE3uWSEVsXSyFEkJA6PF5oEeKAnwi8cUMq",
		"5Dn9oyDjpcm6yp3bNRnsHEDgzxnkRgqvinChpt3WfZScjt48",
		"5FWBTpBSv4vCR4SC5Q5XT4zGvXF3cAT7AHfe7i45yRdUxwAL",
		"5Fv7rAvMJaKGEWJr1DNxhn5AeaiPot2TuHNvsCzAk9LyPDLR",
		"5ECrXnTf7R7W5wF8bv4xJiJYYyQUgnZfGy6uce4t36puANrT",
	];

	addresses
		.into_iter()
		.map(|address| {
			(
				sp_consensus_grandpa::AuthorityId::from_ss58check(address)
					.expect("hard fork authority addresses are static and they should be carefully defined; qed."),
				1,
			)
		})
		.collect()
}

/// GRANDPA hard forks for THX Network mainnet.
///
/// During the finality deadlock incident (2026-02-20), manual `setStorage`
/// interventions and forced authority changes created chaotic GRANDPA state
/// at blocks 14,205,952 through 14,206,626. Only 3 real ForcedChange consensus
/// logs exist in block headers (at 14,206,555, 14,206,564, 14,206,591).
/// The migration at block 14,206,625 incremented runtime set_id but did NOT
/// emit a ForcedChange log, causing a client/runtime set_id divergence.
///
/// These 4 hard fork entries use the GRANDPA CLIENT's internal set_id values
/// (987→988→989→990), each incrementing by exactly 1. After all 4, the client
/// reaches set_id=991, matching the healthy archive nodes on the network.
///
/// This is NOT a chain fork — block hashes and the canonical chain are unchanged.
/// Only the GRANDPA finality gadget's authority tracking is overridden.
#[cfg(feature = "full-node")]
pub(crate) fn thxnet_hard_forks() -> Vec<sc_consensus_grandpa::AuthoritySetHardFork<Block>> {
	use std::str::FromStr;

	// (client_set_id, block_hash, block_number)
	// client_set_id = the GRANDPA client's internal set_id WHEN that block is reached.
	// After processing all 4 entries: 987 + 4 = 991 (matches archive nodes).
	let forks: Vec<(u64, &str, u32)> = vec![
		// Override 1st real ForcedChange log (chaotic incident)
		(987, "9cd4f37aed551dbb8fc422dea295d832b5efffc0230007c86714cac444bd5cff", 14_206_555),
		// Override 2nd real ForcedChange log
		(988, "b24efda871e72649a6512d418e75b5e5e5921307ee04564042f3bd1cdd721d04", 14_206_564),
		// Override 3rd real ForcedChange log
		(989, "323b1605b3030e79bae563f64e5c7f5cad9147632a0230764744d6f04e190b9f", 14_206_591),
		// ADD missing ForcedChange (migration at 14,206,625 failed to emit log)
		(990, "9db27f4ec24dc50ca5c314a76f55384748ee6d0a1af3f719ec07166238a8200c", 14_206_626),
	];

	let authorities = thxnet_post_incident_authorities();

	forks
		.into_iter()
		.map(|(set_id, hash, number)| {
			let hash = Hash::from_str(hash)
				.expect("hard fork hashes are static and they should be carefully defined; qed.");

			sc_consensus_grandpa::AuthoritySetHardFork {
				set_id,
				block: (hash, number),
				authorities: authorities.clone(),
				last_finalized: Some(14_205_952),
			}
		})
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn thxnet_hard_forks_returns_4_entries() {
		let forks = thxnet_hard_forks();
		assert_eq!(forks.len(), 4, "must have exactly 4 hard fork entries");
	}

	#[test]
	fn thxnet_hard_fork_set_ids_are_sequential() {
		let forks = thxnet_hard_forks();
		let set_ids: Vec<u64> = forks.iter().map(|f| f.set_id).collect();
		assert_eq!(set_ids, vec![987, 988, 989, 990]);
	}

	#[test]
	fn thxnet_hard_fork_blocks_are_in_incident_area() {
		let forks = thxnet_hard_forks();
		for fork in &forks {
			let (_, number) = fork.block;
			assert!(
				number >= 14_206_555 && number <= 14_206_626,
				"block {} outside incident area 14206555-14206626",
				number
			);
		}
	}

	#[test]
	fn thxnet_hard_fork_last_finalized_is_pre_incident() {
		let forks = thxnet_hard_forks();
		for fork in &forks {
			assert_eq!(fork.last_finalized, Some(14_205_952));
		}
	}

	#[test]
	fn thxnet_post_incident_authorities_returns_10_validators() {
		let authorities = thxnet_post_incident_authorities();
		assert_eq!(authorities.len(), 10, "must have exactly 10 validators");
		// All weights must be 1
		for (_, weight) in &authorities {
			assert_eq!(*weight, 1);
		}
	}
}
