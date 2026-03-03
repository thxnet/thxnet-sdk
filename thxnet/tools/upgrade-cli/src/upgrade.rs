//! `upgrade` subcommand — submit a runtime WASM via `sudo.sudoUncheckedWeight(system.setCode(wasm))`.
//!
//! This is the most dangerous operation in the CLI. It runs all preflight
//! checks before submission, and supports `--dry-run` to inspect the
//! extrinsic without broadcasting.

use crate::{
    analyze,
    extrinsic::{self, CallIndices},
    preflight,
    rpc::{RpcApiClient, SharedRpcClient},
    types::UpgradeResult,
};
use anyhow::{bail, Context, Result};
use sp_core::{crypto::Pair as _, sr25519};
use std::path::Path;

/// Execute the upgrade subcommand.
///
/// When `dry_run` is true, the extrinsic is constructed and printed but not
/// submitted.
pub async fn run(
    url: &str,
    wasm_path: &Path,
    sudo_seed: &str,
    dry_run: bool,
) -> Result<UpgradeResult> {
    // 1. Run preflight.
    log::info!("running preflight checks...");
    let preflight = preflight::run(url, wasm_path).await?;
    if !preflight.passed {
        println!("{preflight}");
        bail!("preflight checks failed — aborting upgrade");
    }
    log::info!("preflight passed");

    // 2. Read WASM blob.
    let wasm_bytes = std::fs::read(wasm_path)
        .with_context(|| format!("failed to read WASM from {}", wasm_path.display()))?;

    let wasm_info = analyze::extract_wasm_info(wasm_path)?;
    log::info!(
        "WASM: {} v{} ({:.1} KB)",
        wasm_info.spec_name,
        wasm_info.spec_version,
        wasm_info.file_size as f64 / 1024.0,
    );

    // 3. Parse sudo keypair.
    let pair = sr25519::Pair::from_string(sudo_seed, None)
        .map_err(|e| anyhow::anyhow!("invalid sudo seed: {e:?}"))?;
    let account_id = pair.public();
    log::info!("sudo account: {}", sp_core::crypto::Ss58Codec::to_ss58check(&account_id));

    // 4. Connect and fetch chain state.
    let client = SharedRpcClient::new(url)
        .await
        .context("failed to connect")?;

    let runtime_version = client
        .state_get_runtime_version(None)
        .await
        .context("failed to fetch runtime version")?;

    let genesis_hash = client
        .chain_get_block_hash(Some(0))
        .await
        .context("failed to fetch genesis hash")?
        .context("genesis hash was None")?;

    let finalized_hash = client
        .chain_get_finalized_head()
        .await
        .context("failed to fetch finalized head")?;

    let account_ss58 = sp_core::crypto::Ss58Codec::to_ss58check(&account_id);
    let nonce: u32 = client
        .system_account_next_index(&account_ss58)
        .await
        .context("failed to fetch account nonce")?;

    log::info!(
        "chain: spec_version={}, tx_version={}, nonce={nonce}",
        runtime_version.spec_version,
        runtime_version.transaction_version,
    );

    // 5. Build call data: sudo.sudoUncheckedWeight(system.setCode(wasm)).
    let indices = CallIndices::default();
    let set_code_call = extrinsic::encode_set_code_call(&indices, &wasm_bytes);
    let sudo_call = extrinsic::encode_sudo_unchecked_weight(&indices, &set_code_call);

    log::info!(
        "call data: {} bytes (WASM {:.1} KB + overhead)",
        sudo_call.len(),
        wasm_bytes.len() as f64 / 1024.0,
    );

    // 6. Sign the extrinsic.
    let extrinsic = extrinsic::build_signed_extrinsic(
        &sudo_call,
        &pair,
        nonce,
        runtime_version.spec_version,
        runtime_version.transaction_version,
        genesis_hash,
        finalized_hash,
    );

    let extrinsic_hex = format!("0x{}", hex::encode(&extrinsic));

    if dry_run {
        return Ok(UpgradeResult {
            success: true,
            block_hash: None,
            new_spec_version: Some(wasm_info.spec_version),
            extrinsic_hex: Some(extrinsic_hex),
            detail: format!(
                "dry run — extrinsic constructed ({} bytes), not submitted",
                extrinsic.len(),
            ),
        });
    }

    // 7. Submit and watch.
    log::info!("submitting runtime upgrade extrinsic...");
    let block_hash = extrinsic::submit_and_watch(&client, &extrinsic).await?;

    // 8. Verify new spec_version.
    let new_version = client
        .state_get_runtime_version(Some(block_hash))
        .await
        .context("failed to fetch new runtime version")?;

    let success = new_version.spec_version == wasm_info.spec_version;

    Ok(UpgradeResult {
        success,
        block_hash: Some(format!("{block_hash:?}")),
        new_spec_version: Some(new_version.spec_version),
        extrinsic_hex: None,
        detail: if success {
            format!(
                "runtime upgraded: {} -> {}",
                runtime_version.spec_version, new_version.spec_version,
            )
        } else {
            format!(
                "spec_version mismatch: expected {}, got {}",
                wasm_info.spec_version, new_version.spec_version,
            )
        },
    })
}
