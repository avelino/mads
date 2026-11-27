# Campaign strategies

A strategy tells mads which products to push and which searches to buy for them, so you can see why an account aims at sales instead of at whatever the agent finds most likely to convert.

Without a strategy the plan agent spreads the budget by expected return. With one, part of the budget is reserved for the products you care about, and the ad groups for those products buy the kind of search you chose. To pick one and run it, see [pick a campaign strategy](../how-to/pick-a-strategy.md).

## What a strategy decides

A strategy makes three decisions.

**Which products get the money.** The product pages come from the operator, from the strategy file, or from the init agent when only a count is given. Whatever the source, the resulting `[strategy]` table in `business.toml` holds concrete page URLs, never a count. Each URL has to match a catalog page, or the draft cannot finish (`E32`).

**Which intent they buy.** There are three intents.

- `purchase` buys people ready to buy. Keywords carry buying words and information searches (what is, how to use, review, recipe) become negatives.
- `consideration` buys people comparing. Comparison and review searches stay.
- `discovery` buys people who do not know the product yet. Category and problem searches are allowed.

The intent steers the campaign agent through its prompt. It is not a field the validator checks. A business that does not sell what people search for (an app, a guide, a review site) keeps purchase words as negatives, so `purchase` acts as `consideration` there.

**How much of the budget they get.** `min_share` is the percent of the non-brand budget that must go to product campaigns. This one is enforced in code.

## Why the share is a floor

Agents drift. Asked to push three wines, a plan agent can still end up spending most of the budget on generic searches because those look safe. A prompt alone cannot stop that, so the share is a hard check.

The check sums the daily budgets of the campaigns that count. Brand campaigns and App campaigns are left out of the base, because the first is defensive and the second is bought on a different model. A campaign is a product campaign only when every planned ad group in it holds strategy products. A group holds a product when its `entity_ids` are all products. When it has none, it holds a product when its landing page is a product page. One stray generic group takes the whole campaign out of the count.

If the product campaigns fall below `min_share`, validation fails with `E33` and the run cannot be marked ready to import. The message names the products, so the plan agent knows where to move budget. The exact math is in the [validation rules](../reference/validation-rules.md).

## The shipped strategies

mads ships three starting points. Each one is a header plus a short brief for the agents.

- `visibility` aims at the items people search for the most. It ranks items by estimated demand and also buys evaluation and comparison searches, so it uses `consideration`.
- `profitability` aims at the best sellers. It takes them from the products you give, or from the site (badges, review counts) and the description, and says in the research notes which one it used. It uses `purchase`.
- `need` aims at items that sell below target. It needs the products to be listed, because there is nothing to pick them from. It uses `purchase` and the highest share.

The defaults for `top`, `min_share` and the keys of a custom strategy file are in the [business.toml reference](../reference/business-toml.md#strategy). A custom file can extend a shipped strategy and override any key, or stand alone.

## Strategy and focus

Focus and strategy both take URLs, so they look alike. They do different jobs.

Focus is scope. It says the account may only talk about this offer, and code enforces it by refusing URLs and keywords outside it (`E22`, `E23`). Strategy is priority. It says these products get most of the money, and the rest of the catalog may keep a small share.

Think of focus as the wall and strategy as how you spend inside it. A route campaign with a focus and `purchase` intent buys only that route, and only people ready to buy the ticket.

They combine. Every strategy product must then be one of the focus pages. When no products are given, the focus pages are the products. See [draft from your site](../how-to/draft-from-your-site.md) for how focus is set.

## Restricted categories

Google refuses sale words in a restricted category, and a refused ad gets no impressions. When init decides the business is restricted and the strategy asks for `purchase`, it lowers the intent to `consideration`. Products and `min_share` stay. `STRATEGY.md` records the change on its first line with the reason, and init prints the same line.

A `business.toml` edited by hand with `purchase` and a restricted list is refused when it loads. The reasoning behind the restricted rules is in [restricted categories](restricted-categories.md).

## Demand is an estimate

mads has no search volume data. The agent estimates demand from the web, and the research notes say so. The estimate only orders the list. Sales depend on the intent of the search and on the product shown, and neither needs exact volume.

The check comes after launch. [`mads optimize`](how-optimize-decides.md) reads the search terms report and judges the plan by what the account returned. In that mode the share rule is a warning (`W14`) instead of an error. Live performance wins over the strategy, so the plan is not forced back to a share the data does not support. `mads optimize` keeps the strategy of the run it starts from.

## Where it shows up

`report.md` gets a `## Strategy` section with the name, the intent, the product pages and the share they got. See the [report reference](../reference/report-md.md).

A strategy is copied into `STRATEGY.md` next to `business.toml` when init writes it. Upgrading mads does not change an existing account, because the text lives in your files.

## Next

- [Pick a campaign strategy](../how-to/pick-a-strategy.md)
- [Restricted categories](restricted-categories.md)
- [How optimize decides](how-optimize-decides.md)
