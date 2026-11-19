## Strategy

`get_business` has `strategy`: the advertiser chose how the account is run. `strategy.product_ids` are the catalog entities it pushes.

- Put the products in campaigns that only hold the strategy products: every planned ad group of the campaign covers only `product_ids`.
- Those campaigns get at least `min_share` percent of the daily budget left after the Brand and App campaigns: Brand and App campaigns are outside that share. A plan below it is refused (`E33`).
- An image campaign counts toward the share only when every one of its asset groups holds strategy products. Otherwise its default 20 percent comes out of what is left for the other campaigns.
- In `mads optimize` live results win: below the share is a warning (`W14`), not an error.
- Follow the strategy prose.
