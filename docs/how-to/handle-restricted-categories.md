# Handle restricted categories

Fix the `restricted` list in `business.toml`, then ask Google for an exemption on keywords or ads that Google Ads Editor refuses under a restricted content policy.

## Before you start

- A run exists and you are importing it into Google Ads Editor. See [Import into Google Ads Editor](import-into-google-ads-editor.md).
- You know why mads asks about restricted categories. See [Restricted categories](../explanation/restricted-categories.md).
- The six allowed values of `restricted` are in the [business.toml reference](../reference/business-toml.md).

## Steps

1. Check the decision in `business.toml`.

   `mads init` writes `restricted` for every business, `[]` included, and prints the result when it ends.

   Example run (replay provider, no model was called). The init read a local site on `http://127.0.0.1:8765`. The TOML is an excerpt of the generated `business.toml`.

   ```text
   Restricted categories: Alcohol. Google may refuse some keywords: the report explains how to ask for an exception
   ```

   ```toml
   [business]
   restricted = ["alcohol"]
   ```

   If the agent missed a category or added a wrong one, edit the list. The report section and the prompt rules read this key, so a change reaches a run only when you generate again. Generating calls a model and spends your tokens.

2. Open the `Restricted categories` section of `report.md` in the run folder.

   With at least one category listed, the report holds the policy names and a paragraph to paste. With an empty list the report has no such section. See the [report.md reference](../reference/report-md.md).

   Example run (replay provider, no model was called). The run used `--skip-url-check`.

   ```markdown
   ## Restricted categories

   `business.toml` lists this business under Google Ads restricted content: Alcohol. Google may refuse some keywords and ads at upload even when they only inform, often by product name. In Google Ads Editor they show an error such as `Alcohol sale` after posting.

   If the business does not do what the policy restricts (for example, it does not sell the product online), select the refused items in Editor, tick Request exception, paste the text below, and post again. Google reviews it in a few business days. Otherwise remove the items.

   > Vinellu (https://vinellu.com): App social de vinhos. Our ads and keywords give information, reviews and comparisons about products in the Alcohol category. We do not sell these products online. Please review them under the Alcohol policy.
   ```

   The report says "Request exception". Google's button is named **Request exemption**, so look for that name in Editor.

3. Import `google-ads/editor/account.csv` and post.

   Google's help says items that failed to post are marked with an exclamation mark (!). Select one to read its policy violation.

4. Decide if an exemption fits.

   - The business does not do what the policy restricts, for example it does not sell the product online. Go to step 5.
   - The business does sell it. An exemption does not apply. Remove the refused items, or read the policy for your country in Google Ads help.

5. Select the refused keywords or ads and click **Request exemption** in the edit panel.

   Use the paragraph from the report. Google's help names the button and does not say where Editor takes the text, so paste it where Editor asks for an explanation.

6. Post again.

   Google's help states a limit. An item that posted and was then disapproved cannot get an exemption. Fix it so it can serve, or remove it.

Google's page for this is [Fix issues with your ads and keywords](https://support.google.com/google-ads/editor/answer/9059819). It is the source for the labels above.

## Check it worked

- `business.toml` lists every category that applies to what your ads talk about, and `report.md` names the same policies.
- After the new post, each refused item is either accepted, still marked and waiting on Google, or edited or removed by you. None is left unread.
- The rest of the campaign posts normally. A refused keyword does not block the other items.

## Next

- [Import into Google Ads Editor](import-into-google-ads-editor.md)
- [Review a run](review-a-run.md)
- [Restricted categories](../explanation/restricted-categories.md)
