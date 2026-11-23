# The three Rust images share one compile stage: bake builds it once and the three targets in parallel.
#   docker buildx bake -f deploy/docker-bake.hcl            (from the repository root)
# CI selects the build platform for each native runner; local builds use the builder's platform.

group "default" {
  targets = ["api", "worker", "imgproxy"]
}

group "images" {
  targets = ["api", "worker", "imgproxy", "web"]
}

# CI injects the same OCI labels into all four images.
target "docker-metadata-action" {}

target "_rust" {
  inherits   = ["docker-metadata-action"]
  context    = "."
  dockerfile = "deploy/Dockerfile"
}

target "api" {
  inherits = ["_rust"]
  target   = "api"
}

target "worker" {
  inherits = ["_rust"]
  target   = "worker"
}

target "imgproxy" {
  inherits = ["_rust"]
  target   = "imgproxy"
}

target "web" {
  inherits   = ["docker-metadata-action"]
  context    = "."
  dockerfile = "deploy/web.Dockerfile"
}
