# Docker

This page shows you how to run mads from the published container image, locally or in CI, without installing Rust.

## Run it

```bash
docker run --rm ghcr.io/avelino/mads:latest --version
```

The image runs `mads` as its entrypoint, so everything after the image name is a mads command. Mount your project at `/work` and pass keys by name.

```bash
docker run --rm \
  --user "$(id -u):$(id -g)" \
  -v "$PWD:/work" \
  -e ANTHROPIC_API_KEY \
  ghcr.io/avelino/mads:latest \
  generate business.toml --provider anthropic --model <model-id> --out out
```

- `-v "$PWD:/work"` is how mads sees `business.toml` and where `out/` lands. The working directory in the container is `/work`.
- `--user "$(id -u):$(id -g)"` makes the files in `out/` yours. Without it they belong to root.
- For image campaigns forward the image key too, `-e OPENAI_API_KEY` or `-e GEMINI_API_KEY`. The logo path in `business.toml` must be inside the mounted folder.
- `-e ANTHROPIC_API_KEY` with no value forwards the variable from your shell. The key never appears in the command line or in your shell history. mads reads the standard variable of each provider, see [Environment variables](../reference/environment-variables.md).
- `<model-id>` is a model name from your provider. mads has no default.

`mads init` works the same way.

```bash
docker run --rm --user "$(id -u):$(id -g)" -v "$PWD:/work" -e ANTHROPIC_API_KEY \
  ghcr.io/avelino/mads:latest \
  init --from-url https://example.com --daily-budget 50 --currency BRL \
  --provider anthropic --model <model-id>
```

## Tags

| Tag | What it is |
|---|---|
| `latest` | The newest release tag. |
| `1.2.3` | An exact release. Use it for runs you need to reproduce. |
| `1.2` | The newest patch of a minor version. |
| `edge` | The last build of `main`. It can break. |
| `sha-abc1234` | One commit of `main`. |

Every tag is a multi-architecture image for `linux/amd64` and `linux/arm64`. Docker picks the right one for your machine. On an Apple Silicon Mac you get a native `arm64` image.

For the strictest reproducibility, pin the digest.

```bash
docker pull ghcr.io/avelino/mads:1.2.3
docker inspect --format '{{index .RepoDigests 0}}' ghcr.io/avelino/mads:1.2.3
```

Use the printed `ghcr.io/avelino/mads@sha256:...` reference in your workflow.

## GitHub Actions

[GitHub Actions](github-actions.md) has a complete workflow. It installs a small `mads` wrapper that runs the image and forwards `GITHUB_ACTIONS` and `GITHUB_STEP_SUMMARY`. Those two variables are what turn on the `github` output format and the job summary. If you write your own `docker run`, pass them too.

```bash
docker run --rm \
  -v "$PWD:/work" \
  -v "$GITHUB_STEP_SUMMARY:$GITHUB_STEP_SUMMARY" \
  -e GITHUB_ACTIONS -e GITHUB_STEP_SUMMARY \
  ghcr.io/avelino/mads:latest providers
```

## What is in the image

- A static `mads` binary built for musl.
- The CA certificates, so HTTPS to your provider and to the site `init` reads works.
- Nothing else. It is Alpine, so `sh` is there for debugging: `docker run --rm -it --entrypoint sh ghcr.io/avelino/mads:latest`.

The image does not hold your keys, your input or your output. Everything comes from the mount and the environment.

## Agent CLI providers

The claude, codex and gemini CLIs are not in the image. `mads providers` inside it shows them as not found. To use one, derive an image.

```dockerfile
FROM ghcr.io/avelino/mads:latest
RUN apk add --no-cache nodejs npm \
 && npm install -g @anthropic-ai/claude-code
```

```bash
docker build -t mads-claude .
docker run --rm --user "$(id -u):$(id -g)" -v "$PWD:/work" \
  -e CLAUDE_CODE_OAUTH_TOKEN \
  mads-claude generate business.toml --provider claude-cli --out out
```

See [Agent CLIs](agent-clis.md) for the login and the version mads needs.

## Build it yourself

```bash
docker build -t mads .
docker run --rm mads --version
```

The Dockerfile builds the workspace in a Rust Alpine image and copies the binary into a small Alpine image. Dependencies are a separate layer, so a change in `crates/` does not rebuild them. Pass `--build-arg RUST_VERSION=1.98` to change the toolchain.

## How the image is published

Every push to `main` and every tag `v*` runs the same workflow that runs the tests. The image is built only after fmt, clippy and the tests pass. See [Contributing](https://github.com/avelino/mads/blob/main/CONTRIBUTING.md#ci-and-releases).
