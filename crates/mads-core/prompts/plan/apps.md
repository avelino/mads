## App campaigns

`get_business` lists `apps`. `app_installs` shows ads in the store, YouTube, Search and Display and optimizes for installs, which a Demand Gen or Search ad that says "download the app" cannot do. It takes `maximize_conversions` and needs no `conversion_tracking`: the store counts the installs. Its `ad_groups` are themes of the app (one use, one audience), and they land on the store page by themselves.

- With `app_campaigns.available` true, the default is one `app_installs` campaign. Leave it out only for a concrete reason, such as a goal that is sales on the site, and start the rationale of your largest campaign with `No app campaign:` and that reason (`NO_APP_REASON` otherwise). When `required_formats` is set, it decides the formats and no reason is asked. With it false, plan no `app_installs` campaign.
- With an app on each store, plan one `app_installs` campaign per store and set `app_store` to `google_play` or `app_store`: one campaign per store, a second for the same store is `E30`. With one app, leave `app_store` as `auto`. Leave a store out only for a concrete reason, and say it in the rationale of the other app campaign.
- In an optimize run, an App campaign in `live_account` keeps the store it runs on, because Google Ads cannot change the app of a campaign that exists (`E31`). A store with no campaign yet comes in as a new `app_installs` campaign with a name of its own. Do not leave out the store of a live App campaign because another campaign seems to cover it: the one in `live_account` is the one that runs.

Split the app budget between the stores by what the numbers say, and write the reason in the rationale of each campaign:

- With `performance` for both, the cost per install decides. Give more to the store that installs for less and keep the other large enough to keep learning. A store that is `stalled` or `untracked` follows `live_rules`.
- A store with no history is new next to one that ran. Give it a test share: enough to collect installs in a few days, at most 30 percent of the app budget, and no more than the store with history until its cost per install is known.
- With no history on either store, split evenly unless `research` names the platform of the audience, and then lean toward it by at most 60 percent.
- The two stores count installs differently: compare their cost per install only as a rough guide, never as proof.
