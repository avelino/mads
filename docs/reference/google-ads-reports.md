# Google Ads reports

Look up how `mads optimize` reads the CSV reports you export from Google Ads: which file counts as which report, which columns it reads and what the terminal prints about them.

For when to export and how to do it in the Google Ads interface, see [Export Google Ads reports](../how-to/export-google-ads-reports.md). For what mads decides from the numbers, see [How optimize decides](../explanation/how-optimize-decides.md).

## The reports folder

`mads optimize <run> --reports <dir>` reads the folder you pass.

| Rule | Value |
|---|---|
| Files read | Every file with the extension `.csv` (any letter case), in name order. Other files are ignored. |
| File names | Not used. A file is told apart by its columns. |
| Encoding | UTF-8 with or without a BOM, or UTF-16 LE with a BOM (the "Excel" download). Anything else is skipped with `not UTF-8 or UTF-16 text`. |
| Delimiter | Comma or tab. |
| Layout | Title line, date range line, header, rows. Lines before the header are skipped. The first line that is a known header is the header. |
| Total rows | A row whose first cell starts with `Total:` is dropped. A name that merely starts with "total" is a real row. |
| Empty cell | `--` or blank. Text columns read it as empty and number columns as no value. |
| Numbers | Report with a `Campanha` column: `1.020,50`. Any other report: `1,020.50`. A trailing `%` is stripped. |
| Languages | English and Portuguese column names. |
| Column names | Compared after dropping case, accents and punctuation. `Impr.` and `impr` are the same name. The whole name must match, except where a table says "words". |
| Date range | `Oct 3, 2026 - Oct 5, 2026` or `3 de outubro de 2026 - 5 de outubro de 2026`. Both days count. Months are read by their first three letters, in English or Portuguese. |
| Campaign link | A row joins a campaign of the run by campaign name, compared with `normalize`. A name not in the run goes to `not in this run` and its rows are not used, except campaign and by-day rows, which feed the outside-the-run numbers. |

At least one file must be a report. If none is, the command exits with code 1 and this message, where `details` lists each file with the reason it was refused, or says `no .csv file`:

```text
no Google Ads report in <dir>: export search terms, keywords or campaigns as CSV (<details>)
```

A folder that cannot be opened exits with `cannot read reports folder <dir>: <reason>`.

## How a file is told apart

The first rule that matches wins.

| Order | The header has | The file is |
|---|---|---|
| 1 | `Search term` or `Termo de pesquisa` | Search terms |
| 2 | `Keyword`, `Search keyword` or `Palavra-chave`, and `Match type` or `Tipo de corresp.` | Keywords |
| 3 | `Asset` or `Recurso` | Assets |
| 4 | A campaign column and no ad group column, with `Conversion action` or `Ação de conversão` | Conversions by action |
| 5 | A campaign column and no ad group column, with `Network` or `Rede` (the `(with search partners)` suffix is fine) | Networks |
| 6 | A campaign column and no ad group column, with `Day` or `Dia` | Campaigns by day |
| 7 | A campaign column and no ad group column | Campaigns |
| 8 | A campaign column and an ad group column, with no `Keyword`, no `Day` and no ad column | Ad groups |
| none | Anything else | Skipped |

A campaign column is `Campaign` or `Campanha`. An ad group column is `Ad group`, `Grupo de anúncios`, `Asset group` or `Grupo de recursos`. An ad column is `Ad status`, `Ad type`, `Status do anúncio` or `Tipo de anúncio`.

Two files that look close are not read:

- A keywords export without `Match type` has `Keyword` and `Ad group`, so rule 8 refuses it.
- An ads report has an ad column, so rule 8 refuses it.

A network column named `Networks`, or a lost impression share column, does not make a Campaigns report another kind.

## Columns read

The tables below list every column mads reads. A column that is not listed is ignored. A listed column that is absent reads as zero or as no value, except where "Needed to tell it apart" says otherwise.

### Numbers shared by most reports

Campaigns, campaigns by day, keywords, search terms, ad groups and networks all read these.

| Column | English | Portuguese |
|---|---|---|
| Impressions | `Impr.`, `Impressions` | `Impr.`, `Impressões` |
| Clicks | `Clicks` | `Cliques` |
| Cost | `Cost` | `Custo` |
| Conversions | `Conversions` | `Conversões` |
| Conversion value | `Conv. value`, `Conversion value` | `Valor conv.` |

### Search terms

Needed to tell it apart: `Search term`.

| Column | English | Portuguese | Read as |
|---|---|---|---|
| Campaign | `Campaign` | `Campanha` | The campaign of the run. |
| Ad group | `Ad group` | `Grupo de anúncios` | The ad group of the term. |
| Match type | `Match type` | `Tipo de corresp.` | Kept as written. |
| Added or excluded | `Added/Excluded` | `Adicionada/excluída` | See the term states below. |
| Numbers | | | The shared numbers. |

Term states, checked in this order:

| State | When |
|---|---|
| `keyword` | The term is a keyword of that ad group in the run. |
| `negative` | A campaign or ad group negative of the run blocks it. |
| `excluded` | `Added/Excluded` contains `exclu`. |
| `keyword` | `Added/Excluded` contains `adicion` or `added`. |
| `new` | None of the above. |

mads keeps the 30 terms with the highest cost in each ad group (`TERMS_PER_GROUP`, by cost, then impressions). It also records how many terms the group had and the cost of those without conversions.

### Keywords

Needed to tell it apart: a keyword column and `Match type`.

| Column | English | Portuguese | Read as |
|---|---|---|---|
| Keyword | `Keyword`, `Search keyword` | `Palavra-chave` | Normalized. Quotes and brackets are removed. |
| Match type | `Match type` | `Tipo de corresp.` | `phrase` when it contains `phrase` or `frase`, `exact` for `exact` or `exat`, `broad` for `broad` or `ampla` unless it also contains `modif`. Anything else is unknown. |
| Campaign | `Campaign` | `Campanha` | The campaign of the run. |
| Ad group | `Ad group` | `Grupo de anúncios` | The ad group of the keyword. |
| Keyword status | `Keyword status` | `Status da palavra-chave` | `Enabled`, `Paused` or `Removed`. The plain `Status` column is a serving status and is not read. |
| Max. CPC | `Max. CPC` | `CPC máx.` | The bid the keyword ran with. |
| Quality Score | `Quality Score`, `Qual. score` | `Índice de qualidade` | A number. |
| Status reasons | `Status reasons` | `Motivos do status` | Text, see the signals below. |
| First page bid | words `first` and `page` | words `primeira` and `página` | A number. |
| Top of page bid | words `top` and `page` | words `topo` and `página` | A number. |
| Numbers | | | The shared numbers. |

Signals taken from a keyword:

| Signal | When |
|---|---|
| Below first page | Status reasons contain `first page` or `primeira página`. |
| Rarely shown | Status reasons contain `rarely` or `raramente`. |
| Low quality | Status reasons contain `low quality` or `baixa qualidade`, or Quality Score is 4 or less. |

### Campaigns

Needed to tell it apart: a campaign column, and none of the columns of rules 1 to 6.

| Column | English | Portuguese | Read as |
|---|---|---|---|
| Campaign | `Campaign` | `Campanha` | The campaign of the run. |
| Campaign status | `Campaign status` | `Status da campanha` | Text starting with `enabled`, `active` or `ativ` is Enabled, `paus` is Paused, `remov` is Removed. Anything else is unknown. |
| Status reasons | `Status reasons` | `Motivos do status` | A campaign that is Enabled and has `no ads` or `nenhum anúncio` here is stalled. |
| Budget | `Budget` | `Orçamento` | The daily budget, a number. |
| Bid strategy type | `Bid strategy type` | `Tipo de estratégia de lances` | Text. Compared with the strategy of the run. |
| Lost impression share to budget | words `lost` and `budget` | words `perdidas` or `perdida`, and `orçamento` | A percent. |
| Lost impression share to rank | words `lost` and `rank` | words `perdidas` or `perdida`, and `classificação` | A percent. |
| Numbers | | | The shared numbers. |

A campaign Enabled with 0 impressions over 7 or more days is also stalled (`STALLED_DAYS`).

### Campaigns by day

Needed to tell it apart: a campaign column and `Day` or `Dia`.

It reads the same columns as Campaigns, plus the day.

| Rule | Value |
|---|---|
| Totals | When a Campaigns report has the campaign, its numbers win and the day rows add nothing to the totals. Without one, the day rows are summed. |
| Status, budget, strategy | Taken from the row of the latest day. |
| Lost impression share | Averaged over the day rows when no Campaigns report has the campaign. |
| Days running | Counted from the first day with impressions to the end of the date range. |

### Conversions by action

Needed to tell it apart: a campaign column and a conversion action column.

| Column | English | Portuguese | Read as |
|---|---|---|---|
| Campaign | `Campaign` | `Campanha` | The campaign of the run. |
| Conversion action | `Conversion action`, `Conversion action name` | `Ação de conversão`, `Nome da ação de conversão` | A row with `--` or blank is left out. Rows of the same action add up. |
| Conversion category | `Conversion category`, `Conversion action category` | `Categoria de conversão`, `Categoria da ação de conversão` | Kept when present. |
| Conversion source | `Conversion source` | `Origem da conversão` | Kept when present. |
| Conversions | `Conversions` | `Conversões` | What bidding learns from. |
| All conversions | `All conv.`, `All conversions` | `Todas as conv.`, `Todas as conversões` | Kept when present. Counts also what bidding ignores. |

mads keeps the 5 actions with most conversions per campaign (`ACTIONS_PER_CAMPAIGN`). An action counts as an install when its name or category contains `download`, `instal`, `first open` or `primeira abertura`. The store of an action is read from the words Android, Google Play, iOS, iPhone or App Store in its name, category or source. An action that names no store counts for both stores. When this report is read, a campaign with no action in it counted none.

### Networks

Needed to tell it apart: a campaign column and a network column.

| Column | English | Portuguese | Read as |
|---|---|---|---|
| Campaign | `Campaign` | `Campanha` | The campaign of the run. |
| Network | `Network`, `Network (with search partners)` | `Rede`, `Rede (com parceiros de pesquisa)` | Mapped to a network, see below. |
| Numbers | | | The shared numbers. |

The network of a row comes from words in its name, checked in this order.

| Network | The name contains |
|---|---|
| Search partners | `partner` or `parceiro` |
| YouTube | `youtube` |
| Display | `display`, `content` or `conteudo` |
| Gmail | `gmail` |
| Maps | `maps` or `mapas` |
| Discover | `discover` or `descoberta` |
| Search | `search` or `pesquisa` |
| Other | Anything else, such as a mixed row. |

For each campaign mads keeps the share of cost, the share of conversions and the CTR of every network that has impressions, cost or conversions, the one with most cost first. An App campaign has its Display clicks flagged as likely accidental when it has at least 30 Display clicks (`DISPLAY_MIN_CLICKS`) and a Display CTR above 15 percent (`DISPLAY_CTR_PCT`). Both lines are drawn by mads. Google has none. This report does not replace Campaigns for totals, status or budget.

### Ad groups

Needed to tell it apart: a campaign column, an ad group column, and no keyword, day or ad column.

| Column | English | Portuguese | Read as |
|---|---|---|---|
| Campaign | `Campaign` | `Campanha` | The campaign of the run. |
| Ad group | `Ad group`, `Asset group` | `Grupo de anúncios`, `Grupo de recursos` | The ad group of the run. |
| Ad group status | `Ad group status`, `Asset group status` | `Status do grupo de anúncios`, `Status do grupo de recursos` | Enabled, Paused or Removed, read like the campaign status. |
| Numbers | | | The shared numbers. |

It gives totals per ad group. It says nothing about the keywords of the group. Without a Campaigns report, the campaign totals are the sum of its ad groups, or of its keywords when no ad group report is there.

### Assets

Needed to tell it apart: `Asset` or `Recurso`.

| Column | English | Portuguese | Read as |
|---|---|---|---|
| Asset | `Asset` | `Recurso` | The asset text. |
| Campaign | `Campaign` | `Campanha` | The campaign of the run. |
| Ad group | `Ad group`, `Asset group` | `Grupo de anúncios`, `Grupo de recursos` | The group of the asset. |
| Asset type | `Asset type` | `Tipo de recurso` | Text. |
| Performance label | word `performance` | word `desempenho` | Text, such as the label Google shows. |
| Impressions | `Impr.`, `Impressions` | `Impr.`, `Impressões` | A number. |

## The ads report

An ads report has an ad column. It is refused, and mads reads no ad-level numbers. The line is printed as one line:

```text
skipped: Relatório de anúncios.csv: no known report header (search terms, keywords, campaigns, campaigns by day, ad groups, assets, conversion actions or networks): an ads report has one row per ad, and mads reads no ad-level numbers. Export the search keywords and search terms reports instead
```

Any other refused file prints the first six header names it saw:

```text
skipped: notes.csv: no known report header (search terms, keywords, campaigns, campaigns by day, ad groups, assets, conversion actions or networks), saw: Foo, Bar, Baz
```

## What `mads optimize` prints about the reports

These lines come before the missions start. They are printed in the order below when they apply. `app events: <file> read` comes after `skipped`, and `too little data, structure only: <campaigns>` comes after `not in this run`. The finding lines follow, and `missing reports, keywords kept as they ran` is the last of them.

| Line | Meaning |
|---|---|
| `reports: N read (<start> to <end>, <days> days)` | N files were read. The range is the first one found, files in name order. It reads `no date range` when no file has one. |
| `skipped: <file>: <reason>` | One line per refused file. |
| `warning: the reports cover different dates, export them for the same dates` | The files do not share one date range. The cost that campaign rows have and search terms do not list (`hidden_terms_cost`) is not computed. |
| `not in this run: <campaigns>` | Campaign names in the reports that the run does not have. |
| `missing reports, keywords kept as they ran: <kinds>` | A kind is missing. It lists only `Keywords` and `SearchTerms`. |

A report counts as missing in these cases.

| Missing kind | Satisfied by |
|---|---|
| Campaigns | A Campaigns or a Campaigns by day file |
| Keywords | A Keywords file |
| SearchTerms | A Search terms file |
| Assets | An Assets file |

Ad groups, conversions by action and networks are never listed as missing.

When the Keywords report is missing, an ad group that ran and is rebuilt without some of its keywords gets them back, with warning `W10`. A missing search terms report does not trigger `W10`. See [Validation rules](validation-rules.md).

## A campaign with little data

These numbers decide how a campaign is judged. They come from `perf/model.rs`.

| Name | Value | Effect |
|---|---|---|
| `THIN_DAYS` | 14 | Fewer days than this makes the campaign short. |
| `THIN_CLICKS` | 100 | Fewer clicks than this makes the campaign short. |
| `JUDGE_CONVERSIONS` | 30 | A campaign with this many conversions is never thin. |
| `STALLED_DAYS` | 7 | Enabled and silent for this many days is stalled. |

A campaign is thin when it is not stalled, has fewer than 30 conversions and is short.

## Next

- [Export Google Ads reports](../how-to/export-google-ads-reports.md) for the steps in the Google Ads interface.
- [How optimize decides](../explanation/how-optimize-decides.md) for what mads does with these numbers.
- [Exit codes and errors](exit-codes-and-errors.md) for the messages of a failed read.
