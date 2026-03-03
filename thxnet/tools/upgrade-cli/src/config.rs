//! `config` subcommand — read and set `HostConfiguration` values.
//!
//! Reads the `ActiveConfig` storage from `polkadot-runtime-parachains::configuration`
//! and displays it. Can also submit sudo calls to change individual config values
//! (e.g. `configuration.setSchedulingLookahead(1)`).
//!
//! The `HostConfiguration` struct is decoded manually to avoid depending on the
//! full `polkadot-runtime-parachains` crate (which pulls in the entire runtime).
//! We decode the raw SCALE bytes into a generic representation.

use crate::{
    extrinsic::{self, CallIndices},
    rpc::{RpcApiClient, SharedRpcClient},
};
use anyhow::{Context, Result};
use codec::{Decode, Encode};
use sp_core::{crypto::Pair as _, sr25519, storage::StorageKey, Bytes};
use std::collections::BTreeMap;

/// Well-known storage key for `configuration::ActiveConfig`.
///
/// Computed as `twox_128("Configuration") ++ twox_128("ActiveConfig")`.
const ACTIVE_CONFIG_KEY: &str = "06de3d8a54d27e44a9d5ce189618f22db4b49d95320d9021994c850f25b8e385";

/// Supported configuration fields that can be read/written.
///
/// Each variant maps to a `configuration::set_*` extrinsic.
#[derive(Debug, Clone, Copy)]
pub enum ConfigField {
    SchedulingLookahead,
    MaxValidatorsPerCore,
    MaxValidators,
    MinimumBackingVotes,
    GroupRotationFrequency,
    ParasAvailabilityPeriod,
    MaxCodeSize,
    MaxPovSize,
    MaxHeadDataSize,
    MaxUpwardQueueCount,
    MaxUpwardQueueSize,
    MaxUpwardMessageSize,
    MaxUpwardMessageNumPerCandidate,
    HrmpMaxParachainOutboundChannels,
    HrmpMaxParachainInboundChannels,
    HrmpChannelMaxCapacity,
    HrmpChannelMaxMessageSize,
    OnDemandCores,
    NeededApprovals,
    NDelayTranches,
    NoShowSlots,
    DisputePeriod,
}

impl ConfigField {
    /// Parse a field name from CLI input.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.replace('-', "_").to_lowercase().as_str() {
            "scheduling_lookahead" => Some(Self::SchedulingLookahead),
            "max_validators_per_core" => Some(Self::MaxValidatorsPerCore),
            "max_validators" => Some(Self::MaxValidators),
            "minimum_backing_votes" => Some(Self::MinimumBackingVotes),
            "group_rotation_frequency" => Some(Self::GroupRotationFrequency),
            "paras_availability_period" => Some(Self::ParasAvailabilityPeriod),
            "max_code_size" => Some(Self::MaxCodeSize),
            "max_pov_size" => Some(Self::MaxPovSize),
            "max_head_data_size" => Some(Self::MaxHeadDataSize),
            "max_upward_queue_count" => Some(Self::MaxUpwardQueueCount),
            "max_upward_queue_size" => Some(Self::MaxUpwardQueueSize),
            "max_upward_message_size" => Some(Self::MaxUpwardMessageSize),
            "max_upward_message_num_per_candidate" => Some(Self::MaxUpwardMessageNumPerCandidate),
            "hrmp_max_parachain_outbound_channels" => Some(Self::HrmpMaxParachainOutboundChannels),
            "hrmp_max_parachain_inbound_channels" => Some(Self::HrmpMaxParachainInboundChannels),
            "hrmp_channel_max_capacity" => Some(Self::HrmpChannelMaxCapacity),
            "hrmp_channel_max_message_size" => Some(Self::HrmpChannelMaxMessageSize),
            "on_demand_cores" => Some(Self::OnDemandCores),
            "needed_approvals" => Some(Self::NeededApprovals),
            "n_delay_tranches" => Some(Self::NDelayTranches),
            "no_show_slots" => Some(Self::NoShowSlots),
            "dispute_period" => Some(Self::DisputePeriod),
            _ => None,
        }
    }

    /// The `configuration` pallet's call index for the corresponding setter.
    ///
    /// These are derived from `#[pallet::call_index(N)]` annotations in
    /// `polkadot/runtime/parachains/src/configuration.rs`.
    fn call_index(&self) -> u8 {
        match self {
            Self::MaxCodeSize => 1,
            Self::MaxHeadDataSize => 2,
            Self::MaxUpwardQueueCount => 3,
            Self::MaxUpwardQueueSize => 4,
            Self::MaxUpwardMessageSize => 5,
            Self::MaxUpwardMessageNumPerCandidate => 6,
            Self::HrmpMaxParachainOutboundChannels => 7,
            Self::HrmpMaxParachainInboundChannels => 8,
            Self::HrmpChannelMaxCapacity => 9,
            Self::HrmpChannelMaxMessageSize => 10,
            Self::SchedulingLookahead => 11,
            Self::MaxValidatorsPerCore => 12,
            Self::MaxValidators => 13,
            Self::DisputePeriod => 14,
            Self::NoShowSlots => 18,
            Self::NDelayTranches => 19,
            Self::NeededApprovals => 24,
            Self::ParasAvailabilityPeriod => 27,
            Self::GroupRotationFrequency => 28,
            Self::OnDemandCores => 29,
            Self::MaxPovSize => 35,
            Self::MinimumBackingVotes => 52,
        }
    }
}

/// Fetch the raw `ActiveConfig` storage and return the hex-encoded bytes.
pub async fn get_raw(client: &SharedRpcClient) -> Result<Bytes> {
    let key = StorageKey(hex::decode(ACTIVE_CONFIG_KEY)?);
    let bytes = client
        .state_get_storage(&key, None)
        .await
        .context("failed to fetch ActiveConfig storage")?
        .context("ActiveConfig storage not found on-chain")?;
    Ok(bytes)
}

/// Fetch and display the `ActiveConfig` in a human-readable format.
///
/// Since `HostConfiguration` has a generic `BlockNumber` and many fields,
/// we decode field-by-field from the raw SCALE bytes. This avoids importing
/// the full parachains crate.
pub async fn get(client: &SharedRpcClient) -> Result<BTreeMap<String, String>> {
    let raw = get_raw(client).await?;
    let mut cursor = &raw.0[..];

    let mut fields = BTreeMap::new();

    // Decode in struct field order (from configuration.rs).
    // All u32 fields first, then Balance (u128), then BlockNumber (u32), etc.
    macro_rules! decode_field {
        ($name:literal, u32) => {
            fields.insert($name.to_string(), u32::decode(&mut cursor).map(|v| v.to_string()).unwrap_or_else(|_| "?".into()));
        };
        ($name:literal, u64) => {
            fields.insert($name.to_string(), u64::decode(&mut cursor).map(|v| v.to_string()).unwrap_or_else(|_| "?".into()));
        };
        ($name:literal, u128) => {
            fields.insert($name.to_string(), u128::decode(&mut cursor).map(|v| v.to_string()).unwrap_or_else(|_| "?".into()));
        };
        ($name:literal, option_u32) => {
            fields.insert($name.to_string(), Option::<u32>::decode(&mut cursor).map(|v| format!("{v:?}")).unwrap_or_else(|_| "?".into()));
        };
        ($name:literal, skip $n:expr) => {
            if cursor.len() >= $n {
                cursor = &cursor[$n..];
                fields.insert($name.to_string(), format!("<{} bytes>", $n));
            }
        };
    }

    // HostConfiguration<BlockNumber> field order in v1.1.0:
    decode_field!("max_code_size", u32);
    decode_field!("max_head_data_size", u32);
    decode_field!("max_upward_queue_count", u32);
    decode_field!("max_upward_queue_size", u32);
    decode_field!("max_upward_message_size", u32);
    decode_field!("max_upward_message_num_per_candidate", u32);
    decode_field!("hrmp_max_message_num_per_candidate", u32);
    decode_field!("validation_upgrade_cooldown", u32); // BlockNumber = u32
    decode_field!("validation_upgrade_delay", u32);
    // AsyncBackingParams { max_candidate_depth: u32, allowed_ancestry_len: u32 }
    decode_field!("async_backing.max_candidate_depth", u32);
    decode_field!("async_backing.allowed_ancestry_len", u32);
    decode_field!("max_pov_size", u32);
    decode_field!("max_downward_message_size", u32);
    decode_field!("hrmp_max_parachain_outbound_channels", u32);
    // hrmp_sender_deposit: Balance (u128)
    decode_field!("hrmp_sender_deposit", u128);
    // hrmp_recipient_deposit: Balance
    decode_field!("hrmp_recipient_deposit", u128);
    decode_field!("hrmp_channel_max_capacity", u32);
    decode_field!("hrmp_channel_max_total_size", u32);
    decode_field!("hrmp_max_parachain_inbound_channels", u32);
    decode_field!("hrmp_channel_max_message_size", u32);
    // ExecutorParams: Vec<ExecutorParam> — variable length, skip for now
    // We'll just show remaining bytes
    // For a robust implementation we'd need the full type, but for display
    // purposes we decode what we can and note the rest.

    // Try to decode executor_params length to skip over it.
    if let Ok(len) = codec::Compact::<u32>::decode(&mut cursor) {
        let param_count = len.0;
        fields.insert("executor_params".to_string(), format!("<{param_count} params>"));
        // Each ExecutorParam is an enum — we can't easily skip without full type info.
        // Stop decoding here; the most important fields are above.
        // Add remaining field count note.
        fields.insert("_remaining_bytes".to_string(), format!("{} bytes undecoded", cursor.len()));
    }

    // For the key fields that come after executor_params, we use runtime API
    // queries instead, which are more reliable.
    // Specifically, scheduling_lookahead comes much later in the struct.

    Ok(fields)
}

/// Build the call data for setting a config field to a u32 value.
///
/// The call is `configuration.set_<field>(value)` wrapped in `sudo.sudo(call)`.
pub fn build_set_call(indices: &CallIndices, field: ConfigField, value: u32) -> Vec<u8> {
    let mut inner_call = Vec::new();
    inner_call.push(indices.configuration_pallet);
    inner_call.push(field.call_index());
    value.encode_to(&mut inner_call);

    extrinsic::encode_sudo(indices, &inner_call)
}

/// Set a configuration field value on-chain via sudo.
pub async fn set(
    url: &str,
    field: ConfigField,
    value: u32,
    sudo_seed: &str,
    dry_run: bool,
) -> Result<String> {
    let pair = sr25519::Pair::from_string(sudo_seed, None)
        .map_err(|e| anyhow::anyhow!("invalid sudo seed: {e:?}"))?;

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

    let account_ss58 = sp_core::crypto::Ss58Codec::to_ss58check(&pair.public());
    let nonce: u32 = client
        .system_account_next_index(&account_ss58)
        .await
        .context("failed to fetch nonce")?;

    let indices = CallIndices::default();
    let call_data = build_set_call(&indices, field, value);

    let extrinsic = extrinsic::build_signed_extrinsic(
        &call_data,
        &pair,
        nonce,
        runtime_version.spec_version,
        runtime_version.transaction_version,
        genesis_hash,
        finalized_hash,
    );

    if dry_run {
        let hex = format!("0x{}", hex::encode(&extrinsic));
        return Ok(format!("dry run — extrinsic: {hex}"));
    }

    let block_hash = extrinsic::submit_and_watch(&client, &extrinsic).await?;
    Ok(format!(
        "config change submitted, finalized in block {block_hash:?} (takes effect after 2 sessions)"
    ))
}
