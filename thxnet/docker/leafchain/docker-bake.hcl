variable "TAG" {
  default = "develop"
}

variable "CONTAINER_REGISTRY" {
  default = "ghcr.io/thxnet"
}

group "default" {
  targets = [
    "leafchain",
  ]
}

target "leafchain" {
  dockerfile = "thxnet/docker/leafchain/leafchain_builder.Dockerfile"
  target     = "leafchain"
  tags       = ["${CONTAINER_REGISTRY}/leafchain:${TAG}"]
  platforms  = ["linux/amd64"]
  args       = {}
  labels = {
    "description"                 = "Container image for THXNET. leafchain"
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
