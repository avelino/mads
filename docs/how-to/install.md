# Install mads

Install the `mads` binary from source or run it from the container image.

Nothing on this page calls a model. You pick a provider in a later step.

## Before you start

You need one of these.

- Docker, to use the published image. You do not need Rust.
- `git` and [`rustup`](https://rustup.rs), to build from source.

You also need a provider to run mads for real, but not to install it. See [Choose a provider](choose-a-provider.md).

## Install

Pick one path.

### Build from source

1. Clone the repository.

   ```bash
   git clone https://github.com/avelino/mads
   cd mads
   ```

2. Install the binary.

   ```bash
   cargo install --path crates/mads-cli
   ```

   The repository pins its toolchain in `rust-toolchain.toml` (Rust 1.98 with `rustfmt` and `clippy`). The first build inside the clone makes `rustup` install it.

3. Make sure `~/.cargo/bin` is in your `PATH`. `cargo install` puts `mads` there.

To skip the clone, install straight from GitHub. This build does not use the pinned toolchain, so you need Rust 1.88 or newer.

```bash
cargo install --git https://github.com/avelino/mads mads-cli --locked
```

### Use the container image

1. Pull and run the image.

   ```bash
   docker run --rm ghcr.io/avelino/mads:edge --version
   ```

   The image runs `mads` as its entrypoint, so everything after the image name is a mads command. It is built for `linux/amd64` and `linux/arm64`. `edge` is the current `main`. The `latest` tag appears only after the first release tag.

2. To build the image yourself, run this from a clone.

   ```bash
   docker build -t mads .
   docker run --rm mads --version
   ```

   The build uses Rust 1.98 by default. Pass `--build-arg RUST_VERSION=<version>` to use another one.

To run a real command in the container, mount your project and pass keys by name. See [Run in Docker](run-in-docker.md).

## Check it worked

```bash
mads --version
```

```text
mads 0.1.0
```

With the image, use the `docker run` line from step 1 of the container path. It prints the same line.

To see which providers are ready on your machine, run `mads providers`. It prints a provider table and an image provider table, and calls no model. The status column depends on your keys and on which agent CLIs are in `PATH`. See [Providers](../reference/providers.md).

## Next

- [Choose a provider](choose-a-provider.md)
- [Your first run](../tutorials/first-run.md)
