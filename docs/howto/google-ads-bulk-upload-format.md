# Google Ads bulk upload format

This page describes the five CSV files mads writes, column by column, so you can check them, edit them or build tooling around them.

These files hold Search campaigns only. Performance Max and Demand Gen campaigns go to a Google Ads Editor file, see [Image campaigns](../guides/image-campaigns.md).

The format comes from a real Google Ads export. The files in `data/` (Vinellu, 1 campaign, 12 ad groups) are the reference. A golden test exports files 1 to 5 from a fixture and requires them to match `data/` byte for byte.

## File rules

| Rule | Value |
|---|---|
| Line ending | CRLF after every record, including the last |
| Encoding | UTF-8 without BOM |
| Quoting | Only when needed. A field with a comma is quoted. |
| Decimal separator | Comma (`"1,50"`) when `export.decimal_comma` is true, else a dot |
| Linking | Rows reference their parents by name (`Campaign`, `Ad group`), never by ID |

Every file starts with a header row. Every data row has `Action` set to `Add`.

## Upload order

Upload in numeric order. A row can only point at a campaign or ad group that already exists.

| File | One row per | Row Type |
|---|---|---|
| `1-campaign.csv` | campaign | `Campaign` |
| `2-ad-groups.csv` | ad group | `Ad group` |
| `3-keywords.csv` | keyword | `Keyword` |
| `4-negative-keywords.csv` | negative per ad group | `Negative keyword` |
| `5-responsive-search-ads.csv` | ad group (its ad) | `Ad` |

## 1-campaign.csv

```csv
Row Type,Action,Campaign status,Campaign,Campaign type,Networks,Budget,Budget type,Bid strategy type,Language,Location,Final URL suffix,EU political ads
Campaign,Add,Paused,Vinellu - Rotulos - Search,Search,Google search,50,Daily,Manual CPC,pt,Brazil,utm_source=google&utm_medium=cpc&utm_campaign=rotulos&utm_content={adgroupid}&utm_term={keyword},No
```

| Column | Value |
|---|---|
| `Campaign status` | `export.status`, default `Paused` |
| `Campaign` | The campaign name |
| `Campaign type` | `Search` |
| `Networks` | `Google search` |
| `Budget` | The campaign daily budget. No decimals when whole (`50`), else 2 (`37,50`) |
| `Budget type` | `Daily` |
| `Bid strategy type` | `Manual CPC`. No other value is exported today. |
| `Language` | Primary subtag of `business.language` (`pt-BR` gives `pt`) |
| `Location` | The single `business.locations` entry |
| `Final URL suffix` | `export.url_suffix` with `{mads_campaign}` replaced by the campaign slug |
| `EU political ads` | `Yes` or `No` from `export.eu_political_ads` |

## 2-ad-groups.csv

```csv
Row Type,Action,Ad group status,Campaign,Ad group,Ad group type,Default max. CPC
Ad group,Add,Enabled,Vinellu - Rotulos - Search,rotulo-alamos,Standard,"1,50"
```

`Ad group status` is `Enabled`. `Ad group type` is `Standard`. `Default max. CPC` always has 2 decimals.

## 3-keywords.csv

```csv
Row Type,Action,Keyword status,Campaign,Ad group,Keyword,Type
Keyword,Add,Enabled,Vinellu - Rotulos - Search,rotulo-alamos,alamos malbec review,Phrase match
Keyword,Add,Enabled,Vinellu - Rotulos - Search,rotulo-alamos,alamos malbec,Exact match
```

`Type` is `Phrase match` or `Exact match`. Phrase keywords come first in byte order, then exact keywords. See [How campaigns are built](../guides/how-campaigns-are-built.md#keywords).

## 4-negative-keywords.csv

```csv
Row Type,Action,Keyword status,Level,Campaign,Ad group,Negative keyword,Type
Negative keyword,Add,Enabled,Ad group,Vinellu - Rotulos - Search,rotulo-alamos,emprego,Phrase match
```

Every row has `Level` set to `Ad group`. Campaign-level negatives are written into every ad group, campaign ones first, then the ad group's own, without duplicates. Campaign-level rows (Level `Campaign` with an empty `Ad group` cell) wait for a verified Google template. A campaign with 8 negatives and 12 ad groups exports 96 rows.

## 5-responsive-search-ads.csv

```csv
Row Type,Action,Ad status,Campaign,Ad group,Ad type,Headline 1,...,Headline 15,Description 1,...,Description 4,Path 1,Path 2,Final URL
```

(The real file has all 15 headline and 4 description columns. The dots shorten the example.)

- `Ad type` is `Responsive search ad`.
- Headlines and descriptions are the merge of the ad group texts and the brand kit. See [How campaigns are built](../guides/how-campaigns-are-built.md#ads).
- Cells are empty when the ad has fewer than 15 headlines or 4 descriptions.
- `Path 1` and `Path 2` are empty when unset.
- `Final URL` is the ad group's landing page.

## Files not written yet

Files 6 to 8 would hold sitelinks, callouts and structured snippets. Their column layouts need templates from a real Google Ads account, so they are not written. mads collects and validates those assets.

Two more formats wait for templates.

- Campaign-level negative rows.
- Bid strategy labels for Maximize clicks (with a CPC cap) and Maximize conversions.

Until the templates exist, only `manual_cpc` is accepted and campaign negatives are expanded per ad group.

## Get your own template

Google Ads shows the current columns for your account. In Tools, Bulk actions, Uploads, use the templates option to download one. Compare its headers with the files here when Google rejects a column.
