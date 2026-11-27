# Pick a campaign strategy

Reserve a share of the budget for the products you want to sell, and choose which kind of search buys them.

Without a strategy the plan agent spreads the budget by expected return. With one, `mads init` writes a `[strategy]` table and mads refuses a plan that gives the products less than `min_share` percent. To understand why, read [Campaign strategies](../explanation/strategies.md).

## Before you start

- mads is installed. See [Install mads](install.md).
- You know the site mads will read. A strategy is set when you run `mads init --from-url`. See [Draft the files from your site](draft-from-your-site.md).
- `mads init` calls a model. It spends the tokens of the provider you chose. See [Choose a provider](choose-a-provider.md).
- Every product page is on the same site as `--from-url`.

## Pick a strategy

Choose by what you want the account to do.

| You want | Use | `intent` | `min_share` | Products |
|---|---|---|---|---|
| Reach the items people search for the most | `visibility` | `consideration` | 60 | The agent picks 10 by estimated demand. |
| Sell the best sellers | `profitability` | `purchase` | 70 | The agent picks 5, or you give them. |
| Push items that sell below target | `need` | `purchase` | 80 | You list them. |

`intent` is the kind of search the ad groups buy.

- `purchase` buys people ready to buy.
- `consideration` buys people comparing.
- `discovery` buys people who do not know the product yet. No shipped strategy uses it. Set it in your own file.

A shipped strategy is a starting point. If none fits, [write your own](#write-your-own-strategy-file).

## Run init with the strategy

1. Add `--strategy` to your init command. Give the products yourself, or let the agent pick them.

   ```bash
   mads init --from-url https://loja.com --daily-budget 100 --currency BRL \
     --strategy need \
     --product https://loja.com/vinho-x --product https://loja.com/vinho-y
   ```

   ```bash
   mads init --from-url https://loja.com --daily-budget 100 --currency BRL \
     --strategy profitability --top 5
   ```

   - `--product <URL>` is repeatable and replaces the products of the strategy.
   - `--top <n>` replaces the `top` of the strategy. It is how many products the agent picks when you give none.
   - `--product` and `--top` need `--strategy`. Without it init exits `2`.
   - `need` refuses to run with no products, and a `top` does not replace them. With `--focus`, the focus pages are the products.
   - A `--strategy` value that contains `/` or ends in `.md` is read as a file. Any other value must be `visibility`, `profitability` or `need`, or init exits `2`.

   The [CLI reference](../reference/cli.md) lists every flag.

2. Read what init printed. If the business is in a restricted category and the strategy asks for `purchase`, init lowers the intent to `consideration` and prints `Intent lowered from purchase to consideration: ...` with the reason. See [Handle restricted categories](handle-restricted-categories.md).

3. Review `business.toml`, `catalog.csv` and `STRATEGY.md` before you generate. Product URLs must match a catalog page. A leading `www.` and a trailing `/` make no difference. The query string does, so `?id=1` and `?id=2` are two products. A product with no catalog item fails init with `E32`.

## Use it with a focus

`--focus` limits the account to one offer. `--strategy` decides where the money goes inside it. You can use both.

- Every product must be one of the focus pages.
- With no `--product`, the focus pages are the products and `--top` is ignored.

The checks differ. A URL or keyword outside the focus fails with `E22` or `E23`. A product share below `min_share` fails with `E33`.

## Write your own strategy file

Use a file when a shipped strategy is close but not right. It is a TOML header between `+++` lines, then prose for the agents.

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

Save it as `wines.md` and pass the path.

```bash
mads init --from-url https://loja.com --daily-budget 100 --currency BRL \
  --strategy ./wines.md
```

| Key | Rule |
|---|---|
| `extends` | `visibility`, `profitability` or `need`. This file overrides its keys. The prose of the shipped strategy comes first, then yours. Another file is refused. |
| `name` | Shown in the report. Defaults to the file name without `.md`. 1 to 80 characters. |
| `intent` | `purchase`, `consideration` or `discovery`. |
| `products` | 1 to 50 page URLs. |
| `top` | 1 to 50. |
| `min_share` | 0 to 100. |

Without `extends`, `intent` and `min_share` are required. So is one of `products` or `top`, unless you run init with `--focus`. The prose is capped at 8000 characters. An unknown header key is refused. The keys that end up in `business.toml` are in the [business.toml reference](../reference/business-toml.md#strategy).

## Check it worked

1. Open `business.toml`. It has a `[strategy]` table with `name`, `intent`, `min_share` and a concrete list of `products`. It never holds a count.

   ```toml
   [strategy]
   name = "need"
   intent = "purchase"
   products = ["https://vinellu.com/w/alamos-malbec"]
   min_share = 80
   file = "STRATEGY.md"
   ```

   This is the shape from the [business.toml reference](../reference/business-toml.md#strategy), not a run of your site.

2. Open `STRATEGY.md` next to it. It holds the text the agents follow. When init lowered the intent, its first line says so.

3. Generate, then open `report.md`. See [Review a run](review-a-run.md). The `## Strategy` section lists the name, the intent, the products and one line.

   ```text
   Product campaigns got <n>% of the non-brand budget (asked for at least <min_share>%).
   ```

   If the share is under `min_share`, validation fails with `E33` and the run is not ready to import. The message names the products, so you know where to move budget.

## Change the strategy later

Edit `[strategy]` in `business.toml` and `STRATEGY.md`, then run `mads generate` again.

```bash
mads generate business.toml --provider <provider>
```

`STRATEGY.md` is a copy, so upgrading mads does not change an existing account. `mads optimize` keeps the strategy of the run it starts from. There the share rule is warning `W14` instead of `E33`, so live results can override it. See [Optimize a live account](optimize-a-live-account.md).

## Next

- [Draft the files from your site](draft-from-your-site.md)
- [Handle restricted categories](handle-restricted-categories.md)
- [Campaign strategies](../explanation/strategies.md)
