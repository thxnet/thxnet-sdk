#!/usr/bin/env bash
# boot-3val-forknet.sh — 3-validator + 2-collator forknet using v1.12.0 leaf at genesis
set -uo pipefail

ROOT=/mnt/HC_Volume_105402799/worktrees/thxnet-release-v1.12-test
RUN=$ROOT/forknet/run-3val
mkdir -p $RUN/state $RUN/logs $RUN/pids
rm -rf $RUN/state/* $RUN/logs/* $RUN/pids/* 2>/dev/null || true

POLKADOT=$ROOT/target/release/polkadot
LEAFCHAIN=$ROOT/ci-artefacts/binaries/thxnet-leafchain
PARA_JSON=$ROOT/forknet/w6-t3-verify-v1.12.0.json
RELAY_JSON=$ROOT/forknet/forked-thxnet-testnet-3val.json
SEED=/data/forknet-test/rootchain-seed
ALICE_PEER=12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp
ALICE_BOOTNODE=/ip4/127.0.0.1/tcp/40331/p2p/$ALICE_PEER
PARA_ALICE_PEER=12D3KooWHdiAxVd8uMQR1hGWXccidmfCwLqcMpGwR6QcTP6QRMuD
PARA_ALICE_BOOTNODE=/ip4/127.0.0.1/tcp/40334/p2p/$PARA_ALICE_PEER
# Deterministic peer IDs derived from BOB_NODE_KEY=0x..0004 and CHARLIE_NODE_KEY=0x..0005.
# Verified against the previous boot's "Local node identity" log.
BOB_PEER=12D3KooWSsChzF81YDUKpe9Uk5AHV5oqAaXAcWNSPYgoLauUk4st
CHARLIE_PEER=12D3KooWSuTq6MG9gPt7qZqLFKkYrfxMewTZhj9nmRHJkPwzWDG2

ALICE_NODE_KEY=0000000000000000000000000000000000000000000000000000000000000001
PARA_ALICE_NODE_KEY=0000000000000000000000000000000000000000000000000000000000000002
PARA_BOB_NODE_KEY=0000000000000000000000000000000000000000000000000000000000000003
# Bob and Charlie need stable peer IDs too: with random node keys (via
# --unsafe-force-node-key-generation) the parachain validation peer-set never
# opens between Alice ↔ Bob (authority-discovery DHT propagation lags),
# causing "Cluster has too many pending statements" and stalling backing.
BOB_NODE_KEY=0000000000000000000000000000000000000000000000000000000000000004
CHARLIE_NODE_KEY=0000000000000000000000000000000000000000000000000000000000000005

# aura keystore for sand-Alice/Bob
AURA_TYPE=61757261
ALICE_SR25519=d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d
BOB_SR25519=8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48

ts() { date '+%H:%M:%S'; }
log() { echo "[$(ts)] $*"; }

log "=== Step 1: regenerate forked relay spec via fork-genesis (substrate-aligned v1.12.0 polkadot) ==="
# v1.12.0 alignment: the binary's compiled-in runtime is v1.12.0, so the fresh
# GenesisConfig layout matches v1.12.0. We force `:code` to the v1.12.0 testnet
# rootchain wasm so the relay boots directly at v1.12.0 (no separate setCode
# pre-flight). Without --runtime-wasm, filtered's old livenet :code (spec
# 94000004) would survive merge while storage is laid out for v1.12.0 → boot panic.
#
# v1.12.0 polkadot CLI no longer registers `thxnet-testnet` as a built-in
# chain id; only `thxnet-dev` / `thxnet-local` ship in command.rs. We feed in
# the previous-run baseline JSON (id="thxnet_testnet") so fork-genesis dispatches
# through `select_runtime_and_assemble_fresh` to the testnet builder. The actual
# state still comes from `--base-path=$SEED`.
RELAY_WASM=$ROOT/ci-artefacts/wasm-runtimes/thxnet-testnet-runtime/thxnet_testnet_runtime.compact.compressed.wasm
INPUT_SPEC=$ROOT/forknet/forked-thxnet-testnet-baseline.json
rm -f "$RELAY_JSON"
"$POLKADOT" fork-genesis \
    --chain="$INPUT_SPEC" \
    --base-path=$SEED \
    --database=rocksdb \
    --register-leafchain="1003:$PARA_JSON" \
    --leafchain-binary="$LEAFCHAIN" \
    --runtime-wasm="$RELAY_WASM" \
    --output="$RELAY_JSON" 2>&1 | tail -10
[[ -f "$RELAY_JSON" ]] || { echo "FATAL: relay spec missing"; exit 1; }
log "  relay spec regenerated: $(wc -c < $RELAY_JSON) bytes"

log "=== Step 2: boot Alice (relay) ==="
# `-l` filters: enable parachain.collator-protocol + parachain.backing + scheduler
# at debug to diagnose the silent inclusion gap; everything else stays at default.
# `--public-addr` forces authority-discovery to publish loopback addresses so
# group peers can actually reach each other on the validation peer-set
# (without it, AD may publish unroutable interface addresses).
RELAY_LOG_FILTER="parachain::collator-protocol=debug,parachain::backing=debug,parachain::statement-distribution=debug,parachain::candidate-validation=info,parachain::scheduler=debug,parachain::approval-distribution=info,parachain::availability-distribution=info,parachain::gossip-support=debug,sub-authority-discovery=debug"
"$POLKADOT" --alice \
    --base-path=$RUN/state/relay-alice \
    --chain=$RELAY_JSON \
    --port=40331 --rpc-port=9931 \
    --node-key=$ALICE_NODE_KEY \
    --public-addr=/ip4/127.0.0.1/tcp/40331/p2p/$ALICE_PEER \
    --allow-private-ip \
    --discover-local \
    --rpc-methods=Unsafe --rpc-cors=all \
    --no-prometheus --no-telemetry --no-mdns \
    --force-authoring \
    -l "$RELAY_LOG_FILTER" \
    >$RUN/logs/relay-alice.log 2>&1 &
echo $! > $RUN/pids/relay-alice.pid
sleep 10

log "=== Step 3: boot Bob (relay) ==="
# Static node-keys avoid the parachain validation peer-set never opening
# (authority-discovery propagation lags behind the first collation when Bob's
# peer ID is random across boots).
"$POLKADOT" --bob \
    --base-path=$RUN/state/relay-bob \
    --chain=$RELAY_JSON \
    --port=40332 --rpc-port=9932 \
    --bootnodes=$ALICE_BOOTNODE \
    --node-key=$BOB_NODE_KEY \
    --public-addr=/ip4/127.0.0.1/tcp/40332/p2p/$BOB_PEER \
    --allow-private-ip \
    --discover-local \
    --rpc-methods=Unsafe --rpc-cors=all \
    --no-prometheus --no-telemetry --no-mdns \
    --force-authoring \
    -l "$RELAY_LOG_FILTER" \
    >$RUN/logs/relay-bob.log 2>&1 &
echo $! > $RUN/pids/relay-bob.pid

log "=== Step 4: boot Charlie (relay - the new 3rd validator) ==="
"$POLKADOT" --charlie \
    --base-path=$RUN/state/relay-charlie \
    --chain=$RELAY_JSON \
    --port=40333 --rpc-port=9933 \
    --bootnodes=$ALICE_BOOTNODE \
    --node-key=$CHARLIE_NODE_KEY \
    --public-addr=/ip4/127.0.0.1/tcp/40333/p2p/$CHARLIE_PEER \
    --allow-private-ip \
    --discover-local \
    --rpc-methods=Unsafe --rpc-cors=all \
    --no-prometheus --no-telemetry --no-mdns \
    --force-authoring \
    -l "$RELAY_LOG_FILTER" \
    >$RUN/logs/relay-charlie.log 2>&1 &
echo $! > $RUN/pids/relay-charlie.pid

log "  Waiting 30s for relay to finalize..."
sleep 30

# Wait for relay finalized #1
DEADLINE=$(($(date +%s) + 60))
RELAY_FINAL=0
while [ $(date +%s) -lt $DEADLINE ]; do
  if grep -qE "finalized #[1-9]" $RUN/logs/relay-alice.log 2>/dev/null; then
    RELAY_FINAL=1; break
  fi
  sleep 3
done
if [ $RELAY_FINAL -eq 0 ]; then
  echo "FATAL: relay didn't finalize"; exit 1
fi
log "  relay finalized OK"

log "=== Step 5: insert aura keys for collators ==="
mkdir -p $RUN/state/sand-alice/chains/sand_testnet/keystore
mkdir -p $RUN/state/sand-bob/chains/sand_testnet/keystore
echo -n '"//Alice"' > $RUN/state/sand-alice/chains/sand_testnet/keystore/${AURA_TYPE}${ALICE_SR25519}
echo -n '"//Bob"'   > $RUN/state/sand-bob/chains/sand_testnet/keystore/${AURA_TYPE}${BOB_SR25519}

log "=== Step 6: 60s grace for relay-light-client baseline ==="
sleep 60

log "=== Step 7: boot collators ==="
COLLATOR_LOG_FILTER="parachain::collator-protocol=debug,parachain::collation-generation=debug,parachain::network-bridge=info,aura::cumulus=info,cumulus-pov-recovery=info"
"$LEAFCHAIN" --collator --alice \
    --base-path=$RUN/state/sand-alice \
    --chain=$PARA_JSON \
    --port=40334 --rpc-port=9934 \
    --rpc-methods=Unsafe --rpc-cors=all \
    --node-key=$PARA_ALICE_NODE_KEY \
    --force-authoring \
    --no-prometheus --no-telemetry --no-mdns \
    -l "$COLLATOR_LOG_FILTER" \
    -- \
    --chain=$RELAY_JSON \
    --base-path=$RUN/state/sand-alice/relay \
    --port=40335 --rpc-port=9935 \
    --bootnodes=$ALICE_BOOTNODE \
    --no-prometheus --no-telemetry \
    >$RUN/logs/sand-alice.log 2>&1 &
echo $! > $RUN/pids/sand-alice.pid

sleep 5

"$LEAFCHAIN" --collator --bob \
    --base-path=$RUN/state/sand-bob \
    --chain=$PARA_JSON \
    --port=40336 --rpc-port=9936 \
    --rpc-methods=Unsafe --rpc-cors=all \
    --node-key=$PARA_BOB_NODE_KEY \
    --bootnodes=$PARA_ALICE_BOOTNODE \
    --force-authoring \
    --no-prometheus --no-telemetry --no-mdns \
    -- \
    --chain=$RELAY_JSON \
    --base-path=$RUN/state/sand-bob/relay \
    --port=40337 --rpc-port=9937 \
    --bootnodes=$ALICE_BOOTNODE \
    --no-prometheus --no-telemetry \
    >$RUN/logs/sand-bob.log 2>&1 &
echo $! > $RUN/pids/sand-bob.pid

log "=== Step 8: boot complete. Polling para advance ==="
START=$(date +%s)
LAST_PARA=""
while [ $(($(date +%s) - START)) -lt 300 ]; do
  P=$(curl -s -m 3 -X POST http://localhost:9934 -H 'Content-Type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"chain_getHeader"}' 2>&1 | grep -oE '"number":"0x[0-9a-f]+"' | head -1)
  R=$(curl -s -m 3 -X POST http://localhost:9931 -H 'Content-Type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"chain_getHeader"}' 2>&1 | grep -oE '"number":"0x[0-9a-f]+"' | head -1)
  if [ "$P" != "$LAST_PARA" ]; then
    log "  [+$(($(date +%s) - START))s] relay=$R para=$P"
    LAST_PARA=$P
  fi
  if [[ "$P" == *'"0x5"' || "$P" == *'"0xa"' || "$P" == *'"0xf"' || "$P" == *'"0x14"' ]]; then
    log "  PARA ADVANCED PAST #1 to $P — bug fix verified on 3-validator forknet"
    exit 0
  fi
  sleep 5
done
log "PARA STILL STUCK — check logs"
exit 2
