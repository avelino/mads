## Optimizing a live account

`get_business` has `live_account` and `performance`: this account already runs in Google Ads. Plan its next version, not a new account.

Continuity:

- Keep the name of every campaign that continues and of its ad groups. Google Ads Editor matches by name, so a new name is a new group, and its keywords start without the Quality Score data the old ones had. To drop or rename a group that had impressions, add a line `Drop <group>: <reason>` to the campaign `rationale` (`E25` otherwise).
- A campaign you leave out is paused at import, and one you keep comes enabled even if it is paused in Google Ads today. Keep a campaign only if it is worth spending on for the return, and leave out one that is not, for a reason in `performance`.
- `ignored_campaigns` are campaigns of the account that this run does not manage. Do not plan them.
- Split an ad group that mixes themes, merge ones that share the same searches.
- Start each `rationale` with the numbers behind it: days, cost, clicks, conversions.

The shares above (10 to 20 percent for `brand`, at least 20 percent for an image campaign) are where a new account starts. On a live account the numbers beat them, and a `budget_cap` beats everything: a campaign with one never gets more, even if its intent suggests a bigger share. Split the budget starting from the caps.

Decide the budget of each campaign with the first line that fits. `live_rules` explains each flag:

1. `stalled`: broken, not short of data. Fix what Google says (no ads, policy) or cut its budget. It never gets more than it ran with (`E27`).
2. `untracked`: clicks and no conversion while others convert. At most its `avg_daily_cost`, rounded up (`E27`), and no new keywords or groups.
3. `thin` (see `thin_rule`): its numbers are noise. It keeps its budget, and you fix structure only, unless it is broken: no impressions because of bids under the first page, no ads, or a policy limit.
4. `budget_use_pct` under 100: the budget is not what limits it. Cut it to the real spend, which changes nothing it gets.
5. `lost_to_budget_pct` high and it converts: give it more, in steps of about 20 percent per run, taken from cuts and from `idle_budget`, the money the account leaves unspent every day.
6. `lost_to_rank_pct` high: it needs better bids or quality, not more budget.

Move budget toward campaigns that bring customers at a lower cost and away from campaigns that spend without results.
