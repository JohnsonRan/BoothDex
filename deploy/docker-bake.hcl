# The three Rust images share one compile stage: bake builds it once and the three targets in parallel.
#   docker buildx bake -f deploy/docker-bake.hcl            (from the repository root)
# CI adds tags and labels per image through the meta-* targets (see .github/workflows/images.yml).

group "default" {
  targets = ["api", "worker", "imgproxy"]
}

target "_rust" {
  context    = "."
  dockerfile = "deploy/Dockerfile"
  platforms  = ["linux/amd64"]
}

# overridden in CI by docker/metadata-action
target "meta-api" {}
target "meta-worker" {}
target "meta-imgproxy" {}

target "api" {
  inherits = ["_rust", "meta-api"]
  target   = "api"
}

target "worker" {
  inherits = ["_rust", "meta-worker"]
  target   = "worker"
}

target "imgproxy" {
  inherits = ["_rust", "meta-imgproxy"]
  target   = "imgproxy"
}
