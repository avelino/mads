# Restricted categories

Understand why mads asks about Google's restricted content policy for every business, and what your answer changes in a run.

## Why it matters

Google Ads puts some subjects under extra review. Keywords and ads about them follow country rules and can be refused at upload. The refusal does not depend on what the business sells. An app that reviews wines still advertises wine.

Google often reads product names, not intent. A keyword that only informs can be refused, and a similar name can pass. In Google Ads Editor the refusal shows an error such as `Alcohol sale` after you post.

You cannot predict which items fail from their text. So mads does three things. It records the decision in `business.toml`, it keeps sale language out of what the agents write, and it gives you the text to ask Google for an exception.

## What the six values stand for

The `restricted` key takes a list of categories. Each one maps to a policy in Google Ads help, and the report names that policy.

- `alcohol` maps to Alcohol. It applies whether the business sells, reviews, recommends or serves the drinks.
- `gambling` maps to Gambling and games.
- `healthcare` maps to Healthcare and medicines.
- `financial_services` maps to Financial services.
- `political` maps to Political content.
- `sexual_content` maps to Sexual content.

Any other value fails to load. The exact format is in the [business.toml reference](../reference/business-toml.md).

## How init decides

`mads init` makes the decision for every business, with no flag. The agent must pass `restricted` when it writes the business. A call without the key is refused, so the choice is never skipped.

The agent judges by what the ads would talk about, not only by what the business sells. It names each category it sets in the research summary, with the page that shows it.

The key is written even when the list is empty. That tells "checked, none applies" apart from "never checked". A `business.toml` written by hand without the key loads as an empty list.

At the end init prints the result, either the policies it set or a reminder to check the key. Read it. If the agent missed a category or added a wrong one, edit the list. See [Handle restricted categories](../how-to/handle-restricted-categories.md).

## What changes in a run

With at least one category listed, `mads generate` changes five things.

- **Prompts for text.** The plan, campaign and image campaign agents get a rule. No sale or transaction words for the restricted subject in keywords, ad texts or sitelinks, in the business language too. Searches and texts are framed as information, reviews, comparison or community.
- **Prompts per category.** For `alcohol`, no promise of effects, no excess, nothing aimed at minors. For `gambling`, `financial_services` and `healthcare`, no promise of gains, results or cures, and no urgency.
- **Prompts for pictures.** Adults only, moderate and responsible scenes, nothing that ties the subject to success, health or seduction.
- **Purchase intent is lowered.** Purchase asks for buying words, which the policy forbids. When init writes a strategy with `intent = "purchase"` under a restricted category, it lowers it to `consideration` and says so. A `business.toml` that has both is refused at load. See [Campaign strategies](strategies.md).
- **The report.** `report.md` gets a `Restricted categories` section that names the policies and holds the exception text to paste. See the [report.md reference](../reference/report-md.md).

The first three are prompt rules. They lower the number of refusals and cannot remove them. The last two are enforced by code.

## What mads cannot do

mads does not call Google and does not see its review. Refusals show up only when you post in Google Ads Editor. If the business does not do what the policy restricts, you ask for an exception with the text from the report. If it does, an exception does not apply.

## Next

- [Handle restricted categories](../how-to/handle-restricted-categories.md)
- [Campaign strategies](strategies.md)
