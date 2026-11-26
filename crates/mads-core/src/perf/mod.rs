//! Google Ads reports of a live account, read for `mads optimize`.

mod app_events;
mod conversions;
mod digest;
mod model;
mod networks;
mod table;
mod target;

pub use app_events::{AppEvents, AppPerf, StoreEvents, attach as attach_app_events};
pub use conversions::{ConversionActionPerf, NO_ACTION_FOR_STORE};
pub use digest::{ReportInput, digest};
pub use model::*;
pub use networks::{DISPLAY_CTR_PCT, DISPLAY_MIN_CLICKS, Network, NetworkPerf};
pub use table::{ReportKind, Table, Window, read_table};
pub use target::{NO_RESULT_TARGETS, VsTarget, vs_target, vs_target_in_app};

#[cfg(test)]
mod tests;
