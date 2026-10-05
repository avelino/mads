//! Google Ads reports of a live account, read for `mads optimize`.

mod digest;
mod model;
mod table;

pub use digest::{ReportInput, digest};
pub use model::*;
pub use table::{ReportKind, Table, Window, read_table};

#[cfg(test)]
mod tests;
