//! `snapshot` subcommand — backup and restore forked testnet data.
//!
//! Manages chain data snapshots for safe upgrade testing. Automates the
//! manual backup/restore process documented in CLAUDE.md:
//!
//! 1. Scale down all deployments
//! 2. Copy chain data directories
//! 3. Save metadata (versions, block heights)
//! 4. Scale back up
//!
//! Uses `kubectl` and filesystem operations on the Hetzner VM where `/data`
//! is mounted.

use crate::types::SnapshotMetadata;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use tokio::process::Command;

/// Base directory for snapshots.
const BACKUP_BASE: &str = "/data/backups";

/// Deployments to scale down/up during snapshot operations.
const DEPLOYMENTS: &[&str] = &[
    "archive-node",
    "validator-atabek",
    "collator-albania",
    "collator-bahamas",
];

/// Chain data directories to snapshot.
const DATA_DIRS: &[(&str, &str)] = &[
    ("rootchain-archive", "/data/rootchain-archive"),
    ("validators-atabek", "/data/validators/atabek"),
    ("collators-albania", "/data/collators/albania"),
    ("collators-bahamas", "/data/collators/bahamas"),
];

/// Run a kubectl command and return its stdout.
async fn kubectl(args: &[&str], namespace: &str) -> Result<String> {
    let output = Command::new("kubectl")
        .args(args)
        .args(["-n", namespace])
        .output()
        .await
        .context("failed to execute kubectl")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("kubectl {:?} failed: {stderr}", args);
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Scale a deployment to the given replica count.
async fn scale_deployment(name: &str, replicas: u32, namespace: &str) -> Result<()> {
    log::info!("scaling {name} to {replicas}");
    kubectl(
        &["scale", "deployment", name, &format!("--replicas={replicas}")],
        namespace,
    )
    .await?;
    Ok(())
}

/// Wait for all pods in a deployment to terminate (replicas = 0).
async fn wait_for_scale_down(name: &str, namespace: &str) -> Result<()> {
    log::info!("waiting for {name} pods to terminate...");
    kubectl(
        &[
            "wait",
            "--for=delete",
            &format!("pod/-l=app={name}"),
            "--timeout=120s",
        ],
        namespace,
    )
    .await
    .ok(); // Ignore errors — pods may already be gone.
    Ok(())
}

/// Get the size of a directory in bytes using `du -sb`.
async fn dir_size(path: &Path) -> Result<u64> {
    let output = Command::new("du")
        .args(["-sb", &path.to_string_lossy()])
        .output()
        .await?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout
            .split_whitespace()
            .next()
            .and_then(|s| s.parse().ok())
            .context("failed to parse du output")
    } else {
        Ok(0)
    }
}

/// Create a new snapshot.
pub async fn create(name: &str, namespace: &str) -> Result<SnapshotMetadata> {
    let backup_dir = PathBuf::from(BACKUP_BASE).join(name);
    if backup_dir.exists() {
        bail!("snapshot '{}' already exists at {}", name, backup_dir.display());
    }

    // 1. Scale down all deployments.
    for dep in DEPLOYMENTS {
        scale_deployment(dep, 0, namespace).await?;
    }

    // Wait for pods to terminate.
    for dep in DEPLOYMENTS {
        wait_for_scale_down(dep, namespace).await?;
    }

    // Brief pause for filesystem sync.
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // 2. Create backup directory.
    std::fs::create_dir_all(&backup_dir)
        .with_context(|| format!("failed to create {}", backup_dir.display()))?;

    // 3. Copy chain data directories.
    let mut components = Vec::new();
    let mut total_size = 0u64;

    for (label, src_path) in DATA_DIRS {
        let src = Path::new(src_path);
        if !src.exists() {
            log::warn!("skipping {label}: {src_path} does not exist");
            continue;
        }

        let dest = backup_dir.join(label);
        log::info!("copying {src_path} -> {} ...", dest.display());

        let status = Command::new("cp")
            .args(["-a", src_path, &dest.to_string_lossy()])
            .status()
            .await
            .with_context(|| format!("failed to copy {src_path}"))?;

        if !status.success() {
            bail!("cp failed for {src_path}");
        }

        let size = dir_size(&dest).await.unwrap_or(0);
        total_size += size;
        components.push(label.to_string());
        log::info!("{label}: {:.1} GB", size as f64 / (1024.0 * 1024.0 * 1024.0));
    }

    // 4. Write metadata.
    let metadata = SnapshotMetadata {
        name: name.to_owned(),
        created_at: chrono::Utc::now().to_rfc3339(),
        namespace: namespace.to_owned(),
        rootchain_spec_version: None, // Populated by health check if desired.
        leafchain_spec_version: None,
        rootchain_best_block: None,
        leafchain_best_block: None,
        components,
        total_size_bytes: Some(total_size),
    };

    let metadata_path = backup_dir.join("metadata.json");
    let json = serde_json::to_string_pretty(&metadata)?;
    std::fs::write(&metadata_path, &json)
        .with_context(|| format!("failed to write {}", metadata_path.display()))?;

    // 5. Scale deployments back up.
    for dep in DEPLOYMENTS {
        scale_deployment(dep, 1, namespace).await?;
    }

    log::info!("snapshot '{}' created ({:.1} GB)", name, total_size as f64 / (1024.0 * 1024.0 * 1024.0));
    Ok(metadata)
}

/// List all available snapshots.
pub async fn list() -> Result<Vec<SnapshotMetadata>> {
    let base = Path::new(BACKUP_BASE);
    if !base.exists() {
        return Ok(Vec::new());
    }

    let mut snapshots = Vec::new();

    let mut entries: Vec<_> = std::fs::read_dir(base)
        .context("failed to read backup directory")?
        .filter_map(|e| e.ok())
        .collect();

    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let metadata_path = entry.path().join("metadata.json");
        if metadata_path.exists() {
            let content = std::fs::read_to_string(&metadata_path)?;
            if let Ok(meta) = serde_json::from_str::<SnapshotMetadata>(&content) {
                snapshots.push(meta);
            }
        }
    }

    Ok(snapshots)
}

/// Restore a snapshot by name.
pub async fn restore(name: &str, namespace: &str) -> Result<()> {
    let backup_dir = PathBuf::from(BACKUP_BASE).join(name);
    if !backup_dir.exists() {
        bail!("snapshot '{}' not found at {}", name, backup_dir.display());
    }

    let metadata_path = backup_dir.join("metadata.json");
    let content = std::fs::read_to_string(&metadata_path)
        .with_context(|| format!("failed to read {}", metadata_path.display()))?;
    let metadata: SnapshotMetadata = serde_json::from_str(&content)?;

    log::info!("restoring snapshot '{}' (created {})", name, metadata.created_at);

    // 1. Scale down all deployments.
    for dep in DEPLOYMENTS {
        scale_deployment(dep, 0, namespace).await?;
    }

    for dep in DEPLOYMENTS {
        wait_for_scale_down(dep, namespace).await?;
    }

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // 2. Restore each component.
    for (label, dest_path) in DATA_DIRS {
        let src = backup_dir.join(label);
        if !src.exists() {
            log::warn!("skipping {label}: not in snapshot");
            continue;
        }

        let dest = Path::new(dest_path);

        // Remove current data.
        if dest.exists() {
            log::info!("removing {dest_path}...");
            let status = Command::new("rm")
                .args(["-rf", dest_path])
                .status()
                .await?;
            if !status.success() {
                bail!("failed to remove {dest_path}");
            }
        }

        // Copy snapshot data.
        log::info!("restoring {} -> {dest_path}", src.display());
        let status = Command::new("cp")
            .args(["-a", &src.to_string_lossy(), dest_path])
            .status()
            .await?;
        if !status.success() {
            bail!("failed to restore {dest_path}");
        }
    }

    // 3. Scale back up.
    for dep in DEPLOYMENTS {
        scale_deployment(dep, 1, namespace).await?;
    }

    log::info!("snapshot '{}' restored", name);
    Ok(())
}
