# Glossary

Look up a Google Ads or mads term that other pages assume you know.

Terms are in alphabetical order. The last column links to the page that explains the term.

| Term | Meaning | See |
|---|---|---|
| Ad group | The unit of a Search campaign that holds keywords and one responsive search ad. Image campaigns have asset groups instead. | [How a run works](../explanation/how-a-run-works.md) |
| App installs | A campaign format for an app in Google Play or the App Store. It uses asset groups and has no bulk upload file. | [Image and app campaigns](../explanation/image-campaigns.md) |
| Asset group | The unit of an image campaign (Performance Max, Demand Gen or App installs) that holds texts and pictures, in the place of an ad group. | [Image and app campaigns](../explanation/image-campaigns.md) |
| Base run | The finished run that `mads optimize` starts from. It is read and never changed. | [How optimize decides](../explanation/how-optimize-decides.md) |
| Brand kit | The headlines and descriptions that apply to every ad of a run. At export they complete each ad to 15 headlines and 4 descriptions. | [How a run works](../explanation/how-a-run-works.md) |
| Bulk upload | The five CSV files under `bulk/` that the Google Ads web interface imports. They are not the file for Editor. | [Bulk upload format](bulk-upload-format.md) |
| Callout | A short line of text, such as "Free shipping", shown under a Search ad. mads validates callouts but does not export them. | [Go live](../how-to/go-live.md) |
| Campaign | The top unit of an account. It has a format, a budget, a bid strategy and an intent. | [Campaign strategies](../explanation/strategies.md) |
| Cents | How mads stores money, as a whole number of hundredths of the account currency. | [business.toml](business-toml.md) |
| Conversion | An action you count as a result, such as a purchase or an install. The conversion reports tell mads which actions each campaign counted. | [Google Ads reports](google-ads-reports.md) |
| Conversion action | One named kind of conversion in Google Ads. A campaign can count several. | [Google Ads reports](google-ads-reports.md) |
| CPC | Cost per click, in the account currency. The `default_cpc` of an ad group is its bid. mads accepts only `manual_cpc` bidding. | [Campaign strategies](../explanation/strategies.md) |
| Demand Gen | An image campaign format. It needs at least 1 landscape or square picture and takes vertical pictures for mobile feeds. | [Image and app campaigns](../explanation/image-campaigns.md) |
| Drift | A change in Google Ads since the base run was exported. mads flags a budget that moved by more than 20 percent, a different bid strategy, and a campaign that was paused, removed or turned on again. | [How optimize decides](../explanation/how-optimize-decides.md) |
| Editor file | `editor/account.csv`, the only file to import into Google Ads Editor. | [Google Ads Editor file](editor-file.md) |
| Final URL suffix | Tracking parameters that Google adds to the landing page URL. mads writes it from `export.url_suffix`. | [Review a run](../how-to/review-a-run.md) |
| First page bid | The bid Google estimates a keyword needs to reach the first page of results. A bid under it gets little or no traffic, and Google marks the keyword "below first page bid". | [Campaign strategies](../explanation/strategies.md) |
| Google Ads Editor | Google's desktop app for editing an account offline and posting the changes. | [Import into Google Ads Editor](../how-to/import-into-google-ads-editor.md) |
| Intent | What a Search campaign is for. It is one of `brand`, `catalog`, `generic` or `competitor`. | [Campaign strategies](../explanation/strategies.md) |
| Match type | How closely a search must match a keyword. mads writes Phrase and Exact. Broad keywords can exist in a live account and mads reads them from reports. | [Validation rules](validation-rules.md) |
| Mission | One job of one agent. A run is a `plan` mission, then one `campaign:<slug>` mission per campaign. `mads init` is a separate mission. | [How a run works](../explanation/how-a-run-works.md) |
| Negative keyword | A term that stops an ad from showing. Campaign negatives are written into every ad group row of `4-negative-keywords.csv`. mads also adds brand terms as negatives outside the brand campaign, and competitor names outside the competitor campaign. | [How a run works](../explanation/how-a-run-works.md) |
| Performance Max | An image campaign format. It needs at least 1 landscape and 1 square picture and takes no vertical pictures. | [Image and app campaigns](../explanation/image-campaigns.md) |
| RSA | Responsive search ad. It has up to 15 headlines and 4 descriptions, and Google combines them. | [Validation rules](validation-rules.md) |
| Run | One execution of `mads generate` or `mads optimize`, written to `out/<run>/`. | [Output files](output-files.md) |
| Sitelink | An extra link under a Search ad, with its own text and URL. mads validates sitelinks but does not export them. | [Go live](../how-to/go-live.md) |
| Stalled | A campaign that is enabled and has no ads, or has had no impressions for 7 days. It is broken, not short of data, and it wins over thin. | [How optimize decides](../explanation/how-optimize-decides.md) |
| Structured snippet | A header such as "Brands" with a list of values, shown under a Search ad. mads validates it but does not export it. | [Go live](../how-to/go-live.md) |
| Thin | A campaign with too little data to judge. It has under 14 days or under 100 clicks, and under 30 conversions. Optimize changes only its structure. | [How optimize decides](../explanation/how-optimize-decides.md) |

## Next

- [Why mads](../explanation/why-mads.md)
- [CLI](cli.md)
