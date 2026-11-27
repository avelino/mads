# How a run works

Predict a run and review its output by knowing what the agents decide, what mads computes and what the validator enforces.

## The shape of a run

A run has these stages.

```text
business.toml + catalog.csv
        |
   plan mission          brand kit + 1 to 5 campaigns + planned ad groups
        |
   campaign missions     one per campaign, in parallel
        |
   after the missions    cross negatives, pictures, validation, URL check, export
```

Each mission has a system prompt, a short user prompt and a fixed set of tools. The agent works only through those tools. Every tool validates its arguments before it writes anything. A failed call changes nothing and returns the errors, so the agent fixes the call and tries again.

The plan mission runs first. If it fails, the run fails, because the campaign missions have nothing to build. A campaign mission that fails does not stop the others. The run then ends with exit code `1` and no CSV. A failed mission gets a fresh conversation on the same workspace, so ad groups it already saved stay. How many attempts it gets is in [Limits and retries](limits-and-retries.md).

Everything after the missions is plain code. No model is called. That is why `mads export` can redo it on a finished run.

## Who decides what

The agents decide the judgment calls. Which intents the account needs, how to split the budget, which ad groups exist, which words people type, what the ads say and what each ad group bids.

mads computes everything that has a right answer. Keyword lists, the final ad, cross negatives, the CSV rows. It also refuses anything that breaks Google's limits or your instructions. The agent can be wrong. The export cannot be malformed.

## Intents

The plan mission splits the account by search intent.

| Intent | Who searches | Role |
|---|---|---|
| `brand` | People typing your business name | Cheap and protective. |
| `catalog` | People typing a specific entity name plus an evaluation or purchase word | One ad group per entity or tight group. Usually the highest intent. |
| `generic` | People searching the category or the problem without a name | Broader terms. |
| `competitor` | People typing a competitor's name | Only when `business.competitors` is not empty. Carries trademark risk, so it stays small. |

The intent follows the words people type. A search for the kind of product, such as "app to scan a label", is `generic` even when it competes with a known brand. The plan prompt tells the agent never to mix searches with a competitor name and searches without one in one campaign. Otherwise the trademark risk of one would shrink the budget of the other.

An intent that does not fit is skipped. No catalog means no `catalog` campaign. An account has 1 to 5 campaigns.

## Budget

The prompt asks for 10 to 20 percent of the budget on the brand campaign. That is guidance to the agent, not a rule the validator checks. The validator checks the sum.

Campaign budgets are currency units with at most 2 decimals. They must add up to exactly `budget.daily` (`E06`), and each campaign needs at least 1.00. mads computes in integer cents, so rounding never drifts. The agent explains each share in the campaign `rationale`, and it shows up in `report.md`.

## Ad groups and landing pages

Ad groups are tightly themed. One entity, or a few that share the same searches. The plan lists them with a name, a theme, entity ids and a final URL. The campaign mission must build every planned ad group and nothing else (`E12`).

The final URL resolves at plan time. A non-empty one must be in the allowed set (`E07`). An empty one becomes the entity's URL when the ad group has exactly one entity, the first `[focus]` URL when there is one, and `business.url` otherwise. A made-up URL cannot get through, and the URL check at the end catches a real one that does not answer.

## Keywords

The agent does not write the keyword list. It sends a spec of variants, modifiers, an `exact_heads` switch and a few extras. Limits for each field are in [MCP tools](../reference/mcp-tools.md).

mads expands the spec the same way every time.

Variants and modifiers are normalized first. Lowercase, trimmed, spaces collapsed, empties and duplicates dropped. Every variant becomes a phrase keyword, and so does every variant followed by a modifier. A modifier already inside the variant is skipped, so `vinellu app` with `app` does not give `vinellu app app`. The phrase list is sorted and deduplicated. Then, with `exact_heads` on, each variant is added again as an exact keyword in the order the agent gave. Last come the extras that are not already there.

A one-word variant is treated differently outside brand campaigns. As a phrase keyword it matches any search that contains the word, whatever else the person asks. So when `exact_heads` is on, a one-word variant in a non-brand campaign stays exact only. `W09` flags any one-word phrase keyword outside brand campaigns, whether it came from a variant while `exact_heads` is off or from `extra`.

For example, in a brand campaign the variants `vinellu` and `vinellu app` with the modifiers `baixar` and `grátis` give these keywords.

```text
Phrase: vinellu
Phrase: vinellu app
Phrase: vinellu app baixar
Phrase: vinellu app grátis
Phrase: vinellu baixar
Phrase: vinellu grátis
Exact:  vinellu
Exact:  vinellu app
```

Only phrase and exact match exist. Broad match is out of scope.

All variants of one spec are treated as one search. Variants that look like different searches are refused with `E26`, so each distinct search gets its own ad group, its own ad and its own landing page. Google rates a group that mixes searches low quality and raises its price.

Keyword length over 80 characters is `E01`. More than 10 words or a forbidden character is `E08`. The forbidden set and every other limit are in [Validation rules](../reference/validation-rules.md).

## Negatives

Negatives stop wasted clicks. They come from three places.

Ad group negatives come from the agent when it saves the ad group. Campaign negatives come from the agent once per campaign. The prompt asks for the shared ones there and never repeated per ad group, such as job seekers, wholesale, free when the offer is paid, recipes and other meanings of the names. Cross negatives come from mads, after all missions end.

For every campaign that is not `brand`, mads adds each `brand_terms` entry as a campaign-level phrase negative. For every campaign that is not `competitor`, it does the same with each `competitors` entry. Image campaigns take no negatives and are left alone. A term that would block one of the campaign's own keywords is skipped and noted in the Validation section of `report.md`.

```text
skipped negative 'vinellu' in campaign 'generic': it would block keyword 'vinellu app'
```

Running the step twice gives the same account.

The rule that protects your keywords is `E09`. A negative must never block a keyword in its scope. A phrase negative blocks a keyword when its words appear in the keyword's words in the same order and next to each other. An exact negative blocks a keyword with equal text. The tools check this on every write, so an agent cannot add a negative that kills a keyword it built.

## Ads

A responsive search ad takes up to 15 headlines and 4 descriptions. mads builds each one from two parts.

The brand kit is set once by the plan mission and shared by every ad. The plan agent first names 2 to 5 competitive differentiators, facts of the business that its competitors do not offer, and writes the kit around them. The kit carries those differentiators, proof, a call to action and the brand. `get_brief` gives the differentiators to every campaign mission. The specific texts are set per ad group. They start with the main keyword, repeat keywords of the group word for word and apply the differentiators to the entity, such as a question, a benefit or proof.

The final ad is computed at export and never stored. Specific texts come first, the brand kit fills the rest, and duplicates are dropped by normalized text. When a specific text equals a brand kit text, the specific one stays. A duplicate inside either list is an error (`E03`), because it points to an agent mistake. The merged ad must have 15 headlines and 4 descriptions (`E34`), 3 headlines under 20 characters (`E35`), the main keyword as its first headline (`E36`) and 3 headlines that are keywords of the group (`E37`). The main keyword is the first keyword variant, which the campaign agent puts first because it is the most searched way to write the theme. Ad groups of runs made before these rules have no main keyword. They raise `W04` for a short ad and still export.

That is why the later headlines and descriptions repeat in every ad of an account. It is the brand kit showing through. Counts and character limits are in [Validation rules](../reference/validation-rules.md).

## Assets

Each Search campaign sets sitelinks, callouts and structured snippets before it can finish. Sitelink URLs must be in the allowed set. Thin assets raise `W05`.

mads validates assets but does not export them yet. See [Limitations](../reference/limitations.md).

## Focus

With `[focus]` in `business.toml` the account advertises one offer. Every campaign stays on it. Brand and competitor campaigns combine their names with the focus words, and every landing page must be a focus URL (`E22`). A keyword without the focus words is `E23` when the focus has terms. Sitelinks may go elsewhere.

## Formats

Each campaign has a `kind`, and the plan agent picks it per campaign: `search` (the default), `demand_gen`, `performance_max` or `app_installs`. Each kind has the bid strategies it accepts, listed in [Image and app campaigns](image-campaigns.md). Any other pair is `E17`. `maximize_conversions` needs `business.conversion_tracking = true` (`E10`), except for App campaigns, because the store counts the installs.

Image formats open only when the run has an image model and `business.toml` has a logo. With them available, the plan prompt makes one image campaign the default, with at least 20 percent of the daily budget, reserved before the rest is split among Search campaigns. The agent can skip it, but only with a rationale that starts with `No image campaign:` and a reason. Without that reason the plan is refused with `NO_IMAGE_REASON`. The 20 percent is a prompt rule. The code enforces the explanation. App campaigns work the same way with `No app campaign:` and `NO_APP_REASON`. Formats you list in `[campaigns] formats` are mandatory (`E21`) and need no reason.

Image campaigns have asset groups instead of ad groups, and their pictures are drawn after the missions end, within `--max-images`. See [Image and app campaigns](image-campaigns.md).

## Bids

Search campaigns take only `manual_cpc`. The plan tool refuses the other strategies with `UNSUPPORTED`. Maximize clicks and maximize conversions are not accepted on Search yet.

Each Search ad group gets a `default_cpc` and a `cpc_rationale`. These are estimates. mads has no auction data, no Keyword Planner volumes and no history. A keyword under the [first page bid estimate](https://support.google.com/google-ads/answer/105665?hl=en) stays active but [does not reach the first page](https://support.google.com/google-ads/answer/2453978?hl=en). So the prompt asks for what searches of that kind cost, below `budget.max_cpc` when it is set.

A little-known brand gets brand and competitor bids close to the generic ones, because a low bid keeps the group from showing. `E11` rejects a zero CPC and any CPC above `max_cpc`.

The report labels budget shares and CPCs as estimates. Check them before you enable anything.

## After the missions

When every mission finished, mads adds the cross negatives described above. It then draws the pictures for image campaigns, and a run without them skips that. Next it validates the whole account, and any error stops the run with exit code `3` and no CSV. It checks every final and sitelink URL unless you skip the check, and a URL that does not answer 2xx is `E15`. Last it exports the files, then writes `report.md` and `run.json`.

Mission prompts help, but the tools already refused most mistakes. This second validation exists because it is the last gate before a file leaves mads. Before it writes, mads removes CSVs left by an earlier run, so a failed run never leaves old files beside the new report.

## What the validator enforces and why

Rules fall into five groups.

Google's limits are text lengths, counts and keyword syntax. Google rejects these at upload, so mads rejects them first. Your instructions are the `business.avoid` terms, the budget sum, the `max_cpc` ceiling and no `!` in headlines. Hallucination guards are URLs outside your site and URLs that do not answer. A made-up URL returns 404 and gets the ad disapproved. Self-inflicted damage is a negative that blocks your keywords, a duplicate name or a duplicate text. Policy and quality warnings are third-party terms, all-caps words, the same keyword in two ad groups, short ads and thin assets.

Errors block the run. Warnings go to the report and, in CI, to annotations. The full table is in [Validation rules](../reference/validation-rules.md).

## Next

- [Review a run](../how-to/review-a-run.md)
- [Limits and retries](limits-and-retries.md)
- [How optimize decides](how-optimize-decides.md)
- [Architecture](architecture.md)
