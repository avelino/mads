# Why mads

Judge whether mads fits your account by seeing the problem it solves and why it splits the work between agents and rules.

## The problem

A Google Ads account for a catalog business is a lot of rows. Each ad group needs its own keywords, negatives and a responsive search ad with up to 15 headlines and 4 descriptions. The reference export in `data/` holds 12 ad groups for one wine shop. That is 182 keyword rows, 96 negative rows and 12 ads.

By hand, it goes wrong in these places.

Typing. You repeat the same keyword pattern (name variants times intent words) and the same negatives for every entity.

Limits. Headlines stop at 30 characters, descriptions at 90, paths at 15, and a headline cannot hold `!`. Google rejects a row at import and you fix it one cell at a time.

Judgment. Someone has to decide the campaign split, the budget share, the CPC, the negatives and the ad angles. That is the part worth your time.

## Why agents plus deterministic rules

A model is good at judgment work. It writes headlines in your language, picks intent words and explains a budget split. It is bad at counting characters and at keeping a budget exact to the cent.

So each side gets the job it is good at.

Agents decide. They plan campaigns, choose keyword variants and modifiers, write ad texts and set CPC estimates.

Code enforces. Every tool validates its input before it changes anything. A call that breaks a rule returns the issues to the agent and changes nothing. The agent reads them and tries again.

Code expands. The agent does not write the keyword list. It sends variants and modifiers and mads builds the list. It also does not merge ads. It writes the texts that belong to one ad group and mads completes the ad with the shared brand kit.

Code gates the export. After all missions finish, mads validates the whole account again. One error and no CSV is written.

The output can be wrong about marketing. It cannot break Google's format. [How a run works](how-a-run-works.md) walks through the missions.

## What mads does

It drafts the business file and the catalog from your website. It plans 1 to 5 campaigns by intent, then builds the ad groups, keywords, negatives and ads. Search, Performance Max, Demand Gen and App install campaigns are supported.

It adds your brand terms and competitor names as negatives to the campaigns they do not belong to. It checks that final URLs are in the allowed set and answer with a 2xx status.

It writes the import file for Google Ads Editor and the five bulk upload CSVs, plus a `report.md` that says what it decided and why.

It improves a live account too. `mads optimize` reads the report CSVs you export from Google Ads, compares the live account with the last run and prepares the next one. [How optimize decides](how-optimize-decides.md) covers the reasoning.

It runs unattended in CI.

## What mads leaves to you

It does not upload. It writes files, and you import them. mads has no Google Ads API access, so it never touches your account.

It does not enable campaigns. They are exported paused by default. [Safe on a live account](safe-on-a-live-account.md) lists the guards around that.

It has no auction data. Bids and budget shares are estimates, so check them against the Keyword Planner.

It does not guarantee results. It guarantees that the files follow Google's format and the rules. Whether the ads convert depends on your offer, your site and the market.

The full list, with every gap the code has today, lives in [Limitations](../reference/limitations.md).

## Next

Run it end to end in [Your first run](../tutorials/first-run.md).
