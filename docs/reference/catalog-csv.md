# catalog.csv reference

This page lists every column of `catalog.csv` with its rule.

## File

- A header row is required.
- UTF-8, with or without a BOM. LF or CRLF line endings.
- At most 5000 rows.
- Header names are case-insensitive. Column order does not matter. Unknown columns are ignored.
- Fields are trimmed. Quote a field that contains a comma.

## Columns

| Column | Required | Rule |
|---|---|---|
| `name` | yes | 1 to 120 characters |
| `url` | yes | Absolute `http` or `https` URL. Unique in the file after normalization. |
| `category` | no | Free text. Groups entities for the plan mission. `query_catalog` filters on it, exact and case-insensitive. |
| `aliases` | no | Alternative names separated by `\|`. Empty parts are dropped. Used as keyword variants. |
| `third_party` | no | `true` or `false`, case-insensitive. Empty means `false`. `true` marks someone else's trademark (warning `W01`). |
| `notes` | no | Free text handed to the agent. At most 500 characters. |
| `image` | no | Absolute `http` or `https` URL of a real photo of the item. Image campaigns send it to the image model as a reference, so the picture shows the real product. `mads init` fills it from the item page's `og:image`. |

## Ids

Each row gets `id = slugify(name)`. Duplicates get `-2`, `-3` and so on, in file order.

`slugify` normalizes to NFD, drops combining marks, lowercases, turns every run of characters outside `a-z` and `0-9` into one `-` and trims `-` at both ends.

| name | id |
|---|---|
| `Angélica Zapata` | `angelica-zapata` |
| `Alamos Malbec` | `alamos-malbec` |
| `Alamos Malbec` (second row) | `alamos-malbec-2` |

## Example

```csv
name,url,category,aliases,third_party,notes
Alamos Malbec,https://vinellu.com/w/Wv3T9ngB8S/malbec,malbec argentino,alamos,true,
DV Catena,https://vinellu.com/w/CjHn8hVT3r/d-v-catena-cabernet-sauvignon-malbec,blend argentino,d.v. catena|dv catena cabernet malbec,true,
```

## Errors

Row numbers count the header as row 1. All errors exit with code `2`.

| Message | Cause |
|---|---|
| `catalog.csv is not UTF-8: ...` | The file has another encoding. |
| `catalog.csv header: required columns: name, url` | `name` or `url` column missing. |
| `catalog.csv row N name: must be 1 to 120 chars` | Empty or long name. |
| `catalog.csv row N url: not an absolute http(s) URL: <url>` | Bad URL. |
| `catalog.csv row N url: duplicate url: <url>` | Same URL twice. |
| `catalog.csv row N third_party: expected true or false, got <value>` | Bad flag. |
| `catalog.csv row N notes: at most 500 chars` | Long notes. |
| `catalog.csv row N : more than 5000 rows` | Too many rows. |

See [The catalog](../guides/catalog.md) for how to write one.
