#!/usr/bin/env bash
# ARC (Actions Runner Controller) setup on Hetzner dedicated server
# Run this on: ssh thxnethetzactionrunner
#
# Prerequisites:
#   - Docker installed and running
#   - User 'runner' (uid=1000) in docker group
#   - GitHub App created with Organization > Self-hosted runners (Read & Write)
#
# Usage:
#   ./setup.sh phase0   # Install k3d + helm, create cluster
#   ./setup.sh phase1   # Install ARC controller
#   ./setup.sh phase2   # Deploy test runner scale sets
#   ./setup.sh phase3   # Deploy production runner scale sets (after validation)
#   ./setup.sh teardown-test  # Remove test scale sets
#   ./setup.sh status   # Show cluster and pod status

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
K3D_CLUSTER_NAME="ci"
ARC_CONTROLLER_NS="arc-systems"
ARC_RUNNERS_NS="arc-runners"
GITHUB_ORG_URL="https://github.com/thxnet"

# ─── Helpers ────────────────────────────────────────────────────────────

info()  { echo "[INFO]  $*"; }
warn()  { echo "[WARN]  $*" >&2; }
error() { echo "[ERROR] $*" >&2; exit 1; }

check_prereqs() {
  command -v docker >/dev/null || error "docker not found"
  docker info >/dev/null 2>&1  || error "Docker daemon not running"
  info "Docker OK"
}

# ─── Phase 0: Install tools + create k3d cluster ───────────────────────

phase0() {
  info "=== Phase 0: Install k3d, helm, create cluster ==="
  check_prereqs

  # Install k3d if not present
  if ! command -v k3d >/dev/null; then
    info "Installing k3d..."
    curl -s https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | bash
  else
    info "k3d already installed: $(k3d version)"
  fi

  # Install helm if not present
  if ! command -v helm >/dev/null; then
    info "Installing helm..."
    curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash
  else
    info "helm already installed: $(helm version --short)"
  fi

  # Install kubectl if not present
  if ! command -v kubectl >/dev/null; then
    info "Installing kubectl..."
    curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
    chmod +x kubectl
    sudo mv kubectl /usr/local/bin/
  else
    info "kubectl already installed: $(kubectl version --client --short 2>/dev/null || kubectl version --client)"
  fi

  # Create k3d cluster
  if k3d cluster list | grep -q "^${K3D_CLUSTER_NAME} "; then
    warn "Cluster '${K3D_CLUSTER_NAME}' already exists"
    info "To recreate: k3d cluster delete ${K3D_CLUSTER_NAME} && $0 phase0"
  else
    info "Creating k3d cluster '${K3D_CLUSTER_NAME}'..."
    k3d cluster create --config "${SCRIPT_DIR}/k3d-config.yaml"
    info "Waiting for node to be ready..."
    kubectl wait --for=condition=Ready node --all --timeout=120s
  fi

  info "Cluster status:"
  kubectl get nodes -o wide
  echo
  info "Phase 0 complete. Next: $0 phase1"
}

# ─── Phase 1: Install ARC controller ───────────────────────────────────

phase1() {
  info "=== Phase 1: Install ARC controller ==="

  helm install arc \
    --namespace "${ARC_CONTROLLER_NS}" \
    --create-namespace \
    oci://ghcr.io/actions/actions-runner-controller-charts/gha-runner-scale-set-controller

  info "Waiting for ARC controller to be ready..."
  kubectl -n "${ARC_CONTROLLER_NS}" wait --for=condition=Ready pod --all --timeout=120s

  info "ARC controller pods:"
  kubectl -n "${ARC_CONTROLLER_NS}" get pods
  echo
  info "Phase 1 complete. Next: create GitHub App secret, then $0 phase2"
  echo
  info "To create the GitHub App secret:"
  info "  kubectl create namespace ${ARC_RUNNERS_NS}"
  info "  kubectl create secret generic github-app-secret \\"
  info "    --namespace ${ARC_RUNNERS_NS} \\"
  info "    --from-literal=github_app_id=<APP_ID> \\"
  info "    --from-literal=github_app_installation_id=<INSTALL_ID> \\"
  info "    --from-file=github_app_private_key=<PRIVATE_KEY_FILE>"
}

# ─── Phase 2: Deploy TEST runner scale sets ─────────────────────────────

phase2() {
  info "=== Phase 2: Deploy test runner scale sets ==="

  # Verify secret exists
  kubectl -n "${ARC_RUNNERS_NS}" get secret github-app-secret >/dev/null 2>&1 \
    || error "Secret 'github-app-secret' not found in namespace '${ARC_RUNNERS_NS}'. Create it first (see phase1 output)."

  # Install test heavy runner scale set
  info "Installing test heavy runner scale set (arc-test-heavy)..."
  helm install arc-test-heavy \
    --namespace "${ARC_RUNNERS_NS}" \
    --set githubConfigUrl="${GITHUB_ORG_URL}" \
    --set githubConfigSecret=github-app-secret \
    --set runnerScaleSetName=arc-test-heavy \
    -f "${SCRIPT_DIR}/values-heavy.yaml" \
    oci://ghcr.io/actions/actions-runner-controller-charts/gha-runner-scale-set

  # Install test light runner scale set
  info "Installing test light runner scale set (arc-test-light)..."
  helm install arc-test-light \
    --namespace "${ARC_RUNNERS_NS}" \
    --set githubConfigUrl="${GITHUB_ORG_URL}" \
    --set githubConfigSecret=github-app-secret \
    --set runnerScaleSetName=arc-test-light \
    -f "${SCRIPT_DIR}/values-light.yaml" \
    oci://ghcr.io/actions/actions-runner-controller-charts/gha-runner-scale-set

  info "Waiting for listener pods..."
  sleep 5
  kubectl -n "${ARC_RUNNERS_NS}" get pods

  echo
  info "Phase 2 complete."
  info "Now trigger the arc-test.yml workflow via GitHub UI or:"
  info "  gh workflow run arc-test.yml"
  info "After validation passes, run: $0 phase3"
}

# ─── Teardown test scale sets ───────────────────────────────────────────

teardown_test() {
  info "=== Tearing down test runner scale sets ==="
  helm uninstall arc-test-heavy -n "${ARC_RUNNERS_NS}" 2>/dev/null && info "Removed arc-test-heavy" || warn "arc-test-heavy not found"
  helm uninstall arc-test-light -n "${ARC_RUNNERS_NS}" 2>/dev/null && info "Removed arc-test-light" || warn "arc-test-light not found"
  info "Teardown complete."
}

# ─── Phase 3: Deploy PRODUCTION runner scale sets ───────────────────────

phase3() {
  info "=== Phase 3: Deploy production runner scale sets ==="
  warn "This will use the SAME labels as existing persistent runners."
  warn "Make sure to stop the old runners FIRST to avoid label conflicts."
  echo
  read -p "Have you stopped the old persistent runners? (yes/no): " confirm
  [[ "$confirm" == "yes" ]] || error "Aborted. Stop old runners first."

  # Remove test scale sets if still present
  teardown_test 2>/dev/null || true

  # Install production heavy runner scale set
  info "Installing production heavy runner scale set (hetzner-thxnet)..."
  helm install hetzner-thxnet \
    --namespace "${ARC_RUNNERS_NS}" \
    --set githubConfigUrl="${GITHUB_ORG_URL}" \
    --set githubConfigSecret=github-app-secret \
    -f "${SCRIPT_DIR}/values-heavy.yaml" \
    oci://ghcr.io/actions/actions-runner-controller-charts/gha-runner-scale-set

  # Install production light runner scale set
  info "Installing production light runner scale set (hetzner-thxnet-light)..."
  helm install hetzner-thxnet-light \
    --namespace "${ARC_RUNNERS_NS}" \
    --set githubConfigUrl="${GITHUB_ORG_URL}" \
    --set githubConfigSecret=github-app-secret \
    -f "${SCRIPT_DIR}/values-light.yaml" \
    oci://ghcr.io/actions/actions-runner-controller-charts/gha-runner-scale-set

  info "Waiting for listener pods..."
  sleep 5
  kubectl -n "${ARC_RUNNERS_NS}" get pods

  echo
  info "Phase 3 complete. Production runners are live."
  info "Trigger a CI run to validate end-to-end."
}

# ─── Status ─────────────────────────────────────────────────────────────

status() {
  info "=== Cluster Status ==="
  echo "--- Nodes ---"
  kubectl get nodes -o wide 2>/dev/null || warn "kubectl not configured"
  echo
  echo "--- ARC Controller ---"
  kubectl -n "${ARC_CONTROLLER_NS}" get pods 2>/dev/null || warn "ARC controller namespace not found"
  echo
  echo "--- Runner Pods ---"
  kubectl -n "${ARC_RUNNERS_NS}" get pods 2>/dev/null || warn "Runner namespace not found"
  echo
  echo "--- Helm Releases ---"
  helm list -A 2>/dev/null || warn "helm not configured"
}

# ─── Main ───────────────────────────────────────────────────────────────

case "${1:-}" in
  phase0)         phase0 ;;
  phase1)         phase1 ;;
  phase2)         phase2 ;;
  phase3)         phase3 ;;
  teardown-test)  teardown_test ;;
  status)         status ;;
  *)
    echo "Usage: $0 {phase0|phase1|phase2|phase3|teardown-test|status}"
    echo
    echo "  phase0         Install k3d + helm, create cluster"
    echo "  phase1         Install ARC controller"
    echo "  phase2         Deploy test runner scale sets (arc-test-heavy/light)"
    echo "  phase3         Deploy production runner scale sets (hetzner-thxnet)"
    echo "  teardown-test  Remove test runner scale sets"
    echo "  status         Show cluster and pod status"
    exit 1
    ;;
esac
