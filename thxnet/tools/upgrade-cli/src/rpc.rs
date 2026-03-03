//! JSON-RPC client wrapper built on jsonrpsee 0.16.
//!
//! Follows the same `SharedRpcClient` + `#[rpc(client)]` trait pattern used
//! by the upstream `staking-miner` utility, adapted for upgrade-specific RPC
//! methods.

use crate::types::{Hash, SystemHealth, SyncState};
use codec::Decode;
use jsonrpsee::{
    core::{Error as RpcError, RpcResult},
    proc_macros::rpc,
    ws_client::{WsClient, WsClientBuilder},
};
use sc_transaction_pool_api::TransactionStatus;
use sp_core::{storage::StorageKey, Bytes};
use sp_version::RuntimeVersion;
use std::{ops::Deref, sync::Arc, time::Duration};

/// Errors that can occur during RPC helper operations.
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum RpcHelperError {
    #[error("RPC error: {0}")]
    Rpc(#[from] RpcError),

    #[error("SCALE decode error: {0}")]
    Codec(#[from] codec::Error),

    #[error("storage item not found for key {0}")]
    StorageNotFound(String),
}

/// JSON-RPC API surface used by the upgrade CLI.
///
/// The `#[rpc(client)]` macro generates an `RpcApiClient` trait with async
/// methods that can be called on any `jsonrpsee` client.
#[rpc(client)]
pub trait RpcApi {
    /// Fetch the chain name (e.g. "THXnet Testnet").
    #[method(name = "system_chain")]
    async fn system_chain(&self) -> RpcResult<String>;

    /// Fetch system health (peers, syncing status).
    #[method(name = "system_health")]
    async fn system_health(&self) -> RpcResult<serde_json::Value>;

    /// Fetch sync state (current/highest block numbers).
    #[method(name = "system_syncState")]
    async fn system_sync_state(&self) -> RpcResult<serde_json::Value>;

    /// Read a storage value by key, optionally at a specific block.
    #[method(name = "state_getStorage")]
    async fn state_get_storage(
        &self,
        key: &StorageKey,
        at: Option<Hash>,
    ) -> RpcResult<Option<Bytes>>;

    /// Invoke a runtime API function and return the SCALE-encoded result.
    #[method(name = "state_call")]
    async fn state_call(
        &self,
        method: &str,
        data: &Bytes,
        at: Option<Hash>,
    ) -> RpcResult<Bytes>;

    /// Fetch the on-chain runtime version.
    #[method(name = "state_getRuntimeVersion")]
    async fn state_get_runtime_version(
        &self,
        at: Option<Hash>,
    ) -> RpcResult<RuntimeVersion>;

    /// Fetch the hash of the n-th block. Returns latest if `None`.
    #[method(name = "chain_getBlockHash")]
    async fn chain_get_block_hash(
        &self,
        number: Option<u64>,
    ) -> RpcResult<Option<Hash>>;

    /// Fetch the hash of the latest finalized block.
    #[method(name = "chain_getFinalizedHead")]
    async fn chain_get_finalized_head(&self) -> RpcResult<Hash>;

    /// Fetch the header for a given block hash.
    #[method(name = "chain_getHeader")]
    async fn chain_get_header(
        &self,
        hash: Option<Hash>,
    ) -> RpcResult<Option<crate::types::Header>>;

    /// Fetch the next nonce for an account.
    #[method(name = "system_accountNextIndex")]
    async fn system_account_next_index(
        &self,
        account: &str,
    ) -> RpcResult<u32>;

    /// Submit an extrinsic and subscribe to its lifecycle events.
    #[subscription(
        name = "author_submitAndWatchExtrinsic" => "author_extrinsicUpdate",
        unsubscribe = "author_unwatchExtrinsic",
        item = TransactionStatus<Hash, Hash>
    )]
    fn author_submit_and_watch_extrinsic(&self, bytes: &Bytes);
}

/// Thread-safe, cloneable RPC client wrapper.
///
/// Wraps `jsonrpsee::WsClient` in an `Arc` for cheap cloning across async
/// tasks. Implements `Deref` to the inner client so all `RpcApiClient`
/// methods are directly callable.
#[derive(Clone, Debug)]
pub struct SharedRpcClient {
    inner: Arc<WsClient>,
    uri: String,
}

impl Deref for SharedRpcClient {
    type Target = WsClient;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl SharedRpcClient {
    /// Connect to a WebSocket RPC endpoint.
    pub async fn new(uri: &str) -> Result<Self, RpcError> {
        let client = WsClientBuilder::default()
            .connection_timeout(Duration::from_secs(30))
            .request_timeout(Duration::from_secs(120))
            .max_request_body_size(u32::MAX)
            .max_concurrent_requests(64)
            .build(uri)
            .await?;

        Ok(Self {
            inner: Arc::new(client),
            uri: uri.to_owned(),
        })
    }

    /// The URI this client is connected to.
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Fetch a storage value and SCALE-decode it.
    ///
    /// Returns `Ok(None)` if the storage key does not exist on-chain.
    #[allow(dead_code)]
    pub async fn get_storage_decoded<T: Decode>(
        &self,
        key: &StorageKey,
        at: Option<Hash>,
    ) -> Result<Option<T>, RpcHelperError> {
        match self.state_get_storage(key, at).await? {
            Some(bytes) => {
                let decoded = T::decode(&mut &bytes.0[..])?;
                Ok(Some(decoded))
            }
            None => Ok(None),
        }
    }

    /// Fetch a storage value and SCALE-decode it, returning an error if the
    /// key does not exist.
    #[allow(dead_code)]
    pub async fn require_storage_decoded<T: Decode>(
        &self,
        key: &StorageKey,
        at: Option<Hash>,
    ) -> Result<T, RpcHelperError> {
        self.get_storage_decoded(key, at)
            .await?
            .ok_or_else(|| RpcHelperError::StorageNotFound(hex::encode(&key.0)))
    }

    /// Call a runtime API method and SCALE-decode the result.
    pub async fn runtime_call_decoded<T: Decode>(
        &self,
        method: &str,
        at: Option<Hash>,
    ) -> Result<T, RpcHelperError> {
        let result = self.state_call(method, &Bytes(Vec::new()), at).await?;
        let decoded = T::decode(&mut &result.0[..])?;
        Ok(decoded)
    }

    /// Fetch `system_health` and deserialize into our typed struct.
    pub async fn health(&self) -> Result<SystemHealth, RpcHelperError> {
        let value = self.system_health().await?;
        serde_json::from_value(value).map_err(|e| {
            RpcHelperError::Rpc(RpcError::Custom(format!("deserialize system_health: {e}")))
        })
    }

    /// Fetch `system_syncState` and deserialize into our typed struct.
    #[allow(dead_code)]
    pub async fn sync_state(&self) -> Result<SyncState, RpcHelperError> {
        let value = self.system_sync_state().await?;
        serde_json::from_value(value).map_err(|e| {
            RpcHelperError::Rpc(RpcError::Custom(format!("deserialize system_syncState: {e}")))
        })
    }
}
