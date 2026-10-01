# Why mads

This page explains the problem mads solves, why it splits the work between agents and rules, and what it leaves to you.

## The problem

A Google Ads Search account for a catalog business is a lot of rows. A single ad group for one wine label needs 14 keywords, 8 negatives and one responsive search ad with up to 15 headlines and 4 descriptions. Twelve labels is 183 keyword rows, 96 negative rows and 12 ads in the reference export in `data/`.

Building that by hand has three costs.

- **Typing.** You repeat the same keyword pattern (name variants times intent words) and the same negatives for every entity.
- **Limits.** Headlines stop at 30 characters, descriptions at 90, paths at 15. Google rejects a row at upload and you fix it one cell at a time.
- **Judgment.** Someone has to decide the campaign split, the budget share, the CPC, the negatives and the ad angles. That is the part worth your time.

## Why agents plus deterministic rules

An LLM is good at the judgment work. It writes headlines in your language, picks intent words and explains a budget split. It is bad at counting characters, remembering that `!` is banned in headlines and keeping a budget exact to the cent.

mads gives each side the job it is good at.

- **Agents decide.** They plan campaigns, choose keyword variants and modifiers, write ad texts, set CPC estimates and justify them.
- **Code enforces.** Every tool validates its input before it changes state. A call that breaks a rule returns the errors to the agent and changes nothing. The agent reads the errors and fixes the call.
- **Code expands.** The agent does not write the keyword list. It sends variants and modifiers and mads builds the list. The agent does not merge ads either. It writes the ad group specific texts and mads completes the ad with the shared brand kit.
- **Code gates export.** After all missions finish, mads validates the whole account again. One error and no CSV is written.

The result is that the output can be wrong about marketing, but it cannot break Google's format.

## What mads does

- Drafts `business.toml` and `catalog.csv` from your website (`mads init`).
- Plans 1 to 5 campaigns by intent from your business description and catalog.
- Builds ad groups, keywords, negatives, responsive search ads, sitelinks, callouts and structured snippets.
- Adds your brand terms and competitor names as negatives to the campaigns they do not belong to.
- Checks that every final URL belongs to your site and answers with a 2xx status.
- Writes five bulk upload CSVs and a `report.md`.
- Runs unattended in CI.

## What mads does not do

- **It does not upload.** It writes files. You upload them.
- **It does not enable campaigns.** They are exported paused by default.
- **It has no auction data.** Bids and budget shares are estimates. Check them against the Keyword Planner.
- **It does not read your account.** There is no Google Ads API access and no performance data.
- **It does not cover other campaign types.** No Performance Max, Shopping, Dynamic Search Ads, App, video or image campaigns, and no broad match.
- **It does not guarantee results.** It guarantees the files follow the format and the rules. Whether the ads convert depends on your offer, your site and the market.

Two limits to know. Only `manual_cpc` bidding is accepted. Sitelinks, callouts and snippets are collected and validated, but the asset CSVs are not written. See [Limitations in the README](../README.md#limitations).

## Next

Install mads and run it in [Getting started](getting-started.md).
