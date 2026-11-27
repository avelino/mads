# Bulk upload format

Look up every column of the five CSV files mads writes for the Google Ads web bulk upload, so you can check them, edit them or build tooling around them.

These files go through Tools, Bulk actions, Uploads in Google Ads. They do not go into Google Ads Editor. Editor takes `google-ads/editor/account.csv`, which holds the same Search campaigns, so importing both creates every Search campaign twice. See [Editor file](editor-file.md). For when to use the bulk upload, see [Upload the bulk CSVs](../how-to/upload-bulk-csvs.md).

## Scope

| Item | Value |
|---|---|
| Directory | `google-ads/bulk/` in the run directory |
| Written when | `--layout bulk` (the default of `mads generate`) and the run has at least one Search campaign |
| Campaign kinds | Search only. Performance Max, Demand Gen and App campaigns are skipped and exist only in the Editor file |
| Not written with | `--layout drive-folders`. See [Automation package](automation-package.md) |
| Reference data | `data/` in the repository (Vinellu, 1 campaign, 12 ad groups). A golden test requires the export to match it byte for byte |

See [Output files](output-files.md) for the full run directory.

## File rules

| Rule | Value |
|---|---|
| Line ending | CRLF after every record, including the last |
| Encoding | UTF-8 without BOM |
| Quoting | Only when needed. A field with a comma is quoted |
| Decimal separator | Comma when `export.decimal_comma` is true, else a dot |
| Linking | Rows point to their parents by name (`Campaign`, `Ad group`), never by ID |
| Header | Every file starts with a header row |
| `Action` | `Add` on every data row |

`export.decimal_comma` defaults to true when the primary subtag of `business.language` is `pt`, `es`, `fr`, `de` or `it`, and to false otherwise. See [business.toml](business-toml.md).

## Files and upload order

Upload in numeric order. A row can only point at a campaign or ad group that already exists.

| File | One row per | `Row Type` |
|---|---|---|
| `1-campaign.csv` | campaign | `Campaign` |
| `2-ad-groups.csv` | ad group | `Ad group` |
| `3-keywords.csv` | keyword | `Keyword` |
| `4-negative-keywords.csv` | negative per ad group | `Negative keyword` |
| `5-responsive-search-ads.csv` | ad group (its ad) | `Ad` |

## 1-campaign.csv

Header and one row from `data/1-campaign.csv`:

```csv
Row Type,Action,Campaign status,Campaign,Campaign type,Networks,Budget,Budget type,Bid strategy type,Language,Location,Final URL suffix,EU political ads
Campaign,Add,Paused,Vinellu - Rotulos - Search,Search,Google search,50,Daily,Manual CPC,pt,Brazil,utm_source=google&utm_medium=cpc&utm_campaign=rotulos&utm_content={adgroupid}&utm_term={keyword},No
```

| Column | Value |
|---|---|
| `Campaign status` | `export.status`. `Paused` unless set to `Enabled` |
| `Campaign` | The campaign name |
| `Campaign type` | `Search` |
| `Networks` | `Google search` |
| `Budget` | The campaign daily budget. No decimals when whole (`50`), else 2 decimals (`37,50` or `37.50`) |
| `Budget type` | `Daily` |
| `Bid strategy type` | `Manual CPC`. No other value is exported, and any other strategy fails the export. Empty when the run comes from `mads optimize` and the campaign report shows that the live campaign bids with another strategy |
| `Language` | Primary subtag of `business.language` (`pt-BR` gives `pt`) |
| `Location` | Every `business.locations` entry joined by `;` |
| `Final URL suffix` | `export.url_suffix` with `{mads_campaign}` replaced by the campaign slug |
| `EU political ads` | `Yes` when `export.eu_political_ads` is true, else `No` |

`Bid strategy type` is `Manual CPC` when there is no campaign report, when the report has no strategy for the campaign, or when the live strategy is Manual CPC.

The default `export.url_suffix` is `utm_source=google&utm_medium=cpc&utm_campaign={mads_campaign}&utm_content={adgroupid}&utm_term={keyword}`.

## 2-ad-groups.csv

```csv
Row Type,Action,Ad group status,Campaign,Ad group,Ad group type,Default max. CPC
Ad group,Add,Enabled,Vinellu - Rotulos - Search,rotulo-alamos,Standard,"1,50"
```

| Column | Value |
|---|---|
| `Ad group status` | `Enabled` |
| `Campaign` | The campaign name |
| `Ad group` | The ad group name |
| `Ad group type` | `Standard` |
| `Default max. CPC` | The ad group default bid, always with 2 decimals |

## 3-keywords.csv

```csv
Row Type,Action,Keyword status,Campaign,Ad group,Keyword,Type
Keyword,Add,Enabled,Vinellu - Rotulos - Search,rotulo-alamos,alamos,Phrase match
Keyword,Add,Enabled,Vinellu - Rotulos - Search,rotulo-alamos,alamos harmonização,Phrase match
```

| Column | Value |
|---|---|
| `Keyword status` | `Enabled` |
| `Keyword` | The keyword text |
| `Type` | `Phrase match` or `Exact match` |

Keywords built by the expansion step come out in this order: phrase keywords in byte order, then the exact keywords in the order of the variants, then explicit extras that are not already present. See [How a run works](../explanation/how-a-run-works.md).

## 4-negative-keywords.csv

```csv
Row Type,Action,Keyword status,Level,Campaign,Ad group,Negative keyword,Type
Negative keyword,Add,Enabled,Ad group,Vinellu - Rotulos - Search,rotulo-alamos,emprego,Phrase match
```

| Column | Value |
|---|---|
| `Keyword status` | `Enabled` |
| `Level` | `Ad group` on every row |
| `Negative keyword` | The negative text |
| `Type` | `Phrase match` or `Exact match` |

Campaign negatives are written into every ad group. For each ad group the campaign negatives come first, then the ad group's own, without duplicates (compared after normalization and by exact or non-exact match). A campaign with 8 negatives and 12 ad groups and no ad group negatives writes 96 rows. Rows with `Level` `Campaign` are not written.

## 5-responsive-search-ads.csv

The header has 28 columns:

| Columns | Value |
|---|---|
| `Row Type` | `Ad` |
| `Action` | `Add` |
| `Ad status` | `Enabled` |
| `Campaign`, `Ad group` | Names |
| `Ad type` | `Responsive search ad` |
| `Headline 1` to `Headline 15` | Merged headlines. Empty past the last one |
| `Description 1` to `Description 4` | Merged descriptions. Empty past the last one |
| `Path 1`, `Path 2` | The ad paths. Empty when unset |
| `Final URL` | The ad group landing page |

Headlines and descriptions come from the ad group texts first. The brand kit fills the rest up to 15 headlines and 4 descriptions. A brand kit text equal to an ad group text, after normalization, is dropped.

## Not written

| Item | State |
|---|---|
| Sitelinks, callouts and structured snippets | Collected and validated, not exported. No verified template |
| Campaign-level negative rows | Expanded per ad group instead |
| Bid strategy labels for Maximize clicks and Maximize conversions | Only `manual_cpc` is accepted. The plan tool returns `UNSUPPORTED`, and the export fails with `EXPORT`: `<label> needs a Google Ads bulk upload template that is not verified yet` |

See [Limitations](limitations.md).

## Next

- [Upload the bulk CSVs](../how-to/upload-bulk-csvs.md)
- [Editor file](editor-file.md)
- [Output files](output-files.md)
