# The catalog

This page shows how to write `catalog.csv` so mads builds one tight ad group per thing people search for by name.

## When you need one

A catalog is a list of entities that have their own search demand. Wine labels, products, courses, hotels, software integrations, local branches.

- With a catalog, the plan mission creates a `catalog` campaign and gives each entity (or a tight group of entities) its own ad group. Each ad group lands on that entity's page.
- Without a catalog, mads has no entities to name. It builds brand and generic campaigns from your description.

The catalog is optional. Skip it when your business has no list of named things.

`mads init --from-url` can draft a catalog from your sitemap. See [Init from a URL](init-from-url.md).

## Format

A header row, then one row per entity.

```csv
name,url,category,aliases,third_party,notes
Alamos Malbec,https://vinellu.com/w/Wv3T9ngB8S/malbec,malbec argentino,alamos,true,
DV Catena,https://vinellu.com/w/CjHn8hVT3r/d-v-catena-cabernet-sauvignon-malbec,blend argentino,d.v. catena|dv catena cabernet malbec,true,
```

Only `name` and `url` are required. Column names are case-insensitive and the order does not matter. Extra columns are ignored. The file is UTF-8, with or without a BOM, and LF or CRLF line endings both work.

| column | what to put |
|---|---|
| `name` | The name people search for. 1 to 120 characters. |
| `url` | The page for this entity. Absolute `http` or `https`. Unique in the file. |
| `category` | A group name. Helps the plan mission split ad groups. |
| `aliases` | Other spellings, separated by `\|`. Used as keyword variants. |
| `third_party` | `true` when the name is someone else's trademark. Default `false`. |
| `notes` | Facts for the agent, at most 500 characters. |

## Ids

mads gives each row an id with `slugify(name)`. It strips accents, lowercases, turns every run of characters outside `a-z` and `0-9` into one `-` and trims `-` at both ends.

| name | id |
|---|---|
| `Alamos Malbec` | `alamos-malbec` |
| `Angélica Zapata` | `angelica-zapata` |
| `D.V. Catena` | `d-v-catena` |

Duplicate ids get a numeric suffix in file order. A second `Alamos Malbec` is `alamos-malbec-2`. The plan mission uses ids in `entity_ids`.

## Aliases become keyword variants

The campaign agent turns the name and its aliases into variants, and mads crosses variants with intent modifiers. List the ways people really type the name.

- With and without accents (`angélica zapata`, `angelica zapata`).
- Short forms (`alamos`).
- Punctuation variants (`d.v. catena`, `dv catena`).

Do not list words that change the meaning.

## third_party

Set `third_party = true` for names that belong to someone else, such as a wine producer when you review or sell their wine. mads then raises warning `W01` when that name or an alias shows up in ad text. The warning is a prompt to check Google's trademark policy for your case. It does not block the export.

Competitors from `business.competitors` raise `W01` too.

## notes

Use `notes` for facts the agent may state. Prices, awards, a unique feature. The agent sees the full row of every entity in its campaign.

```csv
name,url,category,aliases,third_party,notes
Casa Concha,https://example.com/p/casa-concha,tinto,,false,Vencedor de medalha de ouro em 2025
```

The agent is told to take claims only from your description and notes. Write only what you can back up.

## image

A URL of a real photo of the item. Image campaigns send it to the image model as a reference, so the picture shows the real product and not an invented one. Without it, briefs for that item show people and places, not the product.

Use a photo of the item itself. A site often serves the same share card, with its logo, for every page: that is not a product photo, leave the column empty. See [Image campaigns](image-campaigns.md#real-product-photos).

## Limits

- At most 5000 rows.
- `url` must be unique, compared after normalization (lowercase scheme and host, default port and fragment removed).
- The account has an ad group limit. `--max-ad-groups` defaults to 50. A catalog with 400 labels will not get 400 ad groups. The plan mission groups entities.

## Errors

Row numbers count the header as row 1.

```text
error: catalog.csv row 2 url: not an absolute http(s) URL: not-a-url
error: catalog.csv header: required columns: name, url
error: catalog.csv row 3 url: duplicate url: https://x.com/a
error: catalog.csv row 2 third_party: expected true or false, got yes
```

All exit with code 2. The full column reference is in [catalog.csv reference](../reference/catalog-csv.md).
