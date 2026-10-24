# mads studio security

This page explains what the local studio exposes and how to keep the keys and the projects safe.

## Default posture

`mads serve` binds `127.0.0.1:4780`. Only the local machine reaches it. The server prints a one-time token in the URL fragment. The browser exchanges it for a session cookie.

- The cookie is `HttpOnly`, `SameSite=Strict` and scoped to the server path.
- Mutations need the session cookie and the `X-Mads-CSRF` header.
- The server checks the `Origin` and `Host` headers.
- Mutations are limited to 120 per minute per server process.
- Every response carries a strict Content Security Policy, `Referrer-Policy: no-referrer` and `X-Content-Type-Options: nosniff`.
- File reads are sandboxed to the run directory. Paths with `..`, absolute paths and symlinks outside the root are rejected.

A local website cannot start a run or read a project without the session cookie and the CSRF token.

## API keys

Keys saved in the studio go to `~/.config/mads/secrets.toml` with mode `0600`. On Windows the path is `%APPDATA%\mads\secrets.toml`.

```toml
[keys]
anthropic = "sk-ant-..."
```

- The parent directory is `0700`.
- The server refuses a file that other users can read and prints the `chmod` command.
- The browser never receives a key. The Connections page shows only the source.
- An environment variable wins over the file.
- Keys are never written inside a project.

If the file is too open, fix it.

```bash
chmod 700 ~/.config/mads
chmod 600 ~/.config/mads/secrets.toml
```

## Containers

A container usually binds a non-loopback address. That needs a token.

```bash
docker run --rm \
  -p 127.0.0.1:4780:4780 \
  -v "$PWD/mads:/data" \
  -v "$PWD/token:/run/secrets/mads-token:ro" \
  ghcr.io/avelino/mads:latest \
  serve --root /data --bind 0.0.0.0 --allow-remote --token-file /run/secrets/mads-token
```

- Publish the port on host loopback. Then only the host reaches the container.
- The token file must hold at least 32 characters.
- The server does not terminate TLS. If you publish the port on another interface, put a TLS reverse proxy in front of it.
- One token means one operator. There is no multi-user mode.

## What the studio can do

The server runs the same code as the CLI in the same process. It can start missions that spend tokens, cancel them, write project files and re-export a run.

It cannot upload to Google Ads, read a Google Ads account or change Google Ads credentials.

**Importar** reads a path typed by the user, canonicalizes it and copies regular files into `--root`. It rejects symlinks and skips `.git` and `.mads`. The token therefore grants access to every file the server process can read; run the studio as a user that only sees the projects you want to expose.

## Data at rest

Projects are plain files under `--root`. They are not encrypted. Use full disk encryption and your usual backup policy.

The server creates these folders:

- `~/mads` with mode `0700`.
- Each project with mode `0700`.
- `.mads/` with the lock, job logs, uploads and staging files.
- `out/` with the runs and revisions.

The staging directory used by the draft save is removed after validation. Uploaded report CSVs are removed after `optimize` prepares the run.

## Logs and audit

`.mads/audit.ndjson` records project creation, draft saves, job starts and finishes, revisions, optimization prepares and key changes. It never contains a key value.

`.mads/jobs/<job-id>/events.ndjson` records the events of jobs that have no run directory. A run keeps its own `events.ndjson`. Neither contains API keys.

The process writes logs to stderr. Do not run it with `RUST_LOG=debug` while entering a key.

## No telemetry

mads studio sends nothing to a remote service. The only network calls are the provider calls, the URL check, the site read in `init` and the image model.

## Reporting

See the repository security policy for how to report a vulnerability. Do not open a public issue for a security problem.
