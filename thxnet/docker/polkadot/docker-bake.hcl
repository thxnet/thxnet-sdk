variable "TAG" {
  default = "develop"
}

variable "CONTAINER_REGISTRY" {
  default = "ghcr.io/thxnet"
}

# Stamps the binary with the commit hash. MUST be set by the caller — e.g.
#   SUBSTRATE_CLI_GIT_COMMIT_HASH=$(git rev-parse HEAD) docker buildx bake
# The builder Dockerfile fails the build if this is empty, preventing
# untraceable "Version: X.Y.Z-unknown" binaries from escaping into the wild.
variable "SUBSTRATE_CLI_GIT_COMMIT_HASH" {
  default = ""
}

group "default" {
  targets = [
    "rootchain",
  ]
}

target "rootchain" {
  dockerfile = "thxnet/docker/polkadot/polkadot_builder.Dockerfile"
  target     = "rootchain"
  tags       = ["${CONTAINER_REGISTRY}/rootchain:${TAG}"]
  platforms  = ["linux/amd64"]
  args = {
    SUBSTRATE_CLI_GIT_COMMIT_HASH = "${SUBSTRATE_CLI_GIT_COMMIT_HASH}"
  }
  labels = {
    "description"                 = "Container image for THXNET."
    "io.thxnet.image.type"        = "final"
    "io.thxnet.image.authors"     = "contact@thxlab.io"
    "io.thxnet.image.vendor"      = "thxlab.io"
    "io.thxnet.image.description" = "THXNET.: The Hybrid Next-Gen Blockchain Network"
  }
  contexts = {
    ci-linux = "docker-image://docker.io/paritytech/ci-unified:bullseye-1.70.0-2023-05-23-v20230706"
    ubuntu   = "docker-image://docker.io/library/ubuntu:20.04"
  }
}
