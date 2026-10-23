#!/usr/bin/env bash
set -uo pipefail
bin=$1 chain=$2 out=$3
cd "$(git rev-parse --show-toplevel)"
set -a; . thxnet/ci/rbv-smoke.env; set +a
port=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')
mkdir -p "$out"
"$bin" --version > "$out/version.txt"
force=$("$bin" --help 2>/dev/null | grep -q -- --unsafe-force-node-key-generation && echo --unsafe-force-node-key-generation)
"$bin" --chain "$chain" --tmp --alice ${force:+$force} --rpc-port "$port" --no-telemetry --no-mdns \
  --listen-addr /ip4/127.0.0.1/tcp/0 --no-prometheus > "$out/node.log" 2>&1 &
pid=$!
trap 'kill $pid 2>/dev/null' EXIT
rpc() { curl -s -m 5 -H 'content-type: application/json' -d "{\"id\":1,\"jsonrpc\":\"2.0\",\"method\":\"$1\",\"params\":[${2:-}]}" "http://127.0.0.1:$port"; }
best=0 fin=0
for t in $(seq "$SMOKE_SAMPLE_SEC" "$SMOKE_SAMPLE_SEC" "$SMOKE_SEC"); do
  sleep "$SMOKE_SAMPLE_SEC"
  kill -0 $pid 2>/dev/null || { echo "{\"t\":$t,\"error\":\"node exited\"}" >> "$out/samples.ndjson"; break; }
  b=$(rpc chain_getHeader | jq -r '.result.number // "0x0"')
  fh=$(rpc chain_getFinalizedHead | jq -r '.result // empty')
  f=$( [ -n "$fh" ] && rpc chain_getHeader "\"$fh\"" | jq -r '.result.number // "0x0"' || echo 0x0)
  best=$((b)) fin=$((f))
  echo "{\"t\":$t,\"best\":$best,\"finalized\":$fin}" >> "$out/samples.ndjson"
done
panics=$(grep -ciE 'panicked|thread .* panicked' "$out/node.log")
verdict=PASS
[ "$best" -ge "$SMOKE_MIN_BEST" ] && [ "$fin" -ge "$SMOKE_MIN_FINALIZED" ] && [ "$panics" -eq 0 ] || verdict=FAIL
echo "{\"chain\":\"$chain\",\"version\":\"$(head -1 "$out/version.txt")\",\"best\":$best,\"finalized\":$fin,\"panics\":$panics,\"minBest\":$SMOKE_MIN_BEST,\"minFinalized\":$SMOKE_MIN_FINALIZED,\"verdict\":\"$verdict\"}" | tee "$out/result.json"
[ "$verdict" = PASS ]
