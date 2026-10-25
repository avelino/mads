# mads studio

This page runs mads in the browser, from the first project to the exported CSVs.

## What it is

`mads serve` starts a local web interface. It reads and writes the same project directories as the CLI. It does not upload anything to Google Ads.

The interface has Portuguese and English. English is in beta. Use the language selector in the top bar.

## Start it

```bash
mads serve --root ~/mads --port 4780 --open
```

The server prints an address with a token.

```text
mads studio: http://127.0.0.1:4780/#<token>
```

The browser exchanges the token for a session cookie and removes it from the address. If the page does not open, paste the token in the form.

- `--root` is the directory that holds one subdirectory per project. The default is `~/mads`.
- `--port` defaults to `4780`. If the port is busy the server stops and asks for another one.
- `--bind` defaults to `127.0.0.1`.
- `--allow-remote` and `--token-file` are required for a non-loopback bind. Use them in a container.
- `--open` opens the browser.

Stop the server with Ctrl-C.

## Create a project

Open **Novo projeto**. The form asks for the business name, the site, the daily budget, the currency, the provider and the model.

**Estudar site** runs `mads init`. The agent reads the site, may search the web and writes `business.toml`, `catalog.csv` and `research.md`. The job page shows the tool calls and the token use.

When init finishes, open **Rascunho**.

If init fails, the project stays in the list with the error. Open the project and choose **Estudar o site de novo**. The last init settings are prefilled. The retry form has a **Substituir arquivos existentes** option for a project that already has files.

You can also import an existing folder from the server with **Importar**, or create a valid draft without the agent with **Criar manualmente**. A directory with `business.toml` under `--root` appears on the dashboard.

Open **Configurações** in a project to archive it, reactivate it or move it to `<root>/.trash/`. Archive hides the project from the dashboard; the files stay in place.

## Review the draft

The draft page edits the files the next run will use.

- business fields, budget, export settings and the catalog CSV.
- a table with the parsed catalog, below the CSV editor.
- optional focus, app and `[google_ads]` fields.
- restricted categories and required campaign formats.
- a preview of `business.toml` and a table with the parsed catalog.
- editors for `research.md` and `DESIGN.md`.
- logo upload; the server validates the image and writes `brand/logo.png`.

Save validates the draft with the same parser as `mads generate`. An invalid draft is not written.

A saved draft does not change earlier runs. Each run keeps its own copy in `input/`.

## Generate

Open a project and choose **Nova execução**.

- Pick the provider and model. API providers need a key, see [Connections](#connections).
- Choose the file set. `bulk` writes files 1 to 5 and the Editor file. `drive-folders` writes the automation package.
- Set the token cap, the turns per mission, the timeout and the retries.
- Keep **Revisar o plano** on. The run stops after the plan and shows campaigns, budgets and groups before campaign missions spend tokens.
- Read the cost notice and confirm.

The job page streams the mission events. **Parar** cancels the run. The workspace and the pictures stay. **Retomar** runs the unfinished missions. The top bar **Jobs** page lists the jobs this server process started; history stays in each project folder.

## Review the account

The run page links to the account, the validation, the report, the files and the events.

- The account shows campaigns, ad groups, keywords, negatives, sitelinks, callouts, snippets, the merged ad and the asset groups. The ad preview uses `merge_rsa`, the same code as the export.
- Sitelinks, callouts and snippets are shown and validated. The export still omits them; see [Limitations](#limitations).
- The validation groups issues by code and links to the field path.
- The files page previews text files, decodes the UTF-16 Editor CSV and shows pictures.

## Correct without calling the model

Open **Corrigir** on a finished run.

- Edit texts, budgets, CPCs, final URLs, keywords, negatives, asset groups and campaign assets.
- Save creates a human revision in `out/<run-id>-rN`. The original run is not changed.
- The server validates the account with `Rules` before writing. An invalid account is not saved.
- The revision is exported again without a model call.
- `run.json` records `origin: "human"`, `parent_run`, `revision` and the number of edits.

The correction page edits existing rows. Adding or removing campaigns and ad groups is still structural, so generate another run for that.

Open **Alterações** on a revision to compare its account with the parent. The diff links back to both runs. **Desfazer revisão** creates a new revision from the parent account and exports it again, without touching the original or the revision you are undoing.

## Optimize

Open **Otimizar** on a finished run, or **Otimizar** on the run page.

1. Upload the CSV reports exported from Google Ads. Up to 20 files and 20 MB.
2. The server reads the reports and creates a derived run with the performance digest.
3. Read the digest. It shows spend, clicks, conversions, lost impression share, thin, stalled and untracked campaigns, idle budget and missing reports. After the run, **Performance** shows the same digest as a table.
4. Choose the provider and model and start the missions.

The base run is not changed. The optimized run records its parent. **Alterações** compares the optimized account with the base. Import the result only with Google Ads Editor. Do not upload files 1 to 5 over a live account.

## Connections

Open **Conexões** to see the providers and save API keys.

Keys are stored in `~/.config/mads/secrets.toml` with mode `0600`. On Windows the path is `%APPDATA%\mads\secrets.toml`. The browser never receives a key. Environment variables win over the file. A saved key is used by text and image jobs, including `init` and `generate`; there is no need to restart the server or export the variable.

Agent CLIs use their own login. The studio only checks that the executable exists and its version.

## Export

The files page lists every artifact. Download the files you need. The report explains the import order. Campaigns arrive paused unless `export.status` says otherwise.

The studio does not upload. Use Google Ads, Bulk actions, Uploads, or Google Ads Editor.

## JSON API

The browser uses the same API. It is available with the session cookie and the `X-Mads-CSRF` header for mutations. A bearer token works for scripts.

```text
GET    /api/health
POST   /api/session
DELETE /api/session
GET    /api/projects
POST   /api/projects
GET    /api/projects/:id/draft
PUT    /api/projects/:id/draft
POST   /api/projects/:id/init
POST   /api/projects/:id/runs
GET    /api/projects/:id/logo
POST   /api/projects/:id/logo
PUT    /api/projects/:id/research
PUT    /api/projects/:id/design
GET    /api/secrets
PUT    /api/secrets/:name
DELETE /api/secrets/:name
POST   /api/projects/:id/archive
POST   /api/projects/:id/rename
POST   /api/projects/:id/delete
POST   /api/projects/import
POST   /api/projects/manual
GET    /api/jobs/:id
POST   /api/jobs/:id/cancel
GET    /api/jobs/:id/events
GET    /api/runs/:id
GET    /api/runs/:id/account
GET    /api/runs/:id/validation
GET    /api/runs/:id/report
GET    /api/runs/:id/files
POST   /api/runs/:id/account/validate
POST   /api/runs/:id/resume
POST   /api/runs/:id/continue
POST   /api/runs/:id/export
POST   /api/runs/:id/revisions
POST   /api/runs/:id/revert
POST   /api/runs/:id/optimize/prepare
GET    /api/runs/:id/events
GET    /api/runs/:id/stream
GET    /api/runs/:id/files/*path
```

The HTML pages are not an API contract. The JSON fields follow the workspace types and can change before 1.0.

Two more rules apply to mutations:

- A second job for a project that already has one queued or running answers `409`, not `400`.
- `POST /api/runs/:id/revisions` accepts an `If-Match` header with the account hash from `GET /api/runs/:id/account`. A stale hash answers `409` with the current hash, so two tabs cannot compose edits over each other.
- Deleting a project requires `{"confirm": "<project name>"}`; the typed confirmation is checked on the server, not only in the browser.

The server runs at most two jobs at once. A third waits in a FIFO queue and the job page shows it as `Na fila`. The queue, the run list and the event stream survive a page reload through `Last-Event-ID`.

## CLI parity

Every studio action maps to a CLI command.

| Studio | CLI |
|---|---|
| Create project and study the site | `mads init --from-url ...` |
| Generate | `mads generate business.toml` |
| Resume | `mads generate --resume out/<run-id>` |
| Re-export | `mads export out/<run-id>` |
| Optimize | `mads optimize out/<run-id> --reports ...` |
| Correct | edit the account in a new revision, then `mads export` |

The CLI keeps working. CI should keep using it.

## Language

The language selector lives in the top bar. Portuguese is the default. English is in beta: the main screens, forms and messages are translated, but some labels from generated content stay in the language of the run.

The choice is a cookie and a session field; it survives a reload and a server restart.

## Limitations

- No upload and no Google Ads API.
- One active job per project. A second request is refused.
- No multi-user server.
- No structural account edits in the browser, such as adding or removing campaigns or ad groups.
- Sitelinks, callouts and snippets are validated but not exported. They appear in the account and in the diff.
- English is in beta.
- The interface is local. Bind it to another address only with a token.
