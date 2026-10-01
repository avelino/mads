# syntax=docker/dockerfile:1

# Build stage: Alpine gives a static musl binary with no cross-compile wrapper.
# The toolchain comes from the base image. rust-toolchain.toml is not copied, so
# rustup does not try to download a second 1.98 next to the one in the image.
ARG RUST_VERSION=1.98
FROM rust:${RUST_VERSION}-alpine AS build

RUN apk add --no-cache musl-dev

WORKDIR /src

# Dependencies first. These layers only rebuild when a manifest or Cargo.lock changes.
COPY Cargo.toml Cargo.lock ./
COPY crates/mads-core/Cargo.toml crates/mads-core/Cargo.toml
COPY crates/mads-providers/Cargo.toml crates/mads-providers/Cargo.toml
COPY crates/mads-cli/Cargo.toml crates/mads-cli/Cargo.toml
RUN mkdir -p crates/mads-core/src crates/mads-providers/src crates/mads-cli/src \
 && : > crates/mads-core/src/lib.rs \
 && : > crates/mads-providers/src/lib.rs \
 && echo 'fn main() {}' > crates/mads-cli/src/main.rs \
 && cargo build --release --locked -p mads-cli \
 && rm -rf crates

# The real sources. touch makes cargo see them as newer than the stubs.
COPY crates crates
RUN find crates -name '*.rs' -exec touch {} + \
 && cargo build --release --locked -p mads-cli

# Runtime stage. Alpine, not scratch: the CLI providers need a temp dir, and
# anything that talks HTTPS needs the CA certificates.
FROM alpine:3.22

RUN apk add --no-cache ca-certificates

COPY --from=build /src/target/release/mads /usr/local/bin/mads

# Mount the project here: docker run -v "$PWD:/work" ghcr.io/avelino/mads generate ...
WORKDIR /work
ENTRYPOINT ["mads"]
CMD ["--help"]
