# syntax=docker/dockerfile:1

# ---- Builder ----
# Same Debian release as the runtime stage (bookworm), so the compiled
# binary's glibc dependency matches exactly - no runtime "GLIBC_x.xx not
# found" mismatch between build and run environments.
FROM rust:1-slim-bookworm AS builder
WORKDIR /app

# Cache the dependency graph in its own layer: with only Cargo.toml/lock
# copied, a throwaway `main.rs` lets `cargo build --release` compile every
# dependency once. Later layers that only change application source reuse
# this layer instead of recompiling the whole dependency tree each time.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

# Migrations are embedded into the binary at compile time via `include_str!`
# (see src/db/mod.rs), so they must be present for this build but are not
# needed again at runtime.
COPY src ./src
COPY migrations ./migrations
RUN touch src/main.rs && cargo build --release

# ---- Runtime ----
# Plain debian-slim, not distroless/scratch: sqlx's "sqlite" feature
# statically bundles SQLite into the binary (see libsqlite3-sys's "cc"
# build-dependency in Cargo.lock), and the "runtime-tokio-rustls" feature
# means TLS is pure-Rust (rustls), not OpenSSL - so nothing beyond libc
# needs to exist in this image. No Rust toolchain, no build tools.
#
# The one deliberate addition is `curl`, used only by docker-compose.yml's
# healthcheck (there's no other HTTP client in this minimal image, and the
# healthcheck is how Compose knows the frontend can safely start).
FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --shell /usr/sbin/nologin appuser \
    && mkdir -p /app/data \
    && chown -R appuser:appuser /app

COPY --from=builder /app/target/release/server /usr/local/bin/server

# Containers must listen on all interfaces - 127.0.0.1 inside a container
# is only reachable from within that same container's network namespace,
# never from the host or another container. DATABASE_URL is set
# explicitly by docker-compose.yml to an absolute path under the mounted
# volume (see there) rather than relying on the app's relative-path
# default lining up with WORKDIR - these ENV values are still this
# image's own sane defaults if run without Compose.
ENV HOST=0.0.0.0
ENV PORT=8080

USER appuser
EXPOSE 8080

CMD ["server"]
