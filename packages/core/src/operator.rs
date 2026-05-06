//! Public scaffolding for user-supplied operators.
//!
//! Today this re-exports just [`ContextStack`] — the internal evaluation
//! stack threaded through [`crate::CustomOperator::evaluate`]. Users never
//! construct one; the type lives here only because it appears in the trait
//! method signature, and grouping it under `operator::` keeps the crate
//! root focused on the dominant entry points (`Engine`, `Logic`, `Error`,
//! `DataValue`, …).

pub use crate::arena::ContextStack;
