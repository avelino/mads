# Go live

Enable the imported campaigns one at a time, add what mads does not export, and know when to bring data back.

The import leaves every campaign paused, so nothing serves and nothing costs until you enable it. The steps below start there.

## Before you start

- The run is imported and posted in Google Ads Editor. See [Import into Google Ads Editor](import-into-google-ads-editor.md).
- You reviewed the run. See [Review a run](review-a-run.md).
- The account has a payment method and conversion tracking that records. See [Prepare your Google Ads account](prepare-google-ads.md).

Example run (replay provider, no model was called, `--skip-url-check`). The report that mads writes says this before you import:

```text
Campaigns arrive paused. Review them before enabling.
```

## 1. Check each campaign before you enable it

Open the campaign in Google Ads and read its settings. mads writes these for a Search campaign, and you own them from here on.

- Budget and bid strategy. mads writes manual CPC and the daily budget in the account currency. See [Editor file](../reference/editor-file.md).
- Location. mads writes one location row for each entry of `business.locations`.
- Network. mads writes `Google search` in the `Networks` column. Open the campaign settings and read what the account shows.
- Language. mads writes the primary language of the business.

If a value is wrong, fix the input and generate again. Do not patch the account by hand, because the next `mads optimize` reads the account and the base run, and a hand edit shows up as drift.

## 2. Enable one campaign

1. In Google Ads, open **Campaigns** in the **Campaigns** menu.
2. Click the status icon next to the campaign name, then select **Enable**.

Enabling a campaign resumes only the campaign. A paused ad group or keyword stays paused until you enable it. Steps are in [Enable, pause, or remove a campaign](https://support.google.com/google-ads/answer/2404259?hl=en).

Start with the campaign you trust most. Enable the next one after the first has run for a day or two and its ads are approved.

## 3. Add what mads does not export

mads validates sitelinks, callouts and structured snippets but writes none of them to a file. The validator accepts 2 to 8 sitelinks, 2 to 10 callouts and up to 2 snippets per campaign, and warns with `W05` when a campaign has fewer than 4 sitelinks, fewer than 4 callouts or no snippet. See [Validation rules](../reference/validation-rules.md).

Add them in Google Ads from the same text you reviewed in `report.md`.

- Sitelinks. Go to **Assets** in the **Campaigns** menu, click the plus button, then select **Sitelink**. See [About sitelink assets](https://support.google.com/google-ads/answer/2375416?hl=en).
- Callouts. See [Use as many asset types as possible](https://support.google.com/google-ads/answer/12073962?hl=en).
- Structured snippets. See [About structured snippet assets](https://support.google.com/google-ads/answer/6280012?hl=en).

mads does not write these settings either. Set them in Google Ads when you need them.

| Setting | What mads does | Where Google explains it |
|---|---|---|
| Advanced location options | Not written. The account keeps its default. | [About advanced location options](https://support.google.com/google-ads/answer/1722038?hl=en) |
| Search partners and other networks | Writes `Google search` in `Networks`. Display is not written. | [About the Google Search Network](https://support.google.com/google-ads/answer/1722047?hl=en) |
| Ad schedule | Not written. | [Set up an ad schedule](https://support.google.com/google-ads/answer/6372656?hl=en) |

The ad schedule follows the time zone of the account, not the time zone of your customers.

The full list of what mads leaves out is in [Limitations](../reference/limitations.md).

## 4. Turn off auto-apply recommendations you did not choose

Google can change a live account on its own through auto-apply recommendations. mads cannot tell those changes from yours. When it reads the reports it calls both "changed outside mads (by hand or by auto-applied recommendations)".

1. In Google Ads, open **Recommendations** in the **Campaigns** menu.
2. Select **Auto-apply settings**.
3. Uncheck each recommendation you do not want, then select **Save**.

See [Manage auto-apply recommendations](https://support.google.com/google-ads/answer/10276359?hl=en). A budget that moves by more than 20 percent, a changed bid strategy, a pause or a removal becomes a finding in the next run. See [How optimize decides](../explanation/how-optimize-decides.md#drift-since-the-last-run).

## 5. Check the ads were approved

1. In Google Ads, open **Ads** in the **Campaigns** menu.
2. Read the **Status** column and hover over a status for the reason.

`Under review` usually clears within one business day. `Eligible` can show. `Disapproved` gets no impressions. See [Find your ad status](https://support.google.com/google-ads/answer/1722129?hl=en).

For a policy refusal on a restricted category, see [Handle restricted categories](handle-restricted-categories.md).

## Leave the account alone while data comes in

Every change you make by hand muddies the numbers `mads optimize` reads.

- Do not rename campaigns or ad groups. The names tie the reports to the run.
- Do not pause a campaign you want to keep. A campaign the new plan keeps is exported enabled, even when you paused it.
- After 2 days, read the keyword status and the first page bid estimate. The report tells you what to do with `Below first page bid` and `Rarely shown`. See [report.md](../reference/report-md.md#after-launch).

## When to come back

Come back when each campaign has run 14 days and has 100 clicks. mads treats a campaign as thin when it has fewer than 14 days or fewer than 100 clicks, and fewer than 30 conversions. A thin campaign gets structure fixes only. The agents do not judge its numbers.

- 30 conversions judge a campaign even before 14 days.
- A campaign that is enabled with no ads, or has no impression for 7 days, is not thin. It is broken, and optimize treats it that way.
- A small budget can take months to reach 100 clicks. Wait for the data. Optimizing on noise moves money for no reason.

## Check it worked

- The campaign status in Google Ads is `Enabled`.
- Each ad shows `Eligible` in **Status**.
- Sitelinks and callouts show under the campaign in **Assets**.
- Auto-apply settings list only the recommendations you chose.

## Next

- [Export Google Ads reports](export-google-ads-reports.md)
- [Optimize a live account](optimize-a-live-account.md)
- [Safe on a live account](../explanation/safe-on-a-live-account.md)
