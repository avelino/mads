# Limitations

Look up what mads does not do, what it does only in part, and what has not been checked against a real Google Ads import.

Other pages link here for these facts. Nothing else keeps a copy.

## What mads does not do

| Limit | What happens instead |
|---|---|
| Upload | mads writes files. You import them yourself. See [Import into Google Ads Editor](../how-to/import-into-google-ads-editor.md). |
| Google Ads API access | mads never reads or writes your account through the API. `mads optimize` reads the report CSVs you export and compares them with the base run. See [Google Ads reports](google-ads-reports.md). |
| Auction data | Bids and budget shares are estimates. mads has no Keyword Planner access. |
| Broad match | Keywords are Phrase or Exact. `MatchType` has no other value. |
| Campaign kinds beyond four | Search, Performance Max, Demand Gen and App installs only. No Shopping, video, Display-only or Dynamic Search Ads. |
| Attaching pictures to ads | Editor does not import the link between an ad and its images. You attach the files listed in `report.md`. See [Add image and app campaigns](../how-to/add-image-campaigns.md). |
| Sitelink, callout and snippet files | They are collected and validated, but not exported. They wait for real Google templates. |
| Results | mads guarantees the files follow the format and the rules. It does not guarantee the ads convert. |

## Bidding

| Campaign kind | Accepted bid strategy | Otherwise |
|---|---|---|
| Search | `manual_cpc` | The plan tool returns `UNSUPPORTED`. Validation returns `E17`. |
| Performance Max | `maximize_conversions` | `E17` |
| Demand Gen | `maximize_clicks` or `maximize_conversions` | `E17` |
| App installs | `maximize_conversions` | `E17` |

`maximize_conversions` needs `business.conversion_tracking = true` (`E10`), except on App installs. See [Validation rules](validation-rules.md).

## Export

| Limit | Detail |
|---|---|
| Bulk CSVs hold Search only | Performance Max, Demand Gen and App campaigns exist only in `editor/account.csv`. See [Bulk upload format](bulk-upload-format.md). |
| Campaign negatives are expanded | The campaign negative list is written as ad group negatives in every ad group row of `4-negative-keywords.csv`, not as campaign rows. |
| Places | `business.locations` takes 1 to 10 entries for the whole account. Every campaign targets the same places. The bulk file puts them in one cell of `1-campaign.csv`, separated by `;`. See [business.toml](business-toml.md). |
| In-app action goal | An app with `optimize_for = "in_app_actions"` writes `Campaign optimization` as `In-app actions`, plus `Conversion action` and `Target CPA`, in the Editor file. Only `Installs` was checked against a real Editor export, so these names are unverified. The drive-folders package ignores the goal. See [Editor file](editor-file.md). |
| Image campaigns need inputs | Performance Max and Demand Gen need an image model and `[brand] logo`. App installs need an image model and an `[app]` entry. Without them the plan tool returns `UNSUPPORTED`. |

## Not verified against a real import

| Part | State |
|---|---|
| Search and App rows in the Editor file | Match a real export of Editor 2.13.3. |
| Demand Gen rows | A file with Search, App and Demand Gen imported into Editor 2.13.3 with no error. |
| Performance Max rows | Follow Google's documented headers. Not imported yet. |
| Several places in one `1-campaign.csv` cell | Not tried in a real bulk upload. |

## Text and input

| Limit | Detail |
|---|---|
| Length checks | They count Unicode characters after NFC normalization. |
| `init` reads HTML only | The fetcher skips `script` and `style` content and runs no JavaScript. A site that renders in the browser gives the agent little to read. See [Draft from your site](../how-to/draft-from-your-site.md). |
| `codex-cli` and `gemini-cli` | Their stream parsers follow the documented formats and are tested against recorded fixtures. They were not run against the real CLIs. `claude-cli` ran for real. See [Agent CLIs](../explanation/agent-clis.md). |

## Next

- [Limits](limits.md)
- [Validation rules](validation-rules.md)
- [Import into Google Ads Editor](../how-to/import-into-google-ads-editor.md)
