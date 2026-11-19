# Campaign strategies

A strategy tells mads which products to push and which searches to buy for them, so you can aim an account at sales instead of at whatever the agent finds most likely to convert.

Without a strategy the plan agent spreads the budget by expected return. With one, a share of the budget is reserved for the products you care about, and the ad groups for those products buy the kind of search you chose.

## What a strategy sets

- **Which products get the money.** The product pages come from `--product`, from the strategy file, or from the init agent when you give a count.
- **Which intent they buy.** `purchase` buys people ready to buy. `consideration` buys people comparing. `discovery` buys people who do not know the product yet. With `purchase`, the information searches (what is, how to use, review, recipe) become negatives. `consideration` keeps comparison and review searches, and `discovery` allows category and problem searches. A business that does not sell what people search for (an app, a guide, a review site) keeps purchase words as negatives, so `purchase` acts as `consideration` there.
- **How much of the budget they get.** `min_share` is the percent of the non-brand budget that must go to campaigns that hold only strategy products. A planned ad group holds a product when its `entity_ids` are all products, or when it has none and its landing page is a product page. mads checks it with `E33`.

The App campaigns and the Brand campaigns are left out of that base. See [validation rules](../reference/validation-rules.md) for the exact math.

## Shipped strategies

| Strategy | Products | `intent` | `top` | `min_share` | What it asks for |
|---|---|---|---|---|---|
| `visibility` | The items people search for the most | `consideration` | 10 | 60 | Rank items by estimated demand from the web. Also buy evaluation and comparison searches. |
| `profitability` | The best sellers | `purchase` | 5 | 70 | Take the best sellers from `--product`, or from the site (badges, review counts) and the description. The research notes say which. |
| `need` | The items you list | `purchase` | none | 80 | These items sell below target. Show what makes each worth buying. Needs `--product`, or `--focus`. |

`top` is how many products the init agent picks when you give none. `need` lists its products: it has no `top`, and a `top` you add does not stand in for them, so it refuses to run without products. With `--focus` the focus pages are the products.

## Run it

```bash
mads init --from-url https://loja.com --daily-budget 100 --currency BRL \
  --strategy need \
  --product https://loja.com/vinho-x --product https://loja.com/vinho-y

mads init --from-url https://loja.com --daily-budget 100 --currency BRL \
  --strategy profitability --top 5
```

- `--strategy` takes `visibility`, `profitability`, `need`, or a path to a `.md` file. A value with a `/` or ending in `.md` is read as a file.
- `--product <URL>` is repeatable and replaces the products of the strategy. Every product must be on the same site as `--from-url`.
- `--top <n>` replaces the strategy's `top`.
- `--product` and `--top` need `--strategy`.

The [CLI reference](../reference/cli.md) lists every flag.

Product URLs match catalog pages without a leading `www.` and without a trailing slash at the end. The query string counts, so `?id=1` and `?id=2` are two products. If a product page has no catalog item, `finish` fails with `E32`.

init writes a `[strategy]` table in `business.toml` and the text in `STRATEGY.md`, next to it. `products` is always concrete there, never a count. Edit the file and run `mads generate` to change the strategy. `STRATEGY.md` is a copy, so upgrading mads does not change an existing account. `mads optimize` keeps the strategy of the run it starts from.

## Write your own

A strategy file is a TOML header between `+++` lines, then prose for the agents.

```markdown
+++
extends = "need"
name = "Wines stuck in stock"
intent = "purchase"
products = ["https://loja.com/vinho-x", "https://loja.com/vinho-y"]
min_share = 80
+++

Only people who already decided to buy. Negate pairing, "what is" and recipes.
High ticket: a higher CPC is fine on searches with "buy" and "price".
```

| Key | Meaning |
|---|---|
| `extends` | A shipped strategy. Its header is the base and this file overrides it key by key. Its prose comes first and this file's prose after. It names a shipped one only, never another file. |
| `name` | Shown in the report. Defaults to the file name. |
| `intent` | `purchase`, `consideration` or `discovery`. |
| `products` | 1 to 50 product page URLs. |
| `top` | 1 to 50. How many products the init agent picks when `products` is empty. |
| `min_share` | 0 to 100. Percent of the non-brand budget for product campaigns. |

Without `extends`, `intent` and `min_share` are required, and so is one of `products` or `top`, unless you run init with `--focus`. The prose is capped at 8000 characters. The keys of the resulting `[strategy]` table are in the [business.toml reference](../reference/business-toml.md#strategy).

## Strategy and focus

[`--focus`](init-from-url.md) and `--strategy` both take URLs, so they look alike. They do different jobs, and you can use both.

| | `--focus` | `--strategy` |
|---|---|---|
| Job | Scope: the account may only talk about this offer. | Priority: these products get most of the money. |
| Enforced by | `E22` (URL outside the focus) and `E23` (keyword without the focus terms). | `E33` (minimum budget share). |
| Rest of the catalog | Not there. | May keep a small share. |
| Audience | Not set. | `intent`. |
| Locations | Narrowed to the offer. | Unchanged. |

The focus is the wall. The strategy spends inside it. A route with `--focus --strategy need` and `intent = "purchase"` buys only that route, and only people ready to buy the ticket.

With both:

- Every product must be one of the focus pages.
- Without `--product`, the focus pages are the products and `--top` is ignored. A planned group without `entity_ids` lands on the first focus URL, so it counts as a product group when that URL is a product. `E33` still checks the share. The strategy adds its `intent` and its text.

## Restricted categories

Google refuses sale words in a restricted category, and a refused ad has no impressions. When init decides the business is restricted and the strategy has `intent = "purchase"`, it lowers the intent to `consideration`. Products and `min_share` stay.

`STRATEGY.md` then starts with the line `Intent lowered from purchase to consideration: ...` with the reason, and init prints the same line. The report shows the intent the plan used. `consideration` buys the closest searches Google allows, such as "best red wine under 100" or "is wine x worth it". See [restricted categories](restricted-categories.md).

A hand-edited `business.toml` with `intent = "purchase"` and a `restricted` list is refused when it loads.

## Demand is an estimate

mads has no search volume data. The agent estimates demand from the web, and the research notes say so. The estimate only orders the list. Sales depend on the intent of the search and on the product shown, and neither needs exact volume.

The check comes later. [`mads optimize`](optimize.md) reads the search terms report with conversions, and it judges the plan by what the account returned. There the share rule is a warning, `W14`, not an error: live performance wins over the strategy, so a product campaign that spends without converting can still be cut.

## In the report

`report.md` gets a `## Strategy` section with the name, the intent, the product pages and the share they got:

```text
Product campaigns got 80% of the non-brand budget (asked for at least 80%).
```
