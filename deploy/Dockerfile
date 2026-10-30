# Rust services: api, imgproxy, worker. One build stage, three small runtime images.
#   podman build -f deploy/Dockerfile --target api -t catalogue-api .
# The lindera-ipadic build script downloads the IPADIC dictionary at build time (network needed;
# it has failed intermittently — just rebuild). The dictionary is then embedded in the api binary.
FROM docker.io/library/rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release -p api -p imgproxy -p worker \
 && mkdir -p /out && cp target/release/api target/release/imgproxy target/release/worker /out/

FROM docker.io/library/debian:bookworm-slim AS runtime
# ca-certificates: TLS to booth.pm / pximg / AI provider (rustls uses the system trust store)
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home /app app
WORKDIR /app
USER app

FROM runtime AS api
COPY --from=build /out/api /usr/local/bin/api
ENV BIND=0.0.0.0:3000
EXPOSE 3000
CMD ["api"]

FROM runtime AS imgproxy
COPY --from=build /out/imgproxy /usr/local/bin/imgproxy
ENV BIND=0.0.0.0:3001 CACHE_DIR=/cache
EXPOSE 3001
CMD ["imgproxy"]

FROM runtime AS worker
COPY --from=build /out/worker /usr/local/bin/worker
CMD ["worker", "run"]
