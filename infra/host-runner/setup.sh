#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
set -a; . ./runners.env; set +a
: "${RUNNER_TOKEN:?registration token via env}"
id "$RUNNER_USER" >/dev/null 2>&1 || useradd -m -s /bin/bash -G docker "$RUNNER_USER"
install -d -o "$RUNNER_USER" -g "$RUNNER_USER" "$RUNNER_ROOT" "$RUNNER_ROOT/cache" "$RUNNER_ROOT/target"
tgz="$RUNNER_ROOT/actions-runner-linux-x64-$RUNNER_VERSION.tar.gz"
[ -s "$tgz" ] || curl -sSfL -o "$tgz" "https://github.com/actions/runner/releases/download/v$RUNNER_VERSION/actions-runner-linux-x64-$RUNNER_VERSION.tar.gz"
echo "$RUNNER_SHA256  $tgz" | sha256sum -c -
host=$(hostname -s)
for i in $(seq 1 "$RUNNER_COUNT"); do
  d="$RUNNER_ROOT/runner-$i"
  if [ -f "$d/.runner" ]; then echo "runner-$i already configured"; continue; fi
  install -d -o "$RUNNER_USER" -g "$RUNNER_USER" "$d"
  tar -xzf "$tgz" -C "$d" && chown -R "$RUNNER_USER:$RUNNER_USER" "$d"
  (cd "$d" && sudo -u "$RUNNER_USER" ./config.sh --unattended --url "$RUNNER_ORG_URL" --token "$RUNNER_TOKEN" \
    --name "$host-thxnet-sdk-$i" --runnergroup "$RUNNER_GROUP" --labels "$RUNNER_LABEL" --no-default-labels --work _work --replace)
  (cd "$d" && ./svc.sh install "$RUNNER_USER" && ./svc.sh start)
done
systemctl list-units --no-pager --plain 'actions.runner.*' | grep -c running
