#!/usr/bin/env bash
# Prune sccache branch caches for deleted git branches.
# Install as weekly cron on the Hetzner runner host:
#   sudo cp sccache-gc.sh /etc/cron.weekly/sccache-gc
#   sudo chmod +x /etc/cron.weekly/sccache-gc
#
# Protected branches (main, develop) are never pruned.
# Other branch caches are pruned if the branch no longer exists on origin
# AND the cache directory hasn't been touched in over 14 days.

SCCACHE_BASE="/home/runner/.cache/sccache"
REPO_DIR="/home/runner/actions-runner/_work/thxnet-sdk/thxnet-sdk"
MAX_AGE_DAYS=14
PROTECTED_BRANCHES="main develop"

# Ensure we can access the repo for git ls-remote
if [ ! -d "$REPO_DIR/.git" ]; then
  # Try alternate locations
  for dir in /home/runner/actions-runner-*/thxnet-sdk/thxnet-sdk; do
    if [ -d "$dir/.git" ]; then
      REPO_DIR="$dir"
      break
    fi
  done
fi

pruned=0
kept=0

for dir in "${SCCACHE_BASE}"/*/; do
  [ -d "$dir" ] || continue
  branch=$(basename "$dir")

  # Skip protected branches
  for protected in $PROTECTED_BRANCHES; do
    if [ "$branch" = "$protected" ]; then
      kept=$((kept + 1))
      continue 2
    fi
  done

  # Check age
  age_days=$(( ($(date +%s) - $(stat -c %Y "$dir" 2>/dev/null || echo 0)) / 86400 ))
  if [ "$age_days" -lt "$MAX_AGE_DAYS" ]; then
    kept=$((kept + 1))
    continue
  fi

  # Check if branch still exists on remote
  if [ -d "$REPO_DIR/.git" ]; then
    if git -C "$REPO_DIR" ls-remote --heads origin "$branch" 2>/dev/null | grep -q "$branch"; then
      kept=$((kept + 1))
      continue
    fi
  fi

  # Branch is gone and cache is stale — prune
  size=$(du -sh "$dir" 2>/dev/null | cut -f1)
  echo "Pruning: $branch (${size}, ${age_days} days old)"
  rm -rf "$dir"
  pruned=$((pruned + 1))
done

echo "sccache-gc: pruned=$pruned kept=$kept"
