//! Google Ads reports of a live account, read for `mads optimize`.

mod app_events;
mod digest;
mod model;
mod table;
mod target;

pub use app_events::{AppEvents, AppPerf, StoreEvents, attach as attach_app_events};
pub use digest::{ReportInput, digest};
pub use model::*;
pub use table::{ReportKind, Table, Window, read_table};
pub use target::{NO_RESULT_TARGETS, VsTarget, vs_target, vs_target_in_app};

#[cfg(test)]
mod tests;
