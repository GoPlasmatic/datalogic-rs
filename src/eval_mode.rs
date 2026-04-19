//! Evaluation mode trait that parameterises the interpreter over
//! traced-vs-plain dispatch.
//!
//! Two modes implement [`Mode`]:
//!
//! - [`Plain`] — a zero-sized type whose methods are inlined no-ops. Because
//!   `Plain::TRACED` is `false` at monomorphisation time, any
//!   `if M::TRACED { ... }` branch is dead-code-eliminated under Plain,
//!   collapsing the generic implementation to the same machine code that
//!   the previous hand-rolled untraced evaluator produced.
//! - [`Traced`] — wraps the existing [`TraceCollector`] and node id map,
//!   recording per-node execution steps for step-by-step debugging. Only
//!   available with the `trace` feature.
//!
//! Generic operators and the core evaluator are parameterised over
//! `M: Mode`, so the two dispatch forms share a single source of truth.

use serde_json::Value;

use crate::{CompiledNode, Result};

/// Trait parameterising the evaluator over traced-vs-plain dispatch.
pub trait Mode {
    /// `true` for modes that actually collect traces. Used as a compile-time
    /// constant so generic callers can gate trace-only code with
    /// `if M::TRACED { ... }` and let the optimiser fold it away.
    const TRACED: bool;

    /// Record a completed node result. Called once per non-literal node
    /// after its value (or error) has been computed.
    fn on_node_result(&mut self, node: &CompiledNode, ctx_data: &Value, result: &Result<Value>);

    /// Mark entry into an iteration frame (map/filter/reduce/all/some/none).
    fn push_iteration(&mut self, index: u32, total: u32);

    /// Mark exit from an iteration frame.
    fn pop_iteration(&mut self);
}

/// Zero-sized plain (untraced) evaluation mode.
///
/// All methods are inlined no-ops, and `TRACED` is `false`, so the
/// optimiser collapses any trace-gated branches under this mode.
pub struct Plain;

impl Mode for Plain {
    const TRACED: bool = false;

    #[inline(always)]
    fn on_node_result(&mut self, _: &CompiledNode, _: &Value, _: &Result<Value>) {}

    #[inline(always)]
    fn push_iteration(&mut self, _: u32, _: u32) {}

    #[inline(always)]
    fn pop_iteration(&mut self) {}
}

/// Traced evaluation mode.
///
/// Records an [`ExecutionStep`](crate::trace::ExecutionStep) per non-literal
/// node into the wrapped [`TraceCollector`], keyed by the compile-time id
/// stored directly on the [`CompiledNode`]. No pointer-keyed side-table.
#[cfg(feature = "trace")]
pub struct Traced<'a> {
    /// Collector receiving recorded steps.
    pub collector: &'a mut crate::trace::TraceCollector,
}

#[cfg(feature = "trace")]
impl Mode for Traced<'_> {
    const TRACED: bool = true;

    fn on_node_result(&mut self, node: &CompiledNode, ctx_data: &Value, result: &Result<Value>) {
        let id = node.id();
        match result {
            Ok(v) => self.collector.record_step(id, ctx_data.clone(), v.clone()),
            Err(e) => self
                .collector
                .record_error(id, ctx_data.clone(), e.to_string()),
        }
    }

    fn push_iteration(&mut self, i: u32, t: u32) {
        self.collector.push_iteration(i, t);
    }

    fn pop_iteration(&mut self) {
        self.collector.pop_iteration();
    }
}
