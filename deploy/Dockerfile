# Rust services: api, imgproxy, worker. One build stage, three small runtime images.
#   podman build -f deploy/Dockerfile --target api -t boothdex-api .   (all three at once: deploy/docker-bake.hcl)
# Dependencies are compiled in their own layer (cargo-chef), so a change to our code only recompiles our crates;
# CI keeps that layer in its build cache. The lindera-ipadic build script downloads the IPADIC dictionary
# (network needed; it fails now and then, so it is retried twice) and embeds it in the api binary.
FROM docker.io/library/rust:1-bookworm AS chef
RUN cargo install cargo-chef --version 0.1.78 --locked
WORKDIR /src

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS build
COPY --from=planner /src/recipe.json recipe.json
RUN for i in 1 2 3; do cargo chef cook --release --recipe-path recipe.json -p api -p imgproxy -p worker && break; [ $i = 3 ] && exit 1; sleep 10; done
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release -p api -p imgproxy -p worker \
 && mkdir -p /out && cp target/release/api target/release/imgproxy target/release/worker /out/

FROM docker.io/library/debian:bookworm-slim AS runtime
# ca-certificates: TLS to booth.pm / pximg / AI provider (rustls uses the system trust store)
# writable mount points for the non-root user (named volumes inherit this ownership)
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home /app app \
 && mkdir -p /index /cache && chown app:app /index /cache
WORKDIR /app
USER app

FROM runtime AS api
COPY --from=build /out/api /usr/local/bin/api
ENV BIND=0.0.0.0:3000 INDEX_DIR=/index
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
