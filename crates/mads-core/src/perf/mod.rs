//! Google Ads reports of a live account, read for `mads optimize`.

mod digest;
mod model;
mod table;
mod target;

pub use digest::{ReportInput, digest};
pub use model::*;
pub use table::{ReportKind, Table, Window, read_table};
pub use target::{NO_RESULT_TARGETS, VsTarget, vs_target};

#[cfg(test)]
mod tests;
