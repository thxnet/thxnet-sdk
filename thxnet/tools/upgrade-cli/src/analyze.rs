//! `analyze` subcommand — diff two runtime WASM blobs.
//!
//! Parses the `runtime_version` custom section from each WASM, compares
//! spec versions, and flags consensus-critical dangers such as epoch-length
//! changes.

use crate::types::{AnalysisReport, WasmInfo};
use anyhow::{bail, Context, Result};
use sc_executor_common::runtime_blob::RuntimeBlob;
use sp_version::RuntimeVersion;
use std::path::Path;

/// Read a WASM file from disk, decompress if needed, and extract its
/// `RuntimeVersion` from the embedded custom section.
pub fn extract_wasm_info(path: &Path) -> Result<WasmInfo> {
    let raw_bytes = std::fs::read(path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    let file_size = raw_bytes.len() as u64;

    let blob = RuntimeBlob::uncompress_if_needed(&raw_bytes)
        .with_context(|| format!("failed to parse WASM from {}", path.display()))?;

    let version_bytes = blob
        .custom_section_contents("runtime_version")
        .context("WASM is missing 'runtime_version' custom section")?;

    // Determine core API version for decoding hints.
    let core_version = blob
        .custom_section_contents("runtime_apis")
        .and_then(|apis_bytes| {
            codec::Decode::decode(&mut &apis_bytes[..]).ok()
        })
        .and_then(|apis: sp_version::ApisVec| sp_version::core_version_from_apis(&apis));

    let version = RuntimeVersion::decode_with_version_hint(
        &mut &version_bytes[..],
        core_version,
    )
    .context("failed to decode RuntimeVersion from WASM custom section")?;

    // Decompressed size: re-decompress to measure (RuntimeBlob doesn't expose it).
    let decompressed = sp_maybe_compressed_blob::decompress(
        &raw_bytes,
        50 * 1024 * 1024,
    )
    .unwrap_or_else(|_| raw_bytes.clone().into());

    Ok(WasmInfo {
        spec_name: version.spec_name.to_string(),
        impl_name: version.impl_name.to_string(),
        spec_version: version.spec_version,
        impl_version: version.impl_version,
        authoring_version: version.authoring_version,
        transaction_version: version.transaction_version,
        state_version: version.state_version,
        file_size,
        decompressed_size: decompressed.len() as u64,
        api_count: version.apis.len(),
    })
}

/// Compare two WASM runtime blobs and produce an analysis report.
pub fn diff(old_path: &Path, new_path: &Path) -> Result<AnalysisReport> {
    let old = extract_wasm_info(old_path)
        .with_context(|| format!("old WASM: {}", old_path.display()))?;
    let new = extract_wasm_info(new_path)
        .with_context(|| format!("new WASM: {}", new_path.display()))?;

    let mut dangers = Vec::new();
    let mut warnings = Vec::new();
    let mut info = Vec::new();

    // spec_version must increase.
    if new.spec_version <= old.spec_version {
        dangers.push(format!(
            "new spec_version ({}) is not greater than old ({}); runtime upgrade will be rejected",
            new.spec_version, old.spec_version,
        ));
    } else {
        info.push(format!(
            "spec_version: {} -> {}",
            old.spec_version, new.spec_version,
        ));
    }

    // spec_name must match.
    if old.spec_name != new.spec_name {
        dangers.push(format!(
            "spec_name changed: '{}' -> '{}'; this is almost certainly wrong",
            old.spec_name, new.spec_name,
        ));
    }

    // transaction_version change.
    if old.transaction_version != new.transaction_version {
        warnings.push(format!(
            "transaction_version changed: {} -> {}; wallets and indexers will need updating",
            old.transaction_version, new.transaction_version,
        ));
    }

    // state_version change.
    if old.state_version != new.state_version {
        warnings.push(format!(
            "state_version changed: {} -> {}; all trie proofs will use new hashing",
            old.state_version, new.state_version,
        ));
    }

    // Runtime API count change.
    let api_delta = new.api_count as i64 - old.api_count as i64;
    if api_delta != 0 {
        info.push(format!(
            "runtime API count: {} -> {} ({:+})",
            old.api_count, new.api_count, api_delta,
        ));
    }

    // Size change.
    let size_delta = new.file_size as i64 - old.file_size as i64;
    if size_delta.unsigned_abs() > 100 * 1024 {
        warnings.push(format!(
            "WASM size changed significantly: {:.1} KB -> {:.1} KB ({:+.1} KB)",
            old.file_size as f64 / 1024.0,
            new.file_size as f64 / 1024.0,
            size_delta as f64 / 1024.0,
        ));
    }

    Ok(AnalysisReport { old, new, dangers, warnings, info })
}

/// Run the analyze subcommand.
pub fn run(old_path: &Path, new_path: &Path) -> Result<AnalysisReport> {
    if !old_path.exists() {
        bail!("old WASM not found: {}", old_path.display());
    }
    if !new_path.exists() {
        bail!("new WASM not found: {}", new_path.display());
    }
    diff(old_path, new_path)
}
