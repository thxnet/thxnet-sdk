//! `health` subcommand — chain health monitoring.
//!
//! Queries a chain endpoint and produces a structured health report covering
//! block production, finalization lag, peer connectivity, and runtime version.

use crate::{
    rpc::{RpcApiClient, SharedRpcClient},
    types::HealthReport,
};
use anyhow::{Context, Result};
use sp_runtime::traits::Header as HeaderT;

/// Maximum acceptable gap between best and finalized block.
const MAX_FINALIZATION_GAP: u64 = 50;

/// Minimum peer count for a healthy node.
const MIN_PEERS: u64 = 1;

/// Collect health information from a single chain endpoint.
pub async fn check_health(client: &SharedRpcClient) -> Result<HealthReport> {
    let chain_name = client
        .system_chain()
        .await
        .context("failed to fetch chain name")?;

    let runtime_version = client
        .state_get_runtime_version(None)
        .await
        .context("failed to fetch runtime version")?;

    let health = client
        .health()
        .await
        .context("failed to fetch system health")?;

    let best_header = client
        .chain_get_header(None)
        .await
        .context("failed to fetch best header")?
        .context("best header was None")?;

    let finalized_hash = client
        .chain_get_finalized_head()
        .await
        .context("failed to fetch finalized head")?;

    let finalized_header = client
        .chain_get_header(Some(finalized_hash))
        .await
        .context("failed to fetch finalized header")?
        .context("finalized header was None")?;

    let best_block = (*best_header.number()) as u64;
    let finalized_block = (*finalized_header.number()) as u64;
    let finalization_gap = best_block.saturating_sub(finalized_block);

    let mut issues = Vec::new();

    if health.is_syncing {
        issues.push("node is still syncing".into());
    }
    if health.peers < MIN_PEERS && health.should_have_peers {
        issues.push(format!("peer count {} < minimum {MIN_PEERS}", health.peers));
    }
    if finalization_gap > MAX_FINALIZATION_GAP {
        issues.push(format!(
            "finalization gap {finalization_gap} > threshold {MAX_FINALIZATION_GAP}"
        ));
    }

    let healthy = issues.is_empty();

    Ok(HealthReport {
        url: client.uri().to_owned(),
        chain_name,
        spec_name: runtime_version.spec_name.to_string(),
        spec_version: runtime_version.spec_version,
        best_block,
        finalized_block,
        finalization_gap,
        peers: health.peers,
        is_syncing: health.is_syncing,
        healthy,
        issues,
    })
}

/// Run health checks against rootchain and (optionally) leafchain endpoints.
pub async fn run(
    rootchain_url: &str,
    leafchain_url: Option<&str>,
) -> Result<Vec<HealthReport>> {
    let mut reports = Vec::new();

    log::info!("connecting to rootchain at {rootchain_url}");
    let root_client = SharedRpcClient::new(rootchain_url)
        .await
        .context("failed to connect to rootchain")?;

    let root_report = check_health(&root_client).await?;
    reports.push(root_report);

    if let Some(leaf_url) = leafchain_url {
        log::info!("connecting to leafchain at {leaf_url}");
        let leaf_client = SharedRpcClient::new(leaf_url)
            .await
            .context("failed to connect to leafchain")?;

        let leaf_report = check_health(&leaf_client).await?;
        reports.push(leaf_report);
    }

    Ok(reports)
}
