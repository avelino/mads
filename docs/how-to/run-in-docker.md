# Run in Docker

Run `mads init`, `mads generate` and the other commands from the container image, with no Rust on your machine.

The image runs `mads` as its entrypoint. Everything after the image name is a mads command. `init` and `generate` call a model and spend your tokens. `--version` and `providers` call none.

## Before you start

- Docker is installed and can pull from `ghcr.io`.
- You know which provider you will use. See [Choose a provider](choose-a-provider.md). The image has no agent CLI, see step 6 if you want one.
- A project folder with a `business.toml`, or a site for `mads init`. See [Write the business file](write-the-business-file.md) and [Draft from your site](draft-from-your-site.md).

## Steps

1. Pick a tag.

   | Tag | What it is |
   |---|---|
   | `edge` | The last build of `main` that changed `crates/`, `data/`, the Cargo files, `rust-toolchain.toml`, `clippy.toml`, the `Dockerfile` or `.dockerignore`. A push to `main` that touches none of these builds no image. |
   | `sha-<short>` | One build, tagged with its short commit hash. |
   | `latest`, `<major>.<minor>.<patch>`, `<major>.<minor>` | Published only when a `v*` tag is pushed. |

   The repository has no `v*` tag yet, so only `edge` and `sha-*` exist. The examples below use `edge`. When a release exists, swap in its version to repeat a run exactly.

   Every tag is built for `linux/amd64` and `linux/arm64`. Docker picks the one that matches your machine.

2. Check that the image runs.

   ```bash
   docker run --rm ghcr.io/avelino/mads:edge --version
   ```

3. Run a command on your project.

   Mount the project at `/work`. That is the working directory in the container, so `business.toml` and `out/` live in your folder. Pass the key by name. This step calls a model.

   ```bash
   docker run --rm \
     --user "$(id -u):$(id -g)" \
     -v "$PWD:/work" \
     -e ANTHROPIC_API_KEY \
     ghcr.io/avelino/mads:edge \
     generate business.toml --provider anthropic --model <model-id> --out out
   ```

   - `-v "$PWD:/work"` is the only way mads sees your files. The logo path in `business.toml` must be inside this folder.
   - `--user "$(id -u):$(id -g)"` makes the files in `out/` yours. Without it they belong to root.
   - `-e ANTHROPIC_API_KEY` with no value copies the variable from your shell. The key stays out of the command line and your shell history. Each provider reads its own variable, see [Environment variables](../reference/environment-variables.md).
   - For image campaigns, forward the image key too, `-e OPENAI_API_KEY` or `-e GEMINI_API_KEY`. A key in your shell turns on paid pictures with the default `--image-provider auto`. See [Keep token cost down](keep-cost-down.md).
   - `<model-id>` is a model name from your provider. mads has no default.

4. Run `mads init` the same way.

   This step calls a model.

   ```bash
   docker run --rm --user "$(id -u):$(id -g)" -v "$PWD:/work" -e ANTHROPIC_API_KEY \
     ghcr.io/avelino/mads:edge \
     init --from-url https://example.com --daily-budget 50 --currency BRL \
     --provider anthropic --model <model-id>
   ```

   `init` writes into `--out-dir`, which is `.` by default, so the files land in your project folder. See [`mads init`](../reference/cli.md#mads-init).

5. Pass the GitHub Actions variables when you write your own `docker run` for a workflow.

   `GITHUB_ACTIONS` turns on the `github` output format. `GITHUB_STEP_SUMMARY` makes mads append `report.md` to the job summary. The summary file must be mounted at the same path.

   ```bash
   docker run --rm \
     -v "$PWD:/work" \
     -v "$GITHUB_STEP_SUMMARY:$GITHUB_STEP_SUMMARY" \
     -e GITHUB_ACTIONS -e GITHUB_STEP_SUMMARY \
     ghcr.io/avelino/mads:edge providers
   ```

   The full workflow is in [Run in GitHub Actions](run-in-github-actions.md).

6. Add an agent CLI only if you use `claude-cli`, `codex-cli` or `gemini-cli`.

   The image is Alpine with the static `mads` binary and the CA certificates added, so `sh` is there for debugging: `docker run --rm -it --entrypoint sh ghcr.io/avelino/mads:edge`. It holds no keys, input or output: everything comes from the mount and the environment. `mads providers` inside it shows the three CLIs as not found. Build a derived image that installs the CLI.

   ```dockerfile
   FROM ghcr.io/avelino/mads:edge
   RUN apk add --no-cache nodejs npm \
    && npm install -g @anthropic-ai/claude-code
   ```

   ```bash
   docker build -t mads-claude .
   docker run --rm --user "$(id -u):$(id -g)" -v "$PWD:/work" \
     -e CLAUDE_CODE_OAUTH_TOKEN \
     mads-claude generate business.toml --provider claude-cli --out out
   ```

   mads needs `claude` 2.1.259 or newer and stops with exit `2` before any mission otherwise. The container has no browser login, so give `claude` a token or set `ANTHROPIC_API_KEY`. mads does not read `CLAUDE_CODE_OAUTH_TOKEN`, the CLI does. No test in this repository runs this derived image. See [Agent CLIs](../explanation/agent-clis.md).

7. Build the image yourself, only if you need your own code in it.

   Run this from a clone.

   ```bash
   docker build -t mads .
   docker run --rm mads --version
   ```

   The build uses Rust 1.98 by default. Pass `--build-arg RUST_VERSION=<version>` to use another toolchain.

## Check it worked

Run the provider list in the container.

```bash
docker run --rm -e ANTHROPIC_API_KEY ghcr.io/avelino/mads:edge providers
```

It prints the provider table and the image provider table. The provider you set a key for shows `ready`. A derived image shows its agent CLI as `ready`. After a real run, `out/` in your folder holds the run directory and its files belong to you. See [Output files](../reference/output-files.md).

If a command fails, see [Troubleshoot](troubleshoot.md).

## Next

- [Run in GitHub Actions](run-in-github-actions.md)
- [Choose a provider](choose-a-provider.md)
- [Review a run](review-a-run.md)
