variable "TAG" {
  default = "develop"
}

variable "CONTAINER_REGISTRY" {
  default = "ghcr.io/thxnet"
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
  args       = {}
  labels = {
    "description"                 = "Container image for THXNET."
    "io.thxnet.image.type"        = "final"
    "io.thxnet.image.authors"     = "contact@thxlab.io"
    "io.thxnet.image.vendor"      = "thxlab.io"
    "io.thxnet.image.description" = "THXNET.: The Hybrid Next-Gen Blockchain Network"
  }
  contexts = {
    ci-linux = "docker-image://docker.io/paritytech/ci-linux:production"
    ubuntu   = "docker-image://docker.io/library/ubuntu:20.04"
  }
}
