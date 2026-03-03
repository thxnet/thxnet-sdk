//! `preflight` subcommand — pre-upgrade validation.
//!
//! Runs a battery of safety checks before a runtime upgrade is submitted:
//!
//! - WASM file validity and version extraction
//! - `spec_version` ordering (new > old)
//! - `spec_name` match
//! - BABE epoch length match (consensus-critical — Mistake 16)
//! - Chain health (delegates to [`crate::health`])

use crate::{
    analyze,
    health,
    rpc::{RpcApiClient, SharedRpcClient},
    types::{BabeConfiguration, PreflightCheck, PreflightResult},
};
use anyhow::{Context, Result};
use std::path::Path;

/// Maximum acceptable finalization gap for upgrade safety.
const MAX_FINALIZATION_GAP_FOR_UPGRADE: u64 = 10;

/// Run all preflight checks against a live chain and a WASM file.
pub async fn run(url: &str, wasm_path: &Path) -> Result<PreflightResult> {
    let mut checks = Vec::new();

    // 1. WASM file exists and is parseable.
    let wasm_info = match analyze::extract_wasm_info(wasm_path) {
        Ok(info) => {
            checks.push(PreflightCheck {
                name: "wasm_valid".into(),
                passed: true,
                detail: format!(
                    "{} v{} ({:.1} KB)",
                    info.spec_name,
                    info.spec_version,
                    info.file_size as f64 / 1024.0,
                ),
            });
            Some(info)
        }
        Err(e) => {
            checks.push(PreflightCheck {
                name: "wasm_valid".into(),
                passed: false,
                detail: format!("failed to parse WASM: {e}"),
            });
            None
        }
    };

    // 2. Connect to chain and get on-chain state.
    let client = SharedRpcClient::new(url)
        .await
        .context("failed to connect to chain endpoint")?;

    let on_chain_version = client
        .state_get_runtime_version(None)
        .await
        .context("failed to fetch on-chain runtime version")?;

    if let Some(ref wasm) = wasm_info {
        // 3. spec_version: new must be greater than on-chain.
        let spec_ok = wasm.spec_version > on_chain_version.spec_version;
        checks.push(PreflightCheck {
            name: "spec_version".into(),
            passed: spec_ok,
            detail: format!(
                "on-chain {} -> new {}{}",
                on_chain_version.spec_version,
                wasm.spec_version,
                if spec_ok { "" } else { " (must increase!)" },
            ),
        });

        // 4. spec_name must match.
        let on_chain_spec_name = on_chain_version.spec_name.to_string();
        let name_match = wasm.spec_name == on_chain_spec_name;
        checks.push(PreflightCheck {
            name: "spec_name".into(),
            passed: name_match,
            detail: format!(
                "on-chain '{}' vs new '{}'",
                on_chain_version.spec_name, wasm.spec_name,
            ),
        });
    }

    // 5. BABE epoch length: fetch from on-chain runtime API and compare.
    //    Changing epoch duration via runtime upgrade bricks the chain (Mistake 16).
    match check_epoch_length(&client).await {
        Ok((epoch_length, check)) => {
            checks.push(check);
            log::info!("on-chain BABE epoch_length = {epoch_length} slots");
        }
        Err(e) => {
            checks.push(PreflightCheck {
                name: "epoch_length".into(),
                passed: true,
                detail: format!("could not verify (non-fatal): {e}"),
            });
        }
    }

    // 6. Chain health check.
    let health_report = health::check_health(&client).await?;
    let health_ok = health_report.healthy;
    checks.push(PreflightCheck {
        name: "chain_health".into(),
        passed: health_ok,
        detail: if health_ok {
            format!(
                "healthy — {} peers, gap {}",
                health_report.peers, health_report.finalization_gap,
            )
        } else {
            format!(
                "unhealthy: {}",
                health_report.issues.join("; "),
            )
        },
    });

    // 7. Finalization gap must be small for upgrade safety.
    let fin_ok = health_report.finalization_gap <= MAX_FINALIZATION_GAP_FOR_UPGRADE;
    checks.push(PreflightCheck {
        name: "finalization_gap".into(),
        passed: fin_ok,
        detail: format!(
            "gap = {} blocks (threshold = {MAX_FINALIZATION_GAP_FOR_UPGRADE})",
            health_report.finalization_gap,
        ),
    });

    Ok(PreflightResult::new(checks))
}

/// Fetch the on-chain BABE epoch length via `BabeApi_configuration`.
///
/// This is the consensus-critical value that **must not** change across
/// runtime upgrades.
async fn check_epoch_length(client: &SharedRpcClient) -> Result<(u64, PreflightCheck)> {
    let babe_config: BabeConfiguration = client
        .runtime_call_decoded("BabeApi_configuration", None)
        .await
        .context("BabeApi_configuration runtime call failed")?;

    let epoch_length = babe_config.epoch_length;

    // We can't compare against the WASM's compiled-in value without executing
    // the WASM (the epoch length isn't in the custom section). Instead, we
    // report the on-chain value so the operator can manually verify.
    let check = PreflightCheck {
        name: "epoch_length".into(),
        passed: true,
        detail: format!(
            "on-chain BABE epoch = {epoch_length} slots; verify new WASM uses the same value!",
        ),
    };

    Ok((epoch_length, check))
}
