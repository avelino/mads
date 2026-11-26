//! Google Ads reports of a live account, read for `mads optimize`.

mod app_events;
mod conversions;
mod digest;
mod drift;
mod model;
mod networks;
mod outside;
mod table;
mod target;

pub use app_events::{AppEvents, AppPerf, StoreEvents, attach as attach_app_events};
pub use conversions::{ConversionActionPerf, NO_ACTION_FOR_STORE};
pub use digest::{ReportInput, digest};
pub use drift::{
    Base, CampaignDrift, DRIFT_BUDGET_PCT, DRIFT_SOURCE, Drift, DriftKind, drift_since,
};
pub use model::*;
pub use networks::{DISPLAY_CTR_PCT, DISPLAY_MIN_CLICKS, Network, NetworkPerf};
pub use outside::OUTSIDE_COST_SHARE_PCT;
pub use table::{ReportKind, Table, Window, read_table};
pub use target::{NO_RESULT_TARGETS, VsTarget, vs_target, vs_target_in_app};

#[cfg(test)]
mod tests;
