# Write a catalog

Write `catalog.csv` so mads builds one tight ad group for each thing people search for by name.

## Before you start

- A list of things people search for by name: products, wine labels, courses, hotels, branches. If your business has no such list, skip the catalog. Without one, mads builds brand and generic campaigns from your description and makes no `catalog` campaign.
- A page on your site for each item. The agent lands each ad group on that item's page.
- A `business.toml` with a `[catalog]` table that points at the file. See [Write the business file](write-the-business-file.md) and [business.toml](../reference/business-toml.md).
- Optional shortcut: `mads init --from-url` drafts the catalog from your sitemap. See [Draft the files from your site](draft-from-your-site.md) and then fix the draft with the steps below.

## Steps

1. Create `catalog.csv` next to `business.toml`. Start with the header and one row per item. Only `name` and `url` are required.

   Example from the test fixture of the replay run (no model was called):

   ```csv
   name,url,category,aliases,third_party
   Alamos Malbec,https://vinellu.com/w/Wv3T9ngB8S/malbec,malbec,alamos,true
   DV Catena Cabernet Malbec,https://vinellu.com/w/CjHn8hVT3r/d-v-catena-cabernet-sauvignon-malbec,blend,dv catena|d.v. catena,true
   ```

   The other columns are `notes` and `image`. Leave them out of the header when you do not use them.

   Column names are case-insensitive and their order does not matter. Every rule for each column is in the [catalog.csv reference](../reference/catalog-csv.md).

2. Put the name people type in `name`, without marketing words. The name is 1 to 120 characters. Put the item's own page in `url`. Every `url` must be unique in the file.

3. Put the other ways people write the name in `aliases`, separated by `|`. Cover the spellings people really type.

   - With and without accents (`angélica zapata`, `angelica zapata`).
   - Short forms (`alamos`).
   - Punctuation variants (`d.v. catena`, `dv catena`).

   Do not add words that change the meaning. The campaign agent is told the same thing for keyword variants, and it refuses variants that look like different searches (`E26`). See [Validation rules](../reference/validation-rules.md).

4. Group related items with `category`. The plan agent browses categories first, then pages of items, and uses them to decide which items share an ad group. Keep the names short and few.

5. Set `third_party` to `true` for names that belong to someone else, for example a producer whose wine you sell or review. Leave it empty or `false` for your own names.

   mads then adds the name and every alias of that row to its list of third-party terms, together with your `competitors`. Ad text that contains one of them raises warning `W01`. The warning does not block the export. Check Google's trademark policy for your case before you ship that text. The campaign agent is also told not to put a third-party trademark in ad text unless the item is the thing being reviewed or sold.

6. Write facts the ads may state in `notes`, at most 500 characters. Prices, awards, one unique feature.

   ```csv
   name,url,category,aliases,third_party,notes
   Casa Concha,https://example.com/p/casa-concha,tinto,,false,Gold medal winner in 2025
   ```

   The agents are told to take numbers, prices, awards and claims only from your description and your notes. Write only what you can back up.

7. Put a real photo of the item in `image`, as an absolute `http` or `https` URL. Image campaigns send it to the image model as a reference, so the picture shows your real product. Without it, briefs for that item show people and places, not the product.

   Use a photo of the item itself. Many sites serve the same share card with the logo on every page. That is not a product photo, so leave the column empty. A `reference` to an item without a photo fails with `E19`. See [Add image campaigns](add-image-campaigns.md).

8. Point `business.toml` at the file. The path is relative to the folder that holds `business.toml`.

   ```toml
   [catalog]
   file = "catalog.csv"
   ```

9. Mind the size limits. The file takes at most 5000 rows. The account takes at most `--max-ad-groups` ad groups, 50 by default, so a catalog of 400 items does not get 400 ad groups. The plan agent groups items that share the same searches. See [Limits](../reference/limits.md).

## Check it worked

mads reads the catalog when it reads `business.toml`. A bad file stops the run with exit code `2` and one line that names the row and the column. Row numbers count the header as row 1.

The line has this shape, with the row number and the column of the first bad value:

```text
error: catalog.csv row N url: not an absolute http(s) URL: <url>
```

Fix the row it names and run again. The full list of messages is in the [catalog.csv reference](../reference/catalog-csv.md#errors).

When the file parses, the plan mission browses it with `query_catalog`. This is the plan log of the example run, with 12 items.

Example run (replay provider, no model was called). It used `--skip-url-check`.

```text
10:21:17 [plan] > query_catalog
10:21:17 [plan] < ok query_catalog: 12 of 12 items
```

The plan then makes a `catalog` campaign and gives each entity or tight group its own ad group. Look for it in `report.md`.

Example run (replay provider, no model was called). It used `--skip-url-check`.

```text
| Vinellu - Rotulos | Search | catalog | 30,00 BRL | Manual CPC | 6 | 71 |
```

The third column is the intent. If no row has `catalog` there, the plan skipped it. Check that `[catalog]` is in `business.toml` and that the file has rows.

## Next

- [catalog.csv reference](../reference/catalog-csv.md)
- [Review a run](review-a-run.md)
- [Add image campaigns](add-image-campaigns.md)
