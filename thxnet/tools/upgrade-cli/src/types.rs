//! Shared types for the THXNet upgrade CLI.
//!
//! This module defines domain-specific types used across subcommands. Types are designed
//! to be serializable for `--json` output and have human-readable `Display` implementations.

use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::fmt;

// Re-export polkadot core primitives under shorter names.
// AccountId/Balance/Nonce kept for use in extrinsic construction.
#[allow(unused)]
pub type AccountId = core_primitives::AccountId;
#[allow(unused)]
pub type Balance = core_primitives::Balance;
#[allow(unused)]
pub type Nonce = core_primitives::Nonce;
pub type Hash = core_primitives::Hash;
pub type Header = core_primitives::Header;

// ---------------------------------------------------------------------------
// RPC response types
// ---------------------------------------------------------------------------

/// Response from `system_health` RPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemHealth {
    pub peers: u64,
    pub is_syncing: bool,
    pub should_have_peers: bool,
}

/// Response from `system_syncState` RPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncState {
    pub starting_block: u64,
    pub current_block: u64,
    pub highest_block: u64,
}

// ---------------------------------------------------------------------------
// Health report
// ---------------------------------------------------------------------------

/// Aggregated health assessment of a chain endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct HealthReport {
    pub url: String,
    pub chain_name: String,
    pub spec_name: String,
    pub spec_version: u32,
    pub best_block: u64,
    pub finalized_block: u64,
    pub finalization_gap: u64,
    pub peers: u64,
    pub is_syncing: bool,
    pub healthy: bool,
    pub issues: Vec<String>,
}

impl fmt::Display for HealthReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.healthy { "HEALTHY" } else { "UNHEALTHY" };
        write!(
            f,
            "Chain Health: {status}\n\
             \x20 Chain:             {chain}\n\
             \x20 Spec:              {spec} v{ver}\n\
             \x20 Best block:        #{best}\n\
             \x20 Finalized block:   #{fin}\n\
             \x20 Finalization gap:  {gap} blocks\n\
             \x20 Peers:             {peers}\n\
             \x20 Syncing:           {syncing}",
            chain = self.chain_name,
            spec = self.spec_name,
            ver = self.spec_version,
            best = self.best_block,
            fin = self.finalized_block,
            gap = self.finalization_gap,
            peers = self.peers,
            syncing = self.is_syncing,
        )?;
        for issue in &self.issues {
            write!(f, "\n  - {issue}")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// WASM introspection
// ---------------------------------------------------------------------------

/// Metadata extracted from a WASM runtime blob's custom sections.
#[derive(Debug, Clone, Serialize)]
pub struct WasmInfo {
    pub spec_name: String,
    pub impl_name: String,
    pub spec_version: u32,
    pub impl_version: u32,
    pub authoring_version: u32,
    pub transaction_version: u32,
    pub state_version: u8,
    pub file_size: u64,
    pub decompressed_size: u64,
    pub api_count: usize,
}

impl fmt::Display for WasmInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WASM Runtime Info:\n\
             \x20 spec_name:           {spec_name}\n\
             \x20 impl_name:           {impl_name}\n\
             \x20 spec_version:        {spec_ver}\n\
             \x20 impl_version:        {impl_ver}\n\
             \x20 authoring_version:   {auth_ver}\n\
             \x20 transaction_version: {tx_ver}\n\
             \x20 state_version:       {state_ver}\n\
             \x20 file_size:           {file_kb:.1} KB\n\
             \x20 decompressed_size:   {decomp_kb:.1} KB\n\
             \x20 runtime APIs:        {apis}",
            spec_name = self.spec_name,
            impl_name = self.impl_name,
            spec_ver = self.spec_version,
            impl_ver = self.impl_version,
            auth_ver = self.authoring_version,
            tx_ver = self.transaction_version,
            state_ver = self.state_version,
            file_kb = self.file_size as f64 / 1024.0,
            decomp_kb = self.decompressed_size as f64 / 1024.0,
            apis = self.api_count,
        )
    }
}

// ---------------------------------------------------------------------------
// Runtime analysis (diff of two WASMs)
// ---------------------------------------------------------------------------

/// Comparison report between an old and new runtime WASM.
#[derive(Debug, Clone, Serialize)]
pub struct AnalysisReport {
    pub old: WasmInfo,
    pub new: WasmInfo,
    pub dangers: Vec<String>,
    pub warnings: Vec<String>,
    pub info: Vec<String>,
}

impl fmt::Display for AnalysisReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Runtime Analysis:\n\
             \x20 spec_version:  {} -> {}\n\
             \x20 impl_version:  {} -> {}\n\
             \x20 spec_name:     {} -> {}\n\
             \x20 file_size:     {:.1} KB -> {:.1} KB",
            self.old.spec_version,
            self.new.spec_version,
            self.old.impl_version,
            self.new.impl_version,
            self.old.spec_name,
            self.new.spec_name,
            self.old.file_size as f64 / 1024.0,
            self.new.file_size as f64 / 1024.0,
        )?;
        format_section(f, "DANGERS", "[!!]", &self.dangers)?;
        format_section(f, "Warnings", "[!]", &self.warnings)?;
        format_section(f, "Info", "[i]", &self.info)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Preflight checks
// ---------------------------------------------------------------------------

/// Outcome of a single preflight validation.
#[derive(Debug, Clone, Serialize)]
pub struct PreflightCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

/// Aggregated preflight result.
#[derive(Debug, Clone, Serialize)]
pub struct PreflightResult {
    pub passed: bool,
    pub checks: Vec<PreflightCheck>,
}

impl PreflightResult {
    pub fn new(checks: Vec<PreflightCheck>) -> Self {
        let passed = checks.iter().all(|c| c.passed);
        Self { passed, checks }
    }
}

impl fmt::Display for PreflightResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.passed { "ALL CHECKS PASSED" } else { "CHECKS FAILED" };
        write!(f, "Preflight: {status}")?;
        for check in &self.checks {
            let icon = if check.passed { "OK" } else { "FAIL" };
            write!(f, "\n  [{icon}] {}: {}", check.name, check.detail)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Upgrade result
// ---------------------------------------------------------------------------

/// Outcome of a runtime upgrade submission.
#[derive(Debug, Clone, Serialize)]
pub struct UpgradeResult {
    pub success: bool,
    pub block_hash: Option<String>,
    pub new_spec_version: Option<u32>,
    pub extrinsic_hex: Option<String>,
    pub detail: String,
}

impl fmt::Display for UpgradeResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.success { "SUCCESS" } else { "FAILED" };
        write!(f, "Upgrade: {status}\n  {}", self.detail)?;
        if let Some(ref hash) = self.block_hash {
            write!(f, "\n  Block hash: {hash}")?;
        }
        if let Some(v) = self.new_spec_version {
            write!(f, "\n  New spec_version: {v}")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// BABE configuration (decoded from BabeApi_configuration runtime call)
// ---------------------------------------------------------------------------

/// On-chain BABE consensus parameters.
///
/// We only decode the fields we need for epoch-length safety checks; the
/// `authorities` and `randomness` fields are included for completeness
/// but could be skipped if decoding performance mattered.
#[derive(Debug, Clone, Decode, Encode)]
pub struct BabeConfiguration {
    pub slot_duration: u64,
    pub epoch_length: u64,
    pub c: (u64, u64),
    pub authorities: Vec<(BabeAuthorityId, u64)>,
    pub randomness: [u8; 32],
    pub allowed_slots: AllowedSlots,
}

/// Opaque BABE authority public key (sr25519, 32 bytes).
#[derive(Debug, Clone, Decode, Encode)]
pub struct BabeAuthorityId(pub [u8; 32]);

/// Allowed slot assignment strategy for BABE.
#[derive(Debug, Clone, Decode, Encode)]
pub enum AllowedSlots {
    PrimarySlots,
    PrimaryAndSecondaryPlainSlots,
    PrimaryAndSecondaryVRFSlots,
}

// ---------------------------------------------------------------------------
// Snapshot metadata
// ---------------------------------------------------------------------------

/// Persistent metadata written alongside each forked-testnet snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub name: String,
    pub created_at: String,
    pub namespace: String,
    pub rootchain_spec_version: Option<u32>,
    pub leafchain_spec_version: Option<u32>,
    pub rootchain_best_block: Option<u64>,
    pub leafchain_best_block: Option<u64>,
    pub components: Vec<String>,
    pub total_size_bytes: Option<u64>,
}

impl fmt::Display for SnapshotMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Snapshot: {name}\n\
             \x20 Created:   {ts}\n\
             \x20 Namespace: {ns}",
            name = self.name,
            ts = self.created_at,
            ns = self.namespace,
        )?;
        if let Some(v) = self.rootchain_spec_version {
            write!(f, "\n  Rootchain spec_version: {v}")?;
        }
        if let Some(b) = self.rootchain_best_block {
            write!(f, "\n  Rootchain best block:   #{b}")?;
        }
        if let Some(v) = self.leafchain_spec_version {
            write!(f, "\n  Leafchain spec_version: {v}")?;
        }
        if let Some(b) = self.leafchain_best_block {
            write!(f, "\n  Leafchain best block:   #{b}")?;
        }
        if let Some(size) = self.total_size_bytes {
            write!(f, "\n  Total size: {:.1} GB", size as f64 / (1024.0 * 1024.0 * 1024.0))?;
        }
        write!(f, "\n  Components: {}", self.components.join(", "))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn format_section(f: &mut fmt::Formatter<'_>, header: &str, prefix: &str, items: &[String]) -> fmt::Result {
    if !items.is_empty() {
        write!(f, "\n\n  {header}:")?;
        for item in items {
            write!(f, "\n    {prefix} {item}")?;
        }
    }
    Ok(())
}
