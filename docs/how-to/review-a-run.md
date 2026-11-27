# Review a run

Check a finished run against your account and your claims, so nothing wrong reaches Google Ads Editor.

mads enforces Google's limits. It does not know your account, your prices or what you can promise. The steps below cover what only you can check.

## Before you start

- A finished run directory, for example `out/<run-id>/`, from [`mads generate`](../reference/cli.md#mads-generate) or `mads optimize`. See [Your first run](../tutorials/first-run.md) if you have none.
- Your `business.toml`. The run keeps a copy in `input/` for your records.
- A Google Ads account you can open, to compare the conversion setup. See [Prepare your Google Ads account](prepare-google-ads.md).

The samples below come from an example run (replay provider, no model was called). It ran with `--skip-url-check`, so its URL check says `skipped`.

## 1. Read the status

Open `out/<run-id>/report.md`. The status line is the first quote after the meta line (the run id, date, provider and model).

```markdown
> Ready to import. Review the files in `google-ads/` before uploading.
```

| Status line starts with | Exit code | What to do |
|---|---|---|
| `Ready to import.` | 0 | Go on with step 2. |
| `Not exported:` | 3 | No CSV was written. Fix the errors in the Validation section, then run `mads export <run-dir>` or generate again. |
| `Incomplete:` | 1 | A mission failed. See [Resume or re-export a run](resume-a-run.md). |

The full list of sections is in [report.md](../reference/report-md.md).

## 2. Run the pre-flight list

Six keys decide whether the files are safe to import. Check each one, in `business.toml` and in the run.

| Key | What to check | Where to see it in the run |
|---|---|---|
| `business.conversion_tracking` | It is `true` only when the account records conversions. The default is `false`, and `mads init` keeps it `false` unless the site shows tracking. | With `false`, `maximize_conversions` on Performance Max and Demand Gen fails with `E10`. With `false` and no Search conversion in the reports, the After launch section warns that Google Ads shows clicks and cost only. |
| `budget.max_cpc` | It is set, in the currency of `budget.currency`. Without it no cap exists for `E11` to check. | Budget and bids. Every CPC sits at or under it. |
| `export.status` | It is `Paused`, which is the default. | The `Campaign status` column of `bulk/1-campaign.csv` and the `Campaign Status` column of `editor/account.csv`. |
| `export.url_suffix` | The UTM parameters are the ones your analytics expects. | The `Final URL suffix` column. |
| `business.competitors` and `business.avoid` | Competitors are names you are ready to see as third-party terms. `avoid` holds every phrase your ads must not say. | `W01` warnings, the `4-negative-keywords.csv` file, and the absence of `avoid` terms in ad text (`E05`). |
| `business.description` | Every claim in it is true today. | The ad text. |

Each row in detail.

**conversion_tracking.** Open your Google Ads account and compare. Set the key to `true` only when a conversion action records. If the key is wrong, fix `business.toml` and generate again, because the plan depends on it. See [Prepare your Google Ads account](prepare-google-ads.md).

**max_cpc.** Compare it with what one click is worth to you. The agents set each ad group CPC under it.

```markdown
- rotulo-alamos: CPC 1,50 (Label searches are cheap and specific. 1,50 starts under the 3,00 cap.)
```

**export.status.** Keep `Paused` for the first import. The files then create every new campaign paused, and nothing serves until you enable it.

```bash
cut -d, -f3,4 out/<run-id>/google-ads/bulk/1-campaign.csv
```

```csv
Campaign status,Campaign
Paused,Vinellu - Marca
Paused,Vinellu - Rotulos
Paused,Vinellu - App de vinhos
```

On a run from `mads optimize`, a live campaign that the plan keeps comes out `Enabled` whatever `export.status` says. Read the Changes section of `report.md` before you import. See [Optimize a live account](optimize-a-live-account.md).

**url_suffix.** `{mads_campaign}` becomes the campaign slug. Any other `{...}` is a Google ValueTrack parameter and stays as written.

```csv
Campaign,Final URL suffix
Vinellu - Marca,utm_source=google&utm_medium=cpc&utm_campaign=vinellu-marca&utm_content={adgroupid}&utm_term={keyword}
```

**competitors and avoid.** mads adds each competitor as a phrase negative outside the competitor campaign, and each brand term outside the brand campaign.

```csv
Negative keyword,Add,Enabled,Ad group,Vinellu - Marca,marca,vivino,Phrase match
Negative keyword,Add,Enabled,Ad group,Vinellu - Rotulos,rotulo-alamos,vinellu,Phrase match
Negative keyword,Add,Enabled,Ad group,Vinellu - Rotulos,rotulo-alamos,vivino,Phrase match
```

`avoid` blocks a phrase in headlines, descriptions and asset text, not in keywords. If a phrase must also stay out of keywords, read `3-keywords.csv` yourself.

**description.** The agents write from it. mads blocks `!` in headlines (`E04`) and `avoid` terms (`E05`), and nothing else about claims. Remove any price, guarantee or superlative that you cannot back, then generate again.

For every key and its rule, see [business.toml](../reference/business-toml.md).

### What to expect

Each key set the way your account needs it. If you changed `business.toml`, run `mads generate` again. A new run calls the model and spends your tokens.

## 3. Read the Summary and the budget

The Summary has one row per campaign.

```markdown
| Campaign | Format | Intent | Daily budget | Bidding | Groups | Keywords |
|---|---|---|---|---|---|---|
| Vinellu - Marca | Search | brand | 8,00 BRL | Manual CPC | 1 | 9 |
| Vinellu - Rotulos | Search | catalog | 30,00 BRL | Manual CPC | 6 | 71 |
| Vinellu - App de vinhos | Search | generic | 12,00 BRL | Manual CPC | 3 | 24 |
```

Check these.

- The budgets add up to `budget.daily`. Here 8 plus 30 plus 12 is 50.
- The split fits what you want to learn first.
- The report prints money with a comma when `export.decimal_comma` is on. The bulk CSV prints `8`.

The Budget and bids section says that shares and CPCs are estimates, because mads has no auction data. Compare each CPC with the Keyword Planner in your account.

## 4. Read the warnings

Errors stop the export, so a run with `Ready to import` has none. Warnings never stop it.

```markdown
**W01 (33)**

...
- `campaigns[1].ad_groups[0].rsa.headlines[0]`: third-party term 'alamos malbec' in ad text (trademark policy risk)
...

**W04 (5)**

- `campaigns[0].ad_groups[0].rsa`: merged ad has 14 headlines and 4 descriptions (15 and 4 recommended)
...

**W05 (2)**

- `campaigns[0].assets`: recommended: 4+ sitelinks, 4+ callouts and 1 structured snippet
...
```

| Code | What to do |
|---|---|
| `W01` | A competitor or a catalog item with `third_party = true` is named in ad text. Read each one and decide whether you keep it. |
| `W04` | The ad works. It has fewer than 15 headlines or 4 descriptions, so it has less room to vary. Only runs made before `E34` raise it. |
| `W15` | Two texts repeat the same words. The ad works, but one slot shows the same idea twice. Rewrite one in the next run. |
| `W05` | Sitelinks, callouts and snippets are validated but not exported. Add them in Google Ads after the import. |

The path of each example starts with the campaign index in plan order. Every code is in [Validation rules](../reference/validation-rules.md).

## 5. Check the URL check

The URL check lists every final and sitelink URL with its HTTP status. A URL that does not answer 2xx fails the run with `E15`.

With `--skip-url-check` the section says `skipped`. Open each final URL in a browser yourself, because mads did not.

## 6. Read the ads and the files

Open `out/<run-id>/google-ads/bulk/5-responsive-search-ads.csv` and `3-keywords.csv`. They are plain UTF-8 text with the same Search content as `editor/account.csv`. Read them only. You import `editor/account.csv`.

Read every headline and description out loud.

- Does each claim come from facts you gave?
- Does it sound like your brand and your language?
- Does each final URL fit its keywords?
- Do the negatives block searches you want?

If the run lists a restricted category, read the Restricted categories section. It has the text to paste when Google refuses an item. See [Handle restricted categories](handle-restricted-categories.md).

With `--layout drive-folders` there is no `bulk/` folder and no `account.csv`. See [Use the ads automation package](use-the-automation-package.md).

### Change something without a model call

To change `export.status`, `url_suffix` or `decimal_comma`, edit the `input.export` object in `workspace.json`, keep a copy first, and run `mads export <run-dir>`. `export` reads `workspace.json`, not `input/business.toml`. See [Resume or re-export a run](resume-a-run.md).

To change anything else, edit `business.toml` and run `mads generate` again.

## Check it worked

- The status line says `Ready to import.`
- Every pre-flight key matches your account and your claims.
- The budgets add up, and each CPC is at or under `max_cpc`.
- You decided on every `W01`.
- Every campaign row says `Paused`.
- You read the ads and found no claim you cannot back.

## Next

- [Import into Google Ads Editor](import-into-google-ads-editor.md)
- [Go live](go-live.md)
- [report.md](../reference/report-md.md)
