// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Substrate block-author/full-node API.

pub mod error;
pub mod hash;

use error::Error;
use jsonrpsee::proc_macros::rpc;
use sc_transaction_pool_api::{TransactionPoolEvent, TransactionStatus};
use serde::{Deserialize, Serialize};
use sp_core::{Bytes, H256};

/// Version of the stable `thxnet_pendingExtrinsicsFull` response envelope.
pub const FULL_PENDING_EXTRINSICS_SCHEMA_VERSION: u32 = 1;

/// The pool queues are atomic, while the best hash is read from the client
/// immediately before that pool snapshot and is therefore attribution only.
pub const FULL_PENDING_BEST_HASH_LIMITATION: &str = "best_hash_observed_outside_pool_lock";

/// Reserved options for `thxnet_pendingExtrinsicsFull`.
///
/// The object is intentionally empty in schema v1. Accepting an explicit `{}`
/// gives the method an extensible parameter boundary, while
/// `deny_unknown_fields` prevents misspelled or imagined controls from being
/// silently treated as a complete snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FullPendingExtrinsicsOptions {}

/// Queue containing a transaction returned by `thxnet_pendingExtrinsicsFull`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FullPendingTransactionQueue {
	/// Transaction is ready for block inclusion.
	Ready,
	/// Transaction waits for one or more dependency tags.
	Future,
}

/// One transaction returned by `thxnet_pendingExtrinsicsFull`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FullPendingTransaction<Hash> {
	/// Transaction hash.
	pub hash: Hash,
	/// SCALE-encoded extrinsic bytes.
	pub extrinsic: Bytes,
	/// Queue containing the transaction.
	pub queue: FullPendingTransactionQueue,
	/// Pool priority.
	pub priority: u64,
	/// Remaining validation longevity as reported at import.
	pub longevity: u64,
	/// Dependency tags required by the transaction.
	pub requires: Vec<Bytes>,
	/// Tags provided by the transaction.
	pub provides: Vec<Bytes>,
	/// Whether the transaction may be propagated to peers.
	pub propagable: bool,
}

/// Stable, attributable full-pool response.
///
/// Ready and future are captured together under one pool read lock. The
/// independently observed best hash is bound into `snapshot_id`, but its
/// non-atomic relationship to the pool is stated in `limitations` rather than
/// hidden. The object shape is returned even when both queues are empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FullPendingExtrinsicsResponse<Hash, BlockHash> {
	/// Response schema version.
	pub schema_version: u32,
	/// Content identity over schema, best hash and every ordered transaction field.
	pub snapshot_id: H256,
	/// Best block hash observed immediately before the pool snapshot.
	pub best_hash: BlockHash,
	/// Transactions ready for block inclusion.
	pub ready: Vec<FullPendingTransaction<Hash>>,
	/// Transactions waiting for dependency tags.
	pub future: Vec<FullPendingTransaction<Hash>>,
	/// Explicit qualifications a client must retain with this evidence.
	pub limitations: Vec<String>,
}

/// Substrate authoring RPC API
#[rpc(client, server)]
pub trait AuthorApi<Hash, BlockHash> {
	/// Submit hex-encoded extrinsic for inclusion in block.
	#[method(name = "author_submitExtrinsic")]
	async fn submit_extrinsic(&self, extrinsic: Bytes) -> Result<Hash, Error>;

	/// Insert a key into the keystore.
	#[method(name = "author_insertKey")]
	fn insert_key(&self, key_type: String, suri: String, public: Bytes) -> Result<(), Error>;

	/// Generate new session keys and returns the corresponding public keys.
	#[method(name = "author_rotateKeys")]
	fn rotate_keys(&self) -> Result<Bytes, Error>;

	/// Checks if the keystore has private keys for the given session public keys.
	///
	/// `session_keys` is the SCALE encoded session keys object from the runtime.
	///
	/// Returns `true` iff all private keys could be found.
	#[method(name = "author_hasSessionKeys")]
	fn has_session_keys(&self, session_keys: Bytes) -> Result<bool, Error>;

	/// Checks if the keystore has private keys for the given public key and key type.
	///
	/// Returns `true` if a private key could be found.
	#[method(name = "author_hasKey")]
	fn has_key(&self, public_key: Bytes, key_type: String) -> Result<bool, Error>;

	/// Returns all pending extrinsics, potentially grouped by sender.
	#[method(name = "author_pendingExtrinsics")]
	fn pending_extrinsics(&self) -> Result<Vec<Bytes>, Error>;

	/// Returns every ready and future transaction with its queue and validity facts.
	#[method(name = "thxnet_pendingExtrinsicsFull")]
	fn pending_extrinsics_full(
		&self,
		options: Option<FullPendingExtrinsicsOptions>,
	) -> Result<FullPendingExtrinsicsResponse<Hash, BlockHash>, Error>;

	/// Remove given extrinsic from the pool and temporarily ban it to prevent reimporting.
	#[method(name = "author_removeExtrinsic")]
	fn remove_extrinsic(
		&self,
		bytes_or_hash: Vec<hash::ExtrinsicOrHash<Hash>>,
	) -> Result<Vec<Hash>, Error>;

	/// Submit an extrinsic to watch.
	///
	/// See [`TransactionStatus`](sc_transaction_pool_api::TransactionStatus) for details on
	/// transaction life cycle.
	#[subscription(
		name = "author_submitAndWatchExtrinsic" => "author_extrinsicUpdate",
		unsubscribe = "author_unwatchExtrinsic",
		item = TransactionStatus<Hash, BlockHash>,
	)]
	fn watch_extrinsic(&self, bytes: Bytes);

	/// Subscribe to sequenced transaction-pool lifecycle events.
	///
	/// `since_seq` is the last event already seen. When supplied, the subscription first replays
	/// every retained event after that cursor, then continues with live delivery.
	#[subscription(
		name = "thxnet_subscribeTxPoolEvents" => "thxnet_txPoolEvent",
		unsubscribe = "thxnet_unsubscribeTxPoolEvents",
		item = TransactionPoolEvent<Hash, BlockHash>,
	)]
	fn subscribe_tx_pool_events(&self, since_seq: Option<u64>);
}
