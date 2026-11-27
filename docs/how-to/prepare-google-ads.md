# Prepare your Google Ads account

Set up the account, billing, conversion tracking and Google Ads Editor so a mads run can be imported and judged later.

mads never talks to Google Ads. It writes files and you import them. Money in the files has no currency attached, so the account decides what every number means. Fix the account first and the rest of the loop has nothing to untangle.

## Before you start

- A Google account that can sign in to [Google Ads](https://ads.google.com/).
- A card or other payment method for the account.
- The website you will advertise. Conversion tracking needs to place a tag on it.
- mads installed. See [Install mads](install.md).

## 1. Create the account and add billing

1. Create the Google Ads account. During setup Google asks for a billing country, shows a time zone and asks for a payment method. Currency and time zone are set at this step and cannot be changed later. See [About your language, number format, time zone, and currency settings](https://support.google.com/google-ads/answer/9842104?hl=en).
2. On the "Enter your account and payment details" page, choose the billing country, check the time zone, then select **Add payment method** and **Submit**. See [Add a new payment method in Google Ads](https://support.google.com/google-ads/answer/2375375?hl=en).
3. On an existing account without a payment method, open **Settings** from the **Billing** menu, then under **Payment Methods** select **Add a payment method** and **Save**. Same help page.

No ad shows until billing is set. Google says so on that page: "Your ad won't be able to show until you've done this."

## 2. Match the currency

Pick the currency and time zone with the business in mind, because Google sets both for good when the account is created. A wrong currency means a new account.

The currency you give mads must be the account currency. It goes in `budget.currency` of `business.toml`, or in `--currency` of `mads init`. It takes 3 uppercase letters, such as `BRL`. See [business.toml](../reference/business-toml.md).

mads does not convert. It writes `report.md` with that currency and hands it to the agents. It does not write the currency into the CSV files, so Google reads every amount in the account currency. A `BRL` budget imported into a `USD` account becomes dollars.

## 3. Set up conversion tracking

mads judges a live account by its conversions. A campaign with 100 clicks and no conversion, while another campaign converts, gets its budget capped and its bids held in `mads optimize`. Without tracking the whole loop has nothing to measure. See [How optimize decides](../explanation/how-optimize-decides.md).

For a website:

1. Open **Summary** in the **Goals** menu.
2. Select **+ Create conversion action**, then **Conversions on a website**.
3. Enter the site URL, select **Scan**, and follow the prompts to link a Google Analytics 4 property or set up a Google tag. Select **Done**.
4. Select **Continue** and pick a conversion category.
5. Select **+ Add conversion actions**, pick a data source, and for a code snippet choose **Setup manually using code**. Put the snippet on the page where the action happens, such as the thank-you page.
6. Select **Done**, then **Save and continue**.

Steps are from [Set up conversions manually](https://support.google.com/google-ads/answer/12718882?hl=en). For the other sources (app installs, in-app actions, phone calls, Google Analytics imports, offline imports), see [About conversion measurement](https://support.google.com/google-ads/answer/1722022?hl=en).

Then tell mads. Set `conversion_tracking = true` in the `[business]` table only when the account really counts conversions. A campaign that bids with `maximize_conversions`, such as Performance Max, fails validation with `E10` when it is `false`. App install campaigns are exempt. See [Validation rules](../reference/validation-rules.md).

An App campaign with `optimize_for = "in_app_actions"` also needs `app.conversion_action`, the name of the in-app conversion action exactly as it is in Google Ads. See [business.toml](../reference/business-toml.md).

## 4. Install Google Ads Editor

mads writes `editor/account.csv` for Google Ads Editor. Import it for any run. A `mads optimize` result must go through Editor. See [Import into Google Ads Editor](import-into-google-ads-editor.md).

1. Download Google Ads Editor from [Google's download page](https://ads.google.com/home/tools/ads-editor/). It runs on 64-bit Windows 10 or later and macOS 13 or later. See [System requirements](https://support.google.com/google-ads/editor/answer/30511?hl=en).
2. Open it and add the account. In Google's help you click **Accounts**, then **Add**, enter the account email address and password, click **Next**, choose what to download and click **OK**. See [Download one or more Google Ads accounts](https://support.google.com/google-ads/editor/answer/30534?hl=en).

Sign in with the Google account that manages the ad account.

## Check it worked

- The account has a payment method. Open **Settings** from the **Billing** menu and see it under **Payment Methods**.
- The account currency is the one in `budget.currency`. `grep -A3 '^\[budget\]' business.toml` shows what mads has. Compare it with the currency on your Google Ads billing page.
- Open **Summary** in the **Goals** menu, set the **Conversions** drop-down to **All conversions** and look at the **Tracking status** column. Your conversion action reads **Recording** once the page records conversions. If the column shows **Unverified**, hover over the status and select **Troubleshoot**. See [Use Tag Assistant to troubleshoot your unverified or inactive conversion actions](https://support.google.com/google-ads/answer/10989978?hl=en).
- Google Ads Editor opens and lists the account.

If a conversion action does not record yet, you can still generate and import. Leave `conversion_tracking = false` and keep `export.status` at its default, `Paused`. Do not enable campaigns until tracking records.

## Next

- [Write the business file](write-the-business-file.md)
- [Review a run](review-a-run.md)
- [Run a real account](../tutorials/run-a-real-account.md)
