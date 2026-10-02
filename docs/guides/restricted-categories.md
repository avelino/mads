# Restricted categories

This page explains how mads handles businesses that Google Ads reviews under its restricted content policy, such as alcohol or gambling, and what to do when Google refuses a keyword or an ad.

## Why it matters

Google Ads restricts some subjects. Keywords and ads about them go through extra review, follow country rules, and can be refused at upload. The refusal does not depend on what the business sells: an app that reviews wines still advertises wine. In the first real Vinellu upload, Google refused 23 of 223 keywords with this error, all of them wine names:

```text
Alcohol sale: Your creative promotes the online sale of alcohol. : 'tres medallas vinho'
```

Vinellu does not sell wine. The keywords were for reviews. Google refused them anyway, and some names passed while others with the same pattern did not. You cannot predict it from the text, so mads writes the decision down, keeps sales language out, and tells you how to ask for an exception.

## The categories

`restricted` in `business.toml` takes these values. Each maps to a policy in Google Ads help.

| Value | Google Ads policy | Covers |
|---|---|---|
| `alcohol` | Alcohol | Alcoholic drinks, whether the business sells, reviews, recommends or serves them. |
| `gambling` | Gambling and games | Betting, casinos, lotteries, sweepstakes, games played for money. |
| `healthcare` | Healthcare and medicines | Medicines, pharmacies, supplements with health claims, treatments, clinics, telemedicine. |
| `financial_services` | Financial services | Loans, credit, cards, investing, crypto assets, insurance, debt services. |
| `political` | Political content | Candidates, parties, elections, political issues. |
| `sexual_content` | Sexual content | Sexual products or services, dating with a sexual focus. |

Any other value fails to load with a parse error.

## How init sets it

`mads init` sets `restricted` for every business, with no flag. The agent must give the key in `write_business`: a call without it is refused, so the choice is always made. It judges by what the ads would talk about, not only by what the business sells, and cites the page that shows each category in `research.md`.

The result is written even when empty, so you can tell "checked, none applies" from "never checked":

```toml
[business]
name = "Vinellu"
restricted = ["alcohol"]
```

```toml
restricted = []
```

At the end init prints the decision:

```text
Restricted categories: Alcohol. Google may refuse some keywords: the report explains how to ask for an exception
```

Read it. If the agent missed a category or added a wrong one, edit the list in `business.toml`. An older file without the key loads as an empty list.

## What changes in a run

With at least one category listed, `mads generate` changes three things.

- **Keywords, ads and sitelinks.** The plan, campaign and image campaign agents get a rule: no sale or transaction words for the restricted subject (buy, price, cheap, deal, discount, shop, store, order, delivery, free shipping, and the same in the business language). Searches and texts are framed as information, reviews, comparison or community. For `alcohol`, no promise of effects, no excess, nothing aimed at minors. For `gambling`, `financial_services` and `healthcare`, no promise of gains, results or cures, and no urgency.
- **Pictures.** Adults only, moderate and responsible scenes, nothing that ties the subject to success, health or seduction.
- **report.md.** A `Restricted categories` section names the policies, says what to expect at upload, and gives the exception text to paste.

These rules are in the prompts. They lower the number of refusals but cannot remove them: Google refuses some product names whatever the words around them.

## When Google refuses a keyword or an ad

Refusals show up when you post from Google Ads Editor. The publish summary counts them under Errors, and the items stay marked with a red icon.

1. In Editor, open Keywords (or Ads) and filter by errors.
2. Click a refused item and read the message at the bottom, such as `Alcohol sale`.
3. If the business does not do what the policy restricts (for example, it does not sell alcohol online), select all the refused items, tick Request exception, and paste the text from the `Restricted categories` section of `report.md`. It looks like this:

   ```text
   Vinellu (https://vinellu.com): App social de vinhos com reviews, safras e harmonização. Our ads and keywords give information, reviews and comparisons about products in the Alcohol category. We do not sell these products online. Please review them under the Alcohol policy.
   ```

4. Post again. Google reviews the request in a few business days.
5. If the business does sell the restricted product, an exception does not apply. Check the policy for your country in Google Ads help, or remove the refused items. The rest of the campaign posts normally.

## See also

- [business.toml reference](../reference/business-toml.md#business)
- [Init from a URL](init-from-url.md#restricted-categories)
- [Review and import](review-and-import.md)
