# mads studio design

Status: accepted for implementation
Date: 2026-10-24
Supersedes: 2026-10-24-web-ui.md
Scope: V1

This spec describes mads studio, the browser interface served by `mads serve`.

## 1. Decisions

| Decision | Value |
|---|---|
| Audience | People who do not use a terminal. The CLI stays for CI and power users. |
| Distribution | One binary, `mads`. The new command is `mads serve`. |
| Address | `127.0.0.1:4780`, fixed by default. `--port` exists for conflicts. |
| Project root | `~/mads`, one directory per project. `--root` changes it. |
| Internet exposure | Loopback by default. Containers use `--allow-remote --token-file`. No community server mode. |
| Secrets | A `0600` TOML file. No OS keychain. Environment variables win. |
| Plan review | On by default. The plan stops before campaign missions. |
| Models | A curated list plus a free text field. No silent default. |
| Languages | `pt-BR` and `en` from the start. |
| Human revisions | In V1. The original run is immutable. |
| Optimization | In V1. It is not a later phase. |
| Docs | English, published through GitBook. The spec stays out of `SUMMARY.md`. |

## 2. Context

mads turns `business.toml` into Google Ads bulk upload CSVs. Agents plan and
build. Deterministic Rust validates and writes the files. Today the user reads
`report.md` in an editor, tails `events.ndjson` or watches terminal spinners.

The studio makes that state visible and actionable in a browser. It reuses
`mads-core` and `mads-providers`. It does not add network access to
`mads-core` and it does not change the CLI contract.

## 3. Audience and scope

The primary user owns a business and wants campaigns without installing Rust or
editing TOML by hand. The secondary user manages traffic and needs to review,
correct and optimize. The tertiary user runs the CLI in CI.

V1 covers:

- provider setup and model selection without shell variables;
- project creation, init from a URL and manual drafts;
- draft review and validation;
- campaign generation with a plan checkpoint;
- a live mission timeline with token controls and cancellation;
- account review, validation and safe human corrections;
- export, report and import checklist;
- optimization from Google Ads report CSVs.

V1 does not cover:

- upload to Google Ads or the Google Ads API;
- multi-user accounts, teams or billing;
- free-form chat with the agent;
- structural account edits such as adding or removing campaigns and groups;
- campaign types outside the formats the CLI already supports;
- a hosted SaaS.

## 4. Project model

```text
~/mads/
  vinellu/
    project.json
    business.toml
    catalog.csv
    research.md
    brand/logo.png
    DESIGN.md
    .mads/
      transcripts/
      jobs/<job-id>/events.ndjson
      edits.ndjson
      audit.ndjson
      lock
    out/
      20261024-153012-a1b2c3/
        input/
        workspace.json
        run.json
        report.md
        events.ndjson
        transcripts/
        google-ads/
      20261024-153012-a1b2c3-r1/
```

- The project directory is the editable draft. The run directory is a snapshot.
- `out/<run-id>` matches the CLI layout. Running `mads generate
  business.toml` inside the project writes there.
- `project.json` holds display metadata only, such as id, name, creation time
  and archived flag. The business files stay authoritative.
- `~/mads` is created with mode `0700` on first start. Projects are `0700`.
- Discovery scans direct children for `project.json` or `business.toml`.
  Hidden directories and symlinks outside the root are ignored.
- The directory slug is stable. The display name can change.
- Delete moves a project to `~/mads/.trash/<timestamp>` after a typed
  confirmation. Archive only hides it.
- A human revision is a sibling run directory with the `-rN` suffix. Its
  `run.json` carries `origin: "human"`, `parent_run` and `revision`.

## 5. UX and navigation

```text
Projetos
└── Vinellu
    ├── Visão geral
    ├── Rascunho
    │   ├── Negócio
    │   ├── Pesquisa
    │   ├── Catálogo
    │   ├── Marca e design
    │   └── Avançado
    ├── Execuções
    │   └── 20261024-...
    │       ├── Ao vivo
    │       ├── Conta
    │       ├── Validação
    │       ├── Performance
    │       ├── Relatório
    │       ├── Arquivos
    │       ├── Transcrições
    │       └── Revisões
    └── Otimizações
Conexões, Uso, Auditoria, Configurações
```

| Route | Screen |
|---|---|
| `/` | Project list |
| `/projects/new` | Onboarding and new project |
| `/projects/:id` | Project overview |
| `/projects/:id/draft` | Draft review |
| `/projects/:id/runs` | Runs and revisions |
| `/runs/:id` | Run detail, live or finished |
| `/runs/:id/account` | Account review |
| `/runs/:id/validation` | Validation |
| `/runs/:id/performance` | Optimization performance |
| `/runs/:id/report` | Report |
| `/runs/:id/files` | Files |
| `/runs/:id/transcripts` | Transcripts |
| `/runs/:id/optimize` | Optimize wizard |
| `/settings/connections` | Providers, models and keys |
| `/settings/usage` | Token and cost history |
| `/settings/audit` | Audit log |

The interface is server rendered with Askama. HTMX handles small updates and
SSE carries live events. There is no Node build step and no CDN.

### Status model

| UI state | Source | Action |
|---|---|---|
| Rascunho | no run or edited draft | Gerar |
| Na fila | job queued | Cancelar |
| Em execução | job running | Acompanhar, Parar |
| Plano pronto | plan finished, campaign missions pending | Revisar plano, Gerar campanhas |
| Preparado | optimize prepare finished, missions pending | Iniciar |
| Incompleto | run.json `incomplete` | Retomar |
| Inválido | run.json `invalid` | Corrigir, Reexportar |
| Pronto | run.json `success` | Baixar |
| Cancelado | run.json `cancelled` | Retomar |
| Falhou | job failed before a run existed | Ver erro |
| Revisão humana | `origin: human` | Comparar, Exportar |

`planned` and `prepared` are derived from `workspace.json` and the job state.
They are not stored in `run.json`.

## 6. Flows

### 6.1 First access

1. `mads serve --open` prints
   `http://127.0.0.1:4780/#<token>` and opens the browser.
2. The page posts the fragment token to `POST /api/session`.
3. The server sets an `HttpOnly` session cookie and returns a CSRF token.
4. The page removes the fragment with `history.replaceState`.
5. Onboarding lists providers from `list_providers`. API providers take a key.
   CLI providers show executable and version status. The user picks a model
   from the curated list or types an id.

### 6.2 Project and draft

- Create from a URL with `init`, or fill the form manually.
- `init` progress streams tool calls, pages and tokens. When it finishes, the
  draft opens.
- The draft renders forms from the parsed input, plus raw TOML and CSV.
- Counters and validation run on the server with the real parsers.
- `init --force` asks for confirmation and shows what is replaced and what is
  kept (`max_cpc`, `[export]`, `[campaigns]`, `[google_ads]`).
- Save writes to a staging directory, validates with `load_input`, then swaps
  the files atomically.

### 6.3 Generate and plan checkpoint

- The consent screen shows provider, model, token cap, turns, timeout,
  retries, image cap and layout.
- With the checkpoint on, `generate` stops after the plan mission. The plan
  screen shows campaigns, budgets, formats, groups and rationale.
- `Continue` runs the campaign missions. `Skip review` is an advanced setting.
- The run uses the project draft snapshot in `input/`.

### 6.4 Live run

- Mission lanes show pending, running, attempt, finished and failed.
- The inspector shows tokens against the cap, reported cost, elapsed time and
  mission count.
- `Stop` cancels missions, marks the run `cancelled`, keeps images and writes
  no CSV. `Resume` retries unfinished or cancelled missions.
- The timeline replays from `events.ndjson` and survives a reconnect.

### 6.5 Review and correction

- The account tree shows campaigns, ad groups, keywords, negatives, ads,
  assets and images.
- The validation panel groups issues by code and links to the field.
- `Correct` edits text, budgets, CPCs, final URLs, keywords and negatives.
- The server validates every change with `Rules`.
- `Save revision` creates `out/<run-id>-rN`, copies the input snapshot and
  transcripts, reuses image files through hard links when possible and exports
  again without a model call.
- The original run is never changed.
- Each save records edits in `.mads/edits.ndjson` and the global audit log.

### 6.6 Optimize

1. Pick a finished run.
2. Upload report CSVs. The server identifies them by columns.
3. `optimize::prepare` creates the derived run with `live` and returns the
   performance digest.
4. The digest shows the window, campaigns, spend, conversions, lost impression
   share, thin, stalled and untracked flags, idle budget and missing reports.
5. `Start` runs the plan and campaign missions.
6. Review shows performance columns and a Changes tab. Export writes the
   Editor file only.

### 6.7 Export

- File list with row count, size and format aware preview. `editor/account.csv`
  is UTF-16 LE and tab separated. `.xlsx` files are offered as downloads.
- The report renders sanitized Markdown.
- The import checklist explains bulk files, Editor file and the automation
  package. It warns that campaigns arrive paused.

## 7. Security

Threat model:

| Threat | Control |
|---|---|
| A malicious website posts to loopback | `SameSite=Strict` cookie, `Origin` and `Host` checks, CSRF header on mutations |
| Another local user reaches the port | session token, `0700` project and root directories |
| DNS rebinding | host allowlist for `127.0.0.1:4780` and `localhost:4780` |
| Token leakage in URLs and logs | bootstrap token in the fragment, exchanged for an `HttpOnly` cookie, removed from the URL |
| Path traversal and symlinks | canonicalization, root prefix check, extension allowlist |
| XSS from agent text or the report | Askama autoescape, sanitized Markdown, strict CSP, no inline script |
| API key exposure | server only, `0600` file, never returned, redacted logs |
| Upload abuse | size and file count limits, `.csv` only, parsed with `read_table`, temporary directory removed |
| Accidental spend | authentication, rate limit, consent, caps, checkpoint, cancellation |
| Remote exposure | loopback default, explicit `--allow-remote`, token file, TLS at the proxy |

Session rules:

- `mads serve` generates a 256 bit token and prints it in the URL fragment.
- `POST /api/session` validates the token in constant time and creates a
  session with a CSRF token.
- The cookie is `HttpOnly`, `SameSite=Strict`, `Path=/`, and `Secure` when TLS
  is present.
- Mutations require `X-Mads-CSRF` and a JSON content type. Uploads require the
  same header.
- SSE and static assets use the cookie. No token goes in a query string.
- A `Authorization: Bearer <token>` header is accepted for tests and scripts.

Headers:

- `Content-Security-Policy` without `unsafe-inline`.
- `Referrer-Policy: no-referrer`.
- `X-Content-Type-Options: nosniff`.
- `frame-ancestors 'none'`.
- `Cache-Control: no-store` for HTML and API responses.

Secrets:

- `$XDG_CONFIG_HOME/mads/secrets.toml` or `~/.config/mads/secrets.toml`.
- On Windows, `%APPDATA%\mads\secrets.toml`.
- File `0600`, parent `0700`. A loose permission refuses to load and prints the
  `chmod` command.
- Format: a `[keys]` table keyed by provider name.
- Precedence: environment variable, then file. The UI shows the source and
  warns when an environment variable makes a saved key ineffective.
- The file is never placed inside a project.

Remote and containers:

- Default bind is loopback.
- A non-loopback bind requires `--allow-remote` and `--token-file`.
- Containers publish to host loopback, for example
  `-p 127.0.0.1:4780:4780`. Publishing elsewhere requires TLS.
- No multi-user mode in V1.

Audit:

- `.mads/audit.ndjson` records session creation, failed authentication, job
  start, cancellation, draft save, revision, export, key change and settings
  change. It never records a secret.

No telemetry is sent anywhere.

## 8. Architecture

```text
Browser (HTML, HTMX, EventSource)
        |
        v
mads serve (mads-cli/src/serve)
  projects, jobs, SSE, sessions, secrets, views
        |
        v
mads-core (no network)
  generate, export_run, run_init, optimize::prepare, Rules, export_csvs
        |
        v
mads-providers
  drivers, WebClient, SiteClient, preflight
```

- `mads serve` lives in `mads-cli/src/serve/` behind the `serve` feature,
  enabled by default. A slim build uses `--no-default-features`.
- Templates live in `mads-cli/templates/`. Assets live in `mads-cli/assets/`.
  `rust-embed` ships them in release builds and reads from disk in debug.
- The server calls core functions in process. It does not shell out to `mads`.
- The filesystem is the database. There is no index and no cache that can drift
  from disk.

Jobs:

- A job has an id, kind, project, optional run id, state, cancellation token and
  event log.
- Job kinds are `init`, `generate`, `export`, `revision` and `optimize`.
- `init` has no run directory. Its log lives in `.mads/jobs/<job-id>/`.
- `generate`, `export` and `revision` use the run `events.ndjson`.
- `optimize` starts with a synchronous prepare that creates the run, then a
  `generate` job.
- One job per project. The server runs at most two jobs at a time. A third
  waits in a FIFO queue. A second job for the same project returns 409.

SSE:

- `GET /api/jobs/:id/events` subscribes first, replays the log from
  `Last-Event-ID`, deduplicates by sequence, then streams live events.
- Heartbeat every 15 seconds. The stream closes after the job finishes and the
  client has received the last event.
- A restart loses the in-memory job registry. The startup sweep marks
  `Running` missions as interrupted and the UI offers `Resume`.

Locks:

- `flock` on `.mads/lock` per project and on the run directory for export.
- The CLI takes the same locks. A conflict returns a clear message.
- The lock file stores pid, start time and job id for diagnostics.

Cancellation and checkpoint:

- `RunControl` carries a cancellation token into `generate` and `export_run`.
  It is checked between turns, missions and URL checks.
- On cancel, running missions become failed with reason `cancelled`, images
  stay, no CSV is written, and `run.json` records `status: "cancelled"`.
- `RunConfig.stop_after_plan` returns a paused result after the plan. The
  workspace keeps the plan and pending campaign missions.

Atomic writes:

- `run.json`, `report.md` and CSVs are written to a temporary file and renamed.
- `events.ndjson` is appended.

## 9. Revisions

- The original run is immutable.
- A revision is a new run directory `<run-id>-rN` with the same `input/`,
  `transcripts/` and images. Images are hard linked when the filesystem allows.
- `run.json` gains `origin`, `parent_run`, `revision` and `edits`.
- `workspace.json` keeps version 1. The revision edits only the account.
- Edits use the same paths as validation issues, for example
  `campaigns[0].ad_groups[3].rsa.headlines[2]`.
- The server applies edits to a copy, validates with `Rules`, and only saves
  when there are no errors.
- `If-Match` carries a hash of the account. A mismatch returns 409.
- `POST /api/runs/:id/revisions` creates the revision and starts an export job.
- V1 edits text, budgets, CPCs, final URLs, keywords and negatives. It does not
  add or remove campaigns or ad groups.
- `.mads/edits.ndjson` records before and after values. `.mads/audit.ndjson`
  records the revision. The UI shows a diff and allows undo by opening an
  earlier revision.

## 10. Optimization

- The base run must be finished.
- Reports are uploaded to `.mads/uploads/<job-id>/` and parsed with
  `perf::read_table`.
- `optimize::prepare` copies input and images, sets `live` and returns the
  digest. No model call happens here.
- The digest is shown before the user spends tokens.
- `Start` runs the normal generate pipeline with live tools.
- The review exposes performance columns, keyword signals, search terms and a
  Changes tab.
- Export is Editor only. The UI warns against uploading files 1 to 5.
- The base run is not changed. The optimized run records `parent_run`.

## 11. API and events

| Method | Path | Purpose |
|---|---|---|
| POST | `/api/session` | Exchange the bootstrap token for a session |
| DELETE | `/api/session` | End the session |
| GET | `/api/me` | Session state and locale |
| GET | `/api/health` | Version, uptime, active jobs |
| GET | `/api/providers` | Provider and image status |
| GET | `/api/secrets` | Configured providers and source, never values |
| PUT | `/api/secrets/:name` | Save a key to the `0600` file |
| DELETE | `/api/secrets/:name` | Remove a saved key |
| GET | `/api/projects` | Project list |
| POST | `/api/projects` | Create a project |
| GET | `/api/projects/:id` | Project and latest run |
| PATCH | `/api/projects/:id` | Display name or archive |
| GET | `/api/projects/:id/draft` | Structured draft, raw TOML and CSV |
| PUT | `/api/projects/:id/draft` | Validate and save atomically |
| POST | `/api/projects/:id/draft/validate` | Validate without writing |
| POST | `/api/projects/:id/assets` | Upload a logo |
| POST | `/api/projects/:id/init` | Start an init job |
| GET | `/api/projects/:id/runs` | Runs and revisions |
| POST | `/api/projects/:id/runs` | Start a generate job |
| GET | `/api/jobs/:id` | Job state |
| GET | `/api/jobs/:id/events` | SSE with replay |
| POST | `/api/jobs/:id/cancel` | Cancel a job |
| GET | `/api/runs/:id` | Run metadata and account summary |
| GET | `/api/runs/:id/account` | Account tree |
| GET | `/api/runs/:id/validation` | Issues and URL check |
| GET | `/api/runs/:id/report` | Raw and rendered report |
| GET | `/api/runs/:id/files` | File list |
| GET | `/api/runs/:id/files/*` | File download |
| GET | `/api/runs/:id/download` | Zip of the run |
| GET | `/api/runs/:id/transcripts/:mission` | Transcript |
| POST | `/api/runs/:id/continue` | Continue after the plan checkpoint |
| POST | `/api/runs/:id/resume` | Resume unfinished missions |
| POST | `/api/runs/:id/export` | Re-export without a model |
| POST | `/api/runs/:id/account/validate` | Validate edits without saving |
| POST | `/api/runs/:id/revisions` | Create a human revision |
| POST | `/api/runs/:id/optimize/prepare` | Upload reports and prepare |
| POST | `/api/runs/:run/start` | Start a prepared optimize run |
| GET | `/api/runs/:id/diff?against=:other` | Account diff |

Errors use `{"error": {"code", "message", "details"}}`. Validation issues keep
`code`, `severity`, `path` and `message`.

`run.json` gains optional fields:

```json
{
  "origin": "agent",
  "parent_run": null,
  "revision": 0,
  "edits": 0
}
```

`status` accepts `success`, `invalid`, `incomplete` and `cancelled`.

New events:

- `draft_updated` for init, with the current draft summary.
- The revision flow uses existing `run_started`, `step`, `validation`,
  `artifact_written` and `run_finished` events.

## 12. Core and provider changes

| Change | Where | Why |
|---|---|---|
| `RunControl` and cancellation | `core/run`, `agent` | Stop button and clean interruption |
| `stop_after_plan` and paused result | `core/run` | Plan checkpoint |
| `run.json` origin, parent, revision and cancelled status | `core/run` | Revisions and audit |
| Atomic writes for run artifacts | `core/run` | Concurrent reads |
| `draft_updated` event | `core/init`, `events` | Live draft preview |
| Public draft writer for TOML and CSV | `core/input` | Studio forms |
| Key override for API and image providers | `mads-providers` | `0600` key file |
| Snapshot helper for input and logo | `core/run` or CLI | Reuse across commands |
| Human revision note in the report | `core/report` | Transparency |
| Model registry data | `mads-cli/serve` | Curated list without a default |
| Project and run locks | `mads-cli` | CLI and server concurrency |

`Rules`, `merge_rsa`, `export_csvs`, `optimize::prepare`, `perf::digest` and
`list_providers` stay public and are reused. The UI does not duplicate rules in
JavaScript.

## 13. Internationalization and accessibility

- Catalogs: `locales/pt-BR.toml` and `locales/en.toml`.
- Default `pt-BR`. `Accept-Language` selects `en` when appropriate. A session
  setting overrides both.
- No user facing string is hardcoded in templates or JavaScript.
- Core errors arrive in English. The UI maps known codes to localized messages
  and keeps the original text under Details.
- Dates, numbers and currency follow the UI locale. CSV output keeps the
  `export.decimal_comma` format.
- A test compares the key sets of both catalogs.
- Accessibility: WCAG AA, keyboard paths for every action, visible focus,
  status not conveyed by color alone, live regions only for milestones,
  reduced motion and labeled tables.

## 14. Testing and CI

- Unit tests: path sandbox, session and CSRF, host and origin, secret
  redaction, locks, job states, draft round trip, account patch, revision.
- Integration tests: full flows with `ScriptedDriver`, a fake `SiteFetch` and a
  fake `Web`. Init, generate, export, optimize, revision and plan checkpoint
  run without network and without tokens.
- SSE tests: replay, reconnect, subscribe before replay, cancel and shutdown.
- Concurrency tests: two jobs on one project, CLI and server, export during
  generate, interrupted `Running` missions.
- Security tests: no key in a response or log, hostile Markdown, invalid
  upload, path traversal, UTF-16 CSV preview.
- Golden test: `data/` stays byte for byte identical.
- Template snapshots with `insta`.
- The gate stays `cargo fmt --check`, `cargo clippy --workspace --all-targets
  -- -D warnings` and `cargo test --workspace`.
- One CI workflow. No second workflow and no Node in the gate.
- Docker keeps the single `mads` binary and documents `serve`.

## 15. Phases and acceptance

| Phase | Scope | Acceptance |
|---|---|---|
| F0 Foundation | `mads serve`, projects, jobs, SSE, locks, cancellation, session and CSRF, key file, atomic writes, draft writer, read-only dashboard and run detail | security tests pass, SSE replay works, one binary, golden untouched |
| F1 Studio | onboarding, connections, project draft, init, generate, plan checkpoint, live and cancel, account review, validation, export, report, files | a non technical user finishes init to export without a terminal |
| F2 Correction and optimization | human revisions, edit validation, re-export without a model, diff and undo, optimize prepare, digest, run and changes | an `E01` is fixed and re-exported without a new run, optimize works end to end |
| F3 Hardening | accessibility, i18n coverage, performance, documentation, Docker and remote docs, backup | accessibility audit, docs complete, beta |

Docs are delivered with each phase. The design spec stays out of GitBook.
User pages are added to `docs/SUMMARY.md` only when the behavior exists.

Documentation map:

- New: `docs/guides/mads-studio.md`, `docs/guides/mads-studio-security.md`.
- Update: `docs/SUMMARY.md`, `docs/README.md`, `docs/getting-started.md`,
  `README.md`, `SECURITY.md`, `CONTRIBUTING.md`, `CLAUDE.md`,
  `docs/reference/cli.md`, `docs/reference/environment-variables.md`,
  `docs/reference/output-files.md`, `docs/reference/architecture.md`,
  `docs/reference/validation-rules.md`, `docs/guides/docker.md`,
  `docs/guides/resume-and-export.md`, `docs/guides/optimize.md`,
  `docs/guides/cost-and-limits.md`, `docs/guides/review-and-import.md`,
  `docs/guides/init-from-url.md`, `docs/guides/providers.md`,
  `docs/guides/agent-clis.md`, `docs/guides/troubleshooting.md`.
- Remove: `docs/superpowers/specs/2026-10-24-web-ui.md`.

## 16. Risks and open questions

| Risk | Mitigation |
|---|---|
| Scope of a studio with revisions and optimization | phases, feature flag, no structural edits in V1 |
| Fixed port conflicts | clear error and `--port`, no silent fallback |
| Security regression | tests from F0, strict CSP, no remote by default |
| Draft writer drift | round trip tests against `load_input` |
| Revision disk use | hard links, retention setting, warning |
| CLI and server concurrency | shared locks |
| Model registry staleness | a free text id is always available |
| Agent CLI login | show executable and version, keep login external |
| Token spend | consent, caps, checkpoint, cancellation, no silent retries |
| Browser SSE drops | replay by sequence and persisted logs |
| Windows paths and permissions | explicit tests |

Open questions for later versions:

- Structural edits in the studio.
- A hosted, multi-user mode.
- A desktop wrapper that starts `mads serve`.
- Google Ads API integration.
- A second ad platform.

## 17. Implementation notes (2026-10-25)

Written when the branch landed, so the spec and the code tell the same story.

- The session token is 64 hex characters (two UUIDv4s, 244 bits), not 256. Still CSPRNG-backed and compared in constant time.
- The startup sweep marks the `Running` missions of runs from a previous server life as `Failed { reason: "interrupted" }`; the run page offers resume for that status. `run.json` itself does not record the sweep.
- Routes that differ from the section 11 table:
  - `PATCH /api/projects/:id` became `POST /api/projects/:id/archive` and `POST /api/projects/:id/rename` (rename was missing in V1 code and is now in).
  - `POST /api/projects/:id/assets` became `POST /api/projects/:id/logo`, plus a `GET` for the image.
  - `GET /api/projects/:id`, `GET /api/me`, `GET /api/providers`, `POST /api/projects/:id/draft/validate`, `GET /api/runs/:id/download` and `GET /api/runs/:id/transcripts/:mission` are not separate endpoints: the HTML pages and `GET /api/runs/:id/files/*path` cover their roles. Transcript validation lives in the file route.
  - `POST /api/runs/:run/start` is folded into `POST /api/runs/:id/resume`, which starts a prepared optimize run.
  - `GET /api/runs/:id/diff` exists only as the page `/runs/{id}/diff`, always against `parent_run`.
- `If-Match` on `POST /api/runs/:id/revisions` is enforced whenever the header is present; the browser always sends it. `GET /api/runs/:id/account` returns the hash as `account_hash`.
- Audit events implemented: `job_started`, `job_finished`, `draft_saved`, `research_saved`, `design_saved`, `revision_created`, `revision_reverted`, `optimize_prepared`, `key_saved`, `key_removed`, `project_created`, `project_imported`, `project_archived`, `project_renamed`, `project_deleted`, `logo_saved`. Session creation, failed authentication and cancellation are still not recorded (follow-up).
- The draft save writes the catalog first and commits with the TOML rename; a crash can leave the old TOML with the new catalog, never the reverse.
- Revision images are hard linked with a copy fallback across devices.
- 2026-10-25, user decisions: English is the default interface language (the section 13 "default pt-BR" no longer holds; a pt-BR cookie or `Accept-Language` still switches); the "English is in beta" notice is gone. The published Docker image ships `claude`, `codex` and `gemini`, so the cli providers work in containers. `GET /api/providers/:name/models` asks the provider's API for the current model ids; the project and run forms list only configured providers, offer those ids in a datalist, prefill the `openai-compat` endpoint from `MADS_BASE_URL`, and still accept a free text id (section 1, "Models").
