#![allow(clippy::cast_precision_loss, clippy::too_many_lines)]
#![forbid(unsafe_code)]

pub mod bom;
pub mod error;
pub mod folder;
pub mod fusion;
pub mod manifest;
pub mod model;
pub mod report;
pub mod sizes;
pub mod sourcemap;
pub mod stats;

pub use error::{Error, Result};
pub use model::UnifiedBundleGraph;
