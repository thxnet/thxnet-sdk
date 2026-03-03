//! Extrinsic construction, signing, and submission.
//!
//! Builds raw SCALE-encoded extrinsics for `sudo.sudoUncheckedWeight(call, weight)`
//! without importing any concrete runtime crate. Instead, call indices are
//! discovered dynamically via `state_getMetadata`, and the signed payload is
//! assembled manually following the generic extrinsic format.
//!
//! This approach avoids the `any_runtime!` macro pattern used by `staking-miner`
//! (which imports each concrete runtime). We only target THXNet, so we hard-code
//! the signed extension order (identical to Polkadot-family chains).

use crate::{
    rpc::{RpcApiClient, SharedRpcClient},
    types::Hash,
};
use anyhow::{bail, Context, Result};
use codec::Encode;
use sc_transaction_pool_api::TransactionStatus;
use sp_core::{crypto::Pair as _, sr25519, Bytes, H256};
use sp_runtime::{
    generic::Era,
    traits::{BlakeTwo256, Hash as HashT},
    MultiSignature,
};
use std::time::Duration;

/// Pallet + call indices discovered from metadata or hard-coded for THXNet.
///
/// THXNet uses the same pallet ordering as Polkadot-family runtimes.
/// These indices are validated against on-chain metadata at runtime.
#[derive(Debug, Clone)]
pub struct CallIndices {
    pub sudo_pallet: u8,
    pub sudo_unchecked_weight_call: u8,
    pub system_pallet: u8,
    pub system_set_code_call: u8,
    pub configuration_pallet: u8,
}

impl Default for CallIndices {
    fn default() -> Self {
        Self {
            // THXNet pallet indices (from construct_runtime! in lib.rs)
            sudo_pallet: 15,
            sudo_unchecked_weight_call: 1, // sudo.sudoUncheckedWeight
            system_pallet: 0,
            system_set_code_call: 2, // system.setCode
            configuration_pallet: 51, // configuration pallet index
        }
    }
}

/// Build the SCALE-encoded call data for `system.setCode(code)`.
pub fn encode_set_code_call(indices: &CallIndices, wasm_code: &[u8]) -> Vec<u8> {
    let mut call = Vec::new();
    call.push(indices.system_pallet);
    call.push(indices.system_set_code_call);
    wasm_code.encode_to(&mut call);
    call
}

/// Wrap a call in `sudo.sudoUncheckedWeight(call, weight)`.
///
/// Weight is set to `(0, DispatchClass::Operational)` — the sudo pallet ignores
/// it, but the encoding must be present.
pub fn encode_sudo_unchecked_weight(indices: &CallIndices, inner_call: &[u8]) -> Vec<u8> {
    let mut call = Vec::new();
    call.push(indices.sudo_pallet);
    call.push(indices.sudo_unchecked_weight_call);
    // The inner call is already encoded with pallet+call indices; we wrap it as
    // an opaque `Box<RuntimeCall>` by length-prefixing (Compact<u32> + bytes).
    inner_call.encode_to(&mut call);
    // Weight { ref_time: u64, proof_size: u64 } — both zero.
    0u64.encode_to(&mut call); // ref_time
    0u64.encode_to(&mut call); // proof_size
    call
}

/// Encode call for `sudo.sudo(inner_call)`.
pub fn encode_sudo(indices: &CallIndices, inner_call: &[u8]) -> Vec<u8> {
    let mut call = Vec::new();
    call.push(indices.sudo_pallet);
    call.push(0u8); // sudo.sudo has call_index 0
    inner_call.encode_to(&mut call);
    call
}

/// Build the signed extensions payload for THXNet (same as Polkadot sans PrevalidateAttests).
///
/// Signed extension tuple order:
/// 1. CheckNonZeroSender — no data
/// 2. CheckSpecVersion — spec_version: u32
/// 3. CheckTxVersion — tx_version: u32
/// 4. CheckGenesis — genesis_hash: H256
/// 5. CheckMortality — Era + era_block_hash: H256
/// 6. CheckNonce — nonce: Compact<u32>
/// 7. CheckWeight — no data
/// 8. ChargeTransactionPayment — tip: Compact<u128>
fn build_signed_extra(
    spec_version: u32,
    tx_version: u32,
    genesis_hash: H256,
    era: Era,
    era_block_hash: H256,
    nonce: u32,
    tip: u128,
) -> (Vec<u8>, Vec<u8>) {
    // "Extra" goes into the extrinsic body (signed extensions data).
    let mut extra = Vec::new();
    // CheckNonZeroSender — empty data in extrinsic.
    // CheckSpecVersion — empty data in extrinsic.
    // CheckTxVersion — empty data in extrinsic.
    // CheckGenesis — empty data in extrinsic.
    // CheckMortality — era.
    era.encode_to(&mut extra);
    // CheckNonce — compact nonce.
    codec::Compact(nonce).encode_to(&mut extra);
    // CheckWeight — empty data.
    // ChargeTransactionPayment — compact tip.
    codec::Compact(tip).encode_to(&mut extra);

    // "Additional signed" goes into the signing payload but NOT the extrinsic.
    let mut additional = Vec::new();
    // CheckNonZeroSender — nothing additional.
    // CheckSpecVersion — spec_version.
    spec_version.encode_to(&mut additional);
    // CheckTxVersion — tx_version.
    tx_version.encode_to(&mut additional);
    // CheckGenesis — genesis_hash.
    genesis_hash.encode_to(&mut additional);
    // CheckMortality — block hash for mortality.
    era_block_hash.encode_to(&mut additional);
    // CheckNonce — nothing additional.
    // CheckWeight — nothing additional.
    // ChargeTransactionPayment — nothing additional.

    (extra, additional)
}

/// Construct a signed extrinsic for THXNet.
///
/// Returns the SCALE-encoded extrinsic bytes ready for submission.
pub fn build_signed_extrinsic(
    call_data: &[u8],
    signer: &sr25519::Pair,
    nonce: u32,
    spec_version: u32,
    tx_version: u32,
    genesis_hash: H256,
    _best_hash: H256,
) -> Vec<u8> {
    // Use mortal era (64 blocks).
    let era = Era::mortal(64, 0);
    let era_block_hash = genesis_hash; // For immortal-like mortal, use genesis.

    let (extra, additional) = build_signed_extra(
        spec_version,
        tx_version,
        genesis_hash,
        era,
        era_block_hash,
        nonce,
        0, // tip
    );

    // Signing payload = call_data ++ extra ++ additional.
    // If payload > 256 bytes, hash it first (substrate convention).
    let mut payload = Vec::new();
    payload.extend_from_slice(call_data);
    payload.extend_from_slice(&extra);
    payload.extend_from_slice(&additional);

    let signature = if payload.len() > 256 {
        let hash = BlakeTwo256::hash(&payload);
        signer.sign(hash.as_ref())
    } else {
        signer.sign(&payload)
    };

    // Encode the final extrinsic.
    // Format: Compact(length) ++ version_byte ++ address ++ signature ++ extra ++ call
    let mut extrinsic = Vec::new();

    // Extrinsic version: 0x84 = signed (bit 7) + version 4 (bits 0-6).
    let version_byte: u8 = 0b1000_0100;

    // Build the inner (before length prefix).
    let mut inner = Vec::new();
    inner.push(version_byte);

    // Address: MultiAddress::Id(AccountId32)
    inner.push(0x00); // MultiAddress::Id variant
    inner.extend_from_slice(signer.public().as_ref());

    // Signature: MultiSignature::Sr25519
    let multi_sig = MultiSignature::Sr25519(signature);
    multi_sig.encode_to(&mut inner);

    // Signed extensions data.
    inner.extend_from_slice(&extra);

    // Call data.
    inner.extend_from_slice(call_data);

    // Length prefix the whole thing.
    codec::Compact(inner.len() as u32).encode_to(&mut extrinsic);
    extrinsic.extend_from_slice(&inner);

    extrinsic
}

/// Submit a signed extrinsic and wait for finalization.
///
/// Returns the block hash where the extrinsic was finalized.
pub async fn submit_and_watch(
    client: &SharedRpcClient,
    encoded_extrinsic: &[u8],
) -> Result<Hash> {
    let bytes = Bytes(encoded_extrinsic.to_vec());

    let mut subscription = client
        .author_submit_and_watch_extrinsic(&bytes)
        .await
        .context("failed to submit extrinsic")?;

    let timeout = Duration::from_secs(300);
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        let status = tokio::select! {
            status = subscription.next() => {
                match status {
                    Some(Ok(s)) => s,
                    Some(Err(e)) => bail!("subscription error: {e}"),
                    None => bail!("subscription closed unexpectedly"),
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                bail!("timed out waiting for extrinsic finalization ({timeout:?})");
            }
        };

        match status {
            TransactionStatus::Ready => {
                log::info!("extrinsic in ready queue");
            }
            TransactionStatus::Broadcast(peers) => {
                log::info!("extrinsic broadcast to {} peers", peers.len());
            }
            TransactionStatus::InBlock((hash, idx)) => {
                log::info!("extrinsic included in block {hash:?} at index {idx}");
            }
            TransactionStatus::Finalized((hash, idx)) => {
                log::info!("extrinsic finalized in block {hash:?} at index {idx}");
                return Ok(hash);
            }
            TransactionStatus::FinalityTimeout(hash) => {
                bail!("finality timeout for block {hash:?}");
            }
            TransactionStatus::Retracted(hash) => {
                log::warn!("block {hash:?} retracted, continuing to watch...");
            }
            TransactionStatus::Usurped(hash) => {
                bail!("extrinsic usurped by {hash:?}");
            }
            TransactionStatus::Dropped => {
                bail!("extrinsic dropped from pool");
            }
            TransactionStatus::Invalid => {
                bail!("extrinsic marked invalid by the pool");
            }
            TransactionStatus::Future => {
                log::info!("extrinsic in future queue (nonce too high?)");
            }
        }
    }
}
