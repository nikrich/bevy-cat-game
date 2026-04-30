//! Moved to `crate::edit::history`. This shim keeps existing imports working
//! during the rename; can be deleted once all call sites are updated (Task 3).
pub use crate::edit::history::*;
pub type BuildHistory = crate::edit::history::EditHistory;
