# catalog.csv

Check a `catalog.csv` before `mads generate` reads it, with every column, limit and parse error listed.

`business.toml` points to the file with `[catalog] file`, relative to the folder that holds `business.toml`. See [business.toml](business-toml.md). To decide what to put in each column, see [Write a catalog](../how-to/write-a-catalog.md).

## File rules

| Rule | Value |
|---|---|
| Header row | Required. It is row 1. |
| Encoding | UTF-8. A leading BOM is accepted. |
| Line endings | LF or CRLF. |
| Rows | At most 5000 after the header. |
| Header names | Case-insensitive. Column order does not matter. |
| Unknown columns | Ignored. |
| Fields | Trimmed. Quote a field that contains a comma. |
| Short rows | Allowed. A missing optional field reads as empty. |

## Columns

| Column | Required | Rule |
|---|---|---|
| `name` | yes | 1 to 120 characters, counted as characters. |
| `url` | yes | Absolute `http` or `https` URL with a host. Unique in the file. |
| `category` | no | Free text. `query_catalog` filters on it, exact and case-insensitive. |
| `aliases` | no | Names separated by `\|`. Each part is trimmed and empty parts are dropped. |
| `third_party` | no | `true` or `false`, case-insensitive. Empty means `false`. |
| `notes` | no | Free text, at most 500 characters. |
| `image` | no | Absolute `http` or `https` URL. Empty means no photo. |

### url

Two rows clash when their URLs are the same after the URL is parsed and the fragment is removed. `https://x.com/a#top` and `https://x.com/a` are the same URL. `https://x.com/a` and `https://x.com/a/` are different.

### third_party

`true` adds the item name and every alias to the third-party terms. Ad text that contains one of them raises warning `W01`. The warning does not block the export. See [Validation rules](validation-rules.md).

### image

Image campaigns send the URL to the image model as a reference. `mads init` fills the column from the `og:image` of the item page. See [Add image and app campaigns](../how-to/add-image-campaigns.md).

## Ids

Every row gets an id from `name`. Plans refer to items by id in `entity_ids`.

1. Normalize to NFD and drop combining marks.
2. Lowercase ASCII letters.
3. Turn every run of characters outside `a-z` and `0-9` into one `-`.
4. Trim `-` at both ends.

A repeated id gets a number in file order. The first row keeps the plain id, the second gets `-2`, the third `-3`.

| `name` | id |
|---|---|
| `Angélica Zapata` | `angelica-zapata` |
| `D.V. Catena` | `d-v-catena` |
| `Alamos Malbec` | `alamos-malbec` |
| `Alamos Malbec` (second row) | `alamos-malbec-2` |

## Example

```csv
name,url,category,aliases,third_party,notes,image
Alamos Malbec,https://vinellu.com/w/Wv3T9ngB8S/malbec,malbec argentino,alamos,true,,
DV Catena,https://vinellu.com/w/CjHn8hVT3r/d-v-catena-cabernet-sauvignon-malbec,blend argentino,d.v. catena|dv catena cabernet malbec,true,,
```

## Errors

Row numbers count the header as row 1. The first row that fails stops the parse. `mads generate` prints the message and exits with code `2`. See [Exit codes and errors](exit-codes-and-errors.md).

| Message | Cause |
|---|---|
| `catalog.csv is not UTF-8: <detail>` | The file has another encoding. |
| `catalog.csv header: required columns: name, url` | The header has no `name` or no `url` column. |
| `catalog.csv header: <detail>` | The CSV reader could not read the header. |
| `catalog.csv row N: <detail>` | The CSV reader could not read the row. |
| `catalog.csv row N name: must be 1 to 120 chars` | The name is empty or longer than 120 characters. |
| `catalog.csv row N url: not an absolute http(s) URL: <url>` | The URL is relative, has another scheme or has no host. |
| `catalog.csv row N url: duplicate url: <url>` | An earlier row has the same URL. |
| `catalog.csv row N third_party: expected true or false, got <value>` | The value is not empty, `true` or `false`. `<value>` is lowercased. |
| `catalog.csv row N notes: at most 500 chars` | The notes are longer than 500 characters. |
| `catalog.csv row N image: not an absolute http(s) URL: <url>` | The image URL is relative, has another scheme or has no host. |
| `catalog.csv row N : more than 5000 rows` | The file has a 5001st data row. N is 5002. |

A missing file fails before parsing with `<path>: <system error>`.

## Next

- [Write a catalog](../how-to/write-a-catalog.md)
- [business.toml](business-toml.md)
- [Validation rules](validation-rules.md)
