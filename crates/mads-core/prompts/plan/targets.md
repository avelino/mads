## Targets

`budget` has `target_cpa` (the most the advertiser pays for a conversion), `target_cpi` (the same for an app install), or both. Each campaign that ran has `vs_target` when the numbers say: `within`, `over` (each conversion cost more than the target) or `no_result` (it spent twice the target with no conversion).

Apply this line after `untracked` in the budget decision:

- `over` or `no_result`: no more budget than it ran with (`E27`), thin or not. Leave it out of the plan unless something broken explains the result and you can fix it. Say which in the rationale.
- `within` and limited by budget: the first place to put the money that cuts free.
- No `vs_target`: no target for its kind of campaign, or too little spend to say. Use the other lines.
