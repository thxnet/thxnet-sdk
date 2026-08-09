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

use std::{
	collections::{HashMap, VecDeque},
	fmt::Debug,
	hash,
};

use crate::LOG_TARGET;
use futures::channel::mpsc::{channel, Receiver, Sender};
use linked_hash_map::LinkedHashMap;
use log::{debug, trace};
use sc_transaction_pool_api::{
	TransactionPoolEvent, TransactionPoolEventKind, TransactionPoolEventReason,
	TransactionPoolEventStreamError,
};
use serde::Serialize;
use sp_runtime::traits;

use super::{watcher, BlockHash, ChainApi, ExtrinsicHash};

/// Extrinsic pool default listener.
pub struct Listener<H: hash::Hash + Eq, C: ChainApi> {
	watchers: HashMap<H, watcher::Sender<H, ExtrinsicHash<C>>>,
	finality_watchers: LinkedHashMap<ExtrinsicHash<C>, Vec<H>>,
	events: EventJournal<H, BlockHash<C>>,
}

/// Maximum number of blocks awaiting finality at any time.
const MAX_FINALITY_WATCHERS: usize = 512;
/// Maximum number of transaction-pool events available for cursor replay.
const TXPOOL_EVENT_HISTORY_LIMIT: usize = 4096;
/// Extra channel capacity reserved for live events after replay registration.
const TXPOOL_EVENT_LIVE_BUFFER: usize = 1024;

struct EventJournal<H, BH> {
	next_seq: u64,
	history: VecDeque<TransactionPoolEvent<H, BH>>,
	sinks: Vec<Sender<TransactionPoolEvent<H, BH>>>,
}

impl<H, BH> Default for EventJournal<H, BH> {
	fn default() -> Self {
		Self { next_seq: 1, history: VecDeque::new(), sinks: Vec::new() }
	}
}

impl<H: Clone + Debug, BH: Clone> EventJournal<H, BH> {
	fn latest_seq(&self) -> u64 {
		self.next_seq.saturating_sub(1)
	}

	fn record(&mut self, mut event: TransactionPoolEvent<H, BH>) {
		event.seq = self.next_seq;
		self.next_seq = self
			.next_seq
			.checked_add(1)
			.expect("a process cannot emit u64::MAX transaction-pool events; qed");

		debug!(
			target: LOG_TARGET,
			"[{:?}] THXNET. transaction-pool event kind={:?} seq={}",
			event.tx_hash,
			event.kind,
			event.seq,
		);
		self.history.push_back(event.clone());
		if self.history.len() > TXPOOL_EVENT_HISTORY_LIMIT {
			self.history.pop_front();
		}

		self.sinks.retain_mut(|sink| match sink.try_send(event.clone()) {
			Ok(()) => true,
			Err(error) if error.is_full() => {
				log::warn!(
					target: LOG_TARGET,
					"THXNET. transaction-pool event subscriber fell behind at seq={}; disconnecting for cursor replay",
					event.seq,
				);
				false
			},
			Err(_) => false,
		});
	}

	fn subscribe(
		&mut self,
		since_seq: Option<u64>,
	) -> Result<Receiver<TransactionPoolEvent<H, BH>>, TransactionPoolEventStreamError> {
		let latest_seq = self.latest_seq();
		if let Some(requested_seq) = since_seq {
			if requested_seq > latest_seq {
				return Err(TransactionPoolEventStreamError::CursorAhead {
					requested_seq,
					latest_seq,
				})
			}
			if let Some(oldest) = self.history.front() {
				if requested_seq < oldest.seq.saturating_sub(1) {
					return Err(TransactionPoolEventStreamError::CursorExpired {
						requested_seq,
						oldest_seq: oldest.seq,
						latest_seq,
					})
				}
			}
		}

		let (mut sink, stream) = channel(TXPOOL_EVENT_HISTORY_LIMIT + TXPOOL_EVENT_LIVE_BUFFER);
		if let Some(requested_seq) = since_seq {
			for event in self.history.iter().filter(|event| event.seq > requested_seq) {
				sink.try_send(event.clone())
					.expect("replay capacity exceeds retained event history; qed");
			}
		}
		self.sinks.push(sink);
		Ok(stream)
	}
}

impl<H: hash::Hash + Eq + Debug, C: ChainApi> Default for Listener<H, C> {
	fn default() -> Self {
		Self {
			watchers: Default::default(),
			finality_watchers: Default::default(),
			events: Default::default(),
		}
	}
}

impl<H: hash::Hash + traits::Member + Serialize, C: ChainApi> Listener<H, C> {
	fn record(
		&mut self,
		tx_hash: H,
		kind: TransactionPoolEventKind,
		reason: Option<TransactionPoolEventReason>,
		replacement_hash: Option<H>,
		block_hash: Option<BlockHash<C>>,
		block_index: Option<usize>,
		peers: Vec<String>,
	) {
		self.events.record(TransactionPoolEvent {
			seq: 0,
			tx_hash,
			kind,
			reason,
			replacement_hash,
			block_hash,
			block_index,
			peers,
		});
	}

	/// Open a race-free replay-plus-live event stream.
	pub fn event_stream(
		&mut self,
		since_seq: Option<u64>,
	) -> Result<Receiver<TransactionPoolEvent<H, BlockHash<C>>>, TransactionPoolEventStreamError> {
		self.events.subscribe(since_seq)
	}

	/// Record that a transaction was accepted by the pool.
	pub fn imported(&mut self, tx: &H) {
		self.record(
			tx.clone(),
			TransactionPoolEventKind::Imported,
			None,
			None,
			None,
			None,
			Vec::new(),
		);
	}

	fn fire<F>(&mut self, hash: &H, fun: F)
	where
		F: FnOnce(&mut watcher::Sender<H, ExtrinsicHash<C>>),
	{
		let clean = if let Some(h) = self.watchers.get_mut(hash) {
			fun(h);
			h.is_done()
		} else {
			false
		};

		if clean {
			self.watchers.remove(hash);
		}
	}

	/// Creates a new watcher for given verified extrinsic.
	///
	/// The watcher can be used to subscribe to life-cycle events of that extrinsic.
	pub fn create_watcher(&mut self, hash: H) -> watcher::Watcher<H, ExtrinsicHash<C>> {
		let sender = self.watchers.entry(hash.clone()).or_insert_with(watcher::Sender::default);
		sender.new_watcher(hash)
	}

	/// Notify the listeners about extrinsic broadcast.
	pub fn broadcasted(&mut self, hash: &H, peers: Vec<String>) {
		trace!(target: LOG_TARGET, "[{:?}] Broadcasted", hash);
		self.record(
			hash.clone(),
			TransactionPoolEventKind::Broadcast,
			None,
			None,
			None,
			None,
			peers.clone(),
		);
		self.fire(hash, |watcher| watcher.broadcast(peers));
	}

	/// New transaction was added to the ready pool or promoted from the future pool.
	pub fn ready(&mut self, tx: &H, old: Option<&H>) {
		trace!(target: LOG_TARGET, "[{:?}] Ready (replaced with {:?})", tx, old);
		self.record(
			tx.clone(),
			TransactionPoolEventKind::Ready,
			None,
			None,
			None,
			None,
			Vec::new(),
		);
		self.fire(tx, |watcher| watcher.ready());
		if let Some(old) = old {
			self.record(
				old.clone(),
				TransactionPoolEventKind::Evicted,
				Some(TransactionPoolEventReason::Usurped),
				Some(tx.clone()),
				None,
				None,
				Vec::new(),
			);
			self.fire(old, |watcher| watcher.usurped(tx.clone()));
		}
	}

	/// New transaction was added to the future pool.
	pub fn future(&mut self, tx: &H) {
		trace!(target: LOG_TARGET, "[{:?}] Future", tx);
		self.record(
			tx.clone(),
			TransactionPoolEventKind::Future,
			None,
			None,
			None,
			None,
			Vec::new(),
		);
		self.fire(tx, |watcher| watcher.future());
	}

	/// Transaction was dropped from the pool because of the limit.
	pub fn dropped(&mut self, tx: &H, by: Option<&H>) {
		trace!(target: LOG_TARGET, "[{:?}] Dropped (replaced with {:?})", tx, by);
		self.record(
			tx.clone(),
			TransactionPoolEventKind::Evicted,
			Some(if by.is_some() {
				TransactionPoolEventReason::Usurped
			} else {
				TransactionPoolEventReason::Dropped
			}),
			by.cloned(),
			None,
			None,
			Vec::new(),
		);
		self.fire(tx, |watcher| match by {
			Some(t) => watcher.usurped(t.clone()),
			None => watcher.dropped(),
		})
	}

	/// Transaction was removed as invalid.
	pub fn invalid(&mut self, tx: &H) {
		debug!(target: LOG_TARGET, "[{:?}] Extrinsic invalid", tx);
		self.record(
			tx.clone(),
			TransactionPoolEventKind::Evicted,
			Some(TransactionPoolEventReason::Invalid),
			None,
			None,
			None,
			Vec::new(),
		);
		self.fire(tx, |watcher| watcher.invalid());
	}

	/// Transaction was pruned from the pool.
	pub fn pruned(&mut self, block_hash: BlockHash<C>, tx: &H) {
		debug!(target: LOG_TARGET, "[{:?}] Pruned at {:?}", tx, block_hash);
		// Get the transactions included in the given block hash.
		let tx_index = {
			let txs = self.finality_watchers.entry(block_hash).or_insert(vec![]);
			txs.push(tx.clone());
			// Current transaction is the last one included.
			txs.len() - 1
		};
		self.record(
			tx.clone(),
			TransactionPoolEventKind::Pruned,
			None,
			None,
			Some(block_hash),
			Some(tx_index),
			Vec::new(),
		);

		self.fire(tx, |watcher| watcher.in_block(block_hash, tx_index));

		while self.finality_watchers.len() > MAX_FINALITY_WATCHERS {
			if let Some((hash, txs)) = self.finality_watchers.pop_front() {
				for tx in txs {
					self.record(
						tx.clone(),
						TransactionPoolEventKind::FinalityTimeout,
						None,
						None,
						Some(hash),
						None,
						Vec::new(),
					);
					self.fire(&tx, |watcher| watcher.finality_timeout(hash));
				}
			}
		}
	}

	/// The block this transaction was included in has been retracted.
	pub fn retracted(&mut self, block_hash: BlockHash<C>) {
		if let Some(hashes) = self.finality_watchers.remove(&block_hash) {
			for hash in hashes {
				self.record(
					hash.clone(),
					TransactionPoolEventKind::Retracted,
					None,
					None,
					Some(block_hash),
					None,
					Vec::new(),
				);
				self.fire(&hash, |watcher| watcher.retracted(block_hash))
			}
		}
	}

	/// Notify all watchers that transactions have been finalized
	pub fn finalized(&mut self, block_hash: BlockHash<C>) {
		if let Some(hashes) = self.finality_watchers.remove(&block_hash) {
			for (tx_index, hash) in hashes.into_iter().enumerate() {
				log::debug!(
					target: LOG_TARGET,
					"[{:?}] Sent finalization event (block {:?})",
					hash,
					block_hash,
				);
				self.record(
					hash.clone(),
					TransactionPoolEventKind::Finalized,
					None,
					None,
					Some(block_hash),
					Some(tx_index),
					Vec::new(),
				);
				self.fire(&hash, |watcher| watcher.finalized(block_hash, tx_index))
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use futures::{executor::block_on, StreamExt};

	fn event(hash: u64, kind: TransactionPoolEventKind) -> TransactionPoolEvent<u64, u64> {
		TransactionPoolEvent {
			seq: 0,
			tx_hash: hash,
			kind,
			reason: None,
			replacement_hash: None,
			block_hash: None,
			block_index: None,
			peers: Vec::new(),
		}
	}

	#[test]
	fn event_journal_replays_exact_gap_then_continues_live() {
		let mut journal = EventJournal::default();
		journal.record(event(10, TransactionPoolEventKind::Imported));
		journal.record(event(10, TransactionPoolEventKind::Future));

		let mut stream = journal.subscribe(Some(1)).expect("cursor is retained");
		journal.record(event(10, TransactionPoolEventKind::Evicted));

		let replayed = block_on(stream.next()).expect("replayed event");
		let live = block_on(stream.next()).expect("live event");
		assert_eq!((replayed.seq, replayed.kind), (2, TransactionPoolEventKind::Future));
		assert_eq!((live.seq, live.kind), (3, TransactionPoolEventKind::Evicted));
	}

	#[test]
	fn event_journal_rejects_cursor_older_than_retention() {
		let mut journal = EventJournal::default();
		for hash in 0..=TXPOOL_EVENT_HISTORY_LIMIT as u64 {
			journal.record(event(hash, TransactionPoolEventKind::Imported));
		}

		assert_eq!(
			journal.subscribe(Some(0)).unwrap_err(),
			TransactionPoolEventStreamError::CursorExpired {
				requested_seq: 0,
				oldest_seq: 2,
				latest_seq: TXPOOL_EVENT_HISTORY_LIMIT as u64 + 1,
			},
		);
	}
}
