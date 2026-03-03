//! THXNet Upgrade CLI — automate chain upgrade operations.
//!
//! ```text
//! thxnet-upgrade health     — check chain health
//! thxnet-upgrade preflight  — validate pre-upgrade conditions
//! thxnet-upgrade upgrade    — submit runtime WASM via sudo
//! thxnet-upgrade config     — read/set HostConfiguration values
//! thxnet-upgrade analyze    — diff two WASM runtimes
//! thxnet-upgrade snapshot   — backup/restore forked testnet data
//! ```

mod analyze;
mod config;
mod extrinsic;
mod health;
mod preflight;
mod rpc;
mod snapshot;
mod types;
mod upgrade;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "thxnet-upgrade", version, about = "THXNet chain upgrade automation")]
struct Cli {
    /// Output results as JSON instead of human-readable text.
    #[arg(long, global = true)]
    json: bool,

    /// Log level (error, warn, info, debug, trace).
    #[arg(long, global = true, default_value = "info", env = "RUST_LOG")]
    log_level: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check chain health (block production, finalization, peers).
    Health {
        /// Rootchain WebSocket RPC endpoint.
        #[arg(long, default_value = "ws://localhost:9944")]
        rootchain_url: String,

        /// Leafchain WebSocket RPC endpoint (optional).
        #[arg(long)]
        leafchain_url: Option<String>,
    },

    /// Validate pre-upgrade conditions against a live chain.
    Preflight {
        /// Chain WebSocket RPC endpoint.
        #[arg(long, default_value = "ws://localhost:9944")]
        url: String,

        /// Path to the new runtime WASM file.
        #[arg(long)]
        wasm: PathBuf,
    },

    /// Submit a runtime upgrade via `sudo.sudoUncheckedWeight(system.setCode(wasm))`.
    Upgrade {
        /// Chain WebSocket RPC endpoint.
        #[arg(long, default_value = "ws://localhost:9944")]
        url: String,

        /// Path to the new runtime WASM file.
        #[arg(long)]
        wasm: PathBuf,

        /// Sudo account seed (hex or SURI like `//Alice`).
        #[arg(long, env = "SUDO_SEED")]
        sudo_seed: String,

        /// Print the extrinsic without submitting it.
        #[arg(long)]
        dry_run: bool,
    },

    /// Read or set HostConfiguration values.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Diff two WASM runtimes to surface upgrade-relevant changes.
    Analyze {
        /// Path to the old (current) runtime WASM.
        #[arg(long)]
        old: PathBuf,

        /// Path to the new runtime WASM.
        #[arg(long)]
        new: PathBuf,
    },

    /// Manage forked testnet data snapshots.
    Snapshot {
        #[command(subcommand)]
        action: SnapshotAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Display current HostConfiguration values.
    Get {
        /// Chain WebSocket RPC endpoint.
        #[arg(long, default_value = "ws://localhost:9944")]
        url: String,
    },

    /// Set a configuration value via sudo.
    Set {
        /// Chain WebSocket RPC endpoint.
        #[arg(long, default_value = "ws://localhost:9944")]
        url: String,

        /// Sudo account seed.
        #[arg(long, env = "SUDO_SEED")]
        sudo_seed: String,

        /// Configuration field name (e.g. `scheduling-lookahead`).
        field: String,

        /// New value (u32).
        value: u32,

        /// Print the extrinsic without submitting it.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum SnapshotAction {
    /// Create a snapshot of the current forked testnet state.
    Create {
        /// Snapshot name.
        #[arg(long)]
        name: String,

        /// Kubernetes namespace.
        #[arg(long, default_value = "forked-testnet")]
        namespace: String,
    },

    /// List available snapshots.
    List,

    /// Restore a snapshot.
    Restore {
        /// Snapshot name to restore.
        #[arg(long)]
        name: String,

        /// Kubernetes namespace.
        #[arg(long, default_value = "forked-testnet")]
        namespace: String,
    },
}

/// Print output as JSON or human-readable depending on `--json` flag.
fn output<T: std::fmt::Display + serde::Serialize>(value: &T, json: bool) {
    if json {
        println!("{}", serde_json::to_string_pretty(value).expect("JSON serialization"));
    } else {
        println!("{value}");
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    env_logger::Builder::new()
        .filter_level(cli.log_level.parse().unwrap_or(log::LevelFilter::Info))
        .format_timestamp_millis()
        .init();

    match cli.command {
        Command::Health { rootchain_url, leafchain_url } => {
            let reports = health::run(
                &rootchain_url,
                leafchain_url.as_deref(),
            )
            .await?;

            for report in &reports {
                output(report, cli.json);
                if !cli.json {
                    println!();
                }
            }

            // Exit with non-zero if any endpoint is unhealthy.
            if reports.iter().any(|r| !r.healthy) {
                std::process::exit(1);
            }
        }

        Command::Preflight { url, wasm } => {
            let result = preflight::run(&url, &wasm).await?;
            output(&result, cli.json);
            if !result.passed {
                std::process::exit(1);
            }
        }

        Command::Upgrade { url, wasm, sudo_seed, dry_run } => {
            let result = upgrade::run(&url, &wasm, &sudo_seed, dry_run).await?;
            output(&result, cli.json);
            if !result.success {
                std::process::exit(1);
            }
        }

        Command::Config { action } => match action {
            ConfigAction::Get { url } => {
                let client = rpc::SharedRpcClient::new(&url).await?;
                let fields = config::get(&client).await?;

                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&fields)?);
                } else {
                    println!("HostConfiguration (ActiveConfig):");
                    for (key, value) in &fields {
                        if !key.starts_with('_') {
                            println!("  {key}: {value}");
                        }
                    }
                    if let Some(remaining) = fields.get("_remaining_bytes") {
                        println!("\n  Note: {remaining} (fields after executor_params not decoded)");
                    }
                }
            }
            ConfigAction::Set { url, sudo_seed, field, value, dry_run } => {
                let config_field = config::ConfigField::from_str(&field)
                    .ok_or_else(|| anyhow::anyhow!(
                        "unknown config field '{}'; try: scheduling-lookahead, \
                         max-validators, minimum-backing-votes, ...",
                        field,
                    ))?;

                let result = config::set(&url, config_field, value, &sudo_seed, dry_run).await?;
                println!("{result}");
            }
        },

        Command::Analyze { old, new } => {
            let report = analyze::run(&old, &new)?;
            output(&report, cli.json);
            if !report.dangers.is_empty() {
                std::process::exit(1);
            }
        }

        Command::Snapshot { action } => match action {
            SnapshotAction::Create { name, namespace } => {
                let metadata = snapshot::create(&name, &namespace).await?;
                output(&metadata, cli.json);
            }
            SnapshotAction::List => {
                let snapshots = snapshot::list().await?;
                if snapshots.is_empty() {
                    println!("No snapshots found.");
                } else {
                    for snap in &snapshots {
                        output(snap, cli.json);
                        if !cli.json {
                            println!();
                        }
                    }
                }
            }
            SnapshotAction::Restore { name, namespace } => {
                snapshot::restore(&name, &namespace).await?;
                println!("Snapshot '{name}' restored successfully.");
            }
        },
    }

    Ok(())
}
