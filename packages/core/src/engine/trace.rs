//! Engine trace helpers — gated on `feature = "trace"`.
//!
//! These methods sit beside the dispatch loop in `engine/mod.rs` rather
//! than the `crate::trace` module because they call `Engine::dispatch_node`
//! directly and need the `&mut crate::arena::ContextStack` both before
//! (to attach the tracer) and after (to take the error breadcrumb) the
//! evaluation. Inline `#[cfg(feature = "trace")]` snippets in
//! `dispatch_node` and `dispatch_iter_body` cooperate with the tracer
//! attached here.

use std::sync::Arc;

use serde_json::Value;

use crate::Logic;
use crate::Result;
use crate::engine::Engine;
use crate::trace::{ExpressionNode, TraceCollector, TracedResult};

impl Engine {
    /// Run a traced evaluation and assemble the [`TracedResult`]. Used by the
    /// [`crate::compat::LegacyApi`] trace shims.
    #[doc(hidden)]
    pub(crate) fn run_trace(&self, compiled: &Logic, data_arc: Arc<Value>) -> TracedResult {
        let expression_tree = ExpressionNode::build_from_compiled(&compiled.root);
        let mut collector = TraceCollector::new();
        let (result, error_path) = self.run_with_trace(compiled, data_arc, &mut collector);
        let steps = collector.into_steps();
        match result {
            Ok(value) => TracedResult {
                result: value,
                expression_tree,
                steps,
                error: None,
                structured_error: None,
            },
            Err(e) => {
                let message = e.to_string();
                let e = e.decorated(error_path, compiled, true);
                TracedResult {
                    result: Value::Null,
                    expression_tree,
                    steps,
                    error: Some(message),
                    structured_error: Some(e),
                }
            }
        }
    }

    /// Arena-mode traced evaluation. Allocates an arena, attaches the
    /// caller's [`TraceCollector`] to the arena context, and dispatches
    /// through `Engine::dispatch_node`. Returns `(result, error_path)` where
    /// `error_path` is the structured-error breadcrumb of node ids leading
    /// to the failure (empty on success). Calls `dispatch_node` directly
    /// (not the public [`Engine::evaluate`]) because the trace path needs the
    /// [`crate::arena::ContextStack`] both before (to attach the tracer)
    /// and after (to extract the breadcrumb) the evaluation.
    fn run_with_trace(
        &self,
        compiled: &Logic,
        data: Arc<Value>,
        collector: &mut TraceCollector,
    ) -> (Result<Value>, Vec<u32>) {
        let arena = bumpalo::Bump::new();
        let data_av = crate::arena::value_to_data(&data, &arena);
        let mut ctx = crate::arena::ContextStack::new(arena.alloc(data_av));
        // Move the caller's collector into ctx, leaving a fresh empty
        // collector in its place; restore the populated one back to the
        // caller's slot after dispatch.
        let owned = std::mem::take(collector);
        ctx.attach_tracer(owned);
        let result = self.dispatch_node(&compiled.root, &mut ctx, &arena);
        *collector = ctx.detach_tracer().expect("attach_tracer was called above");
        match result {
            Ok(av) => {
                let owned = crate::arena::data_to_value(av);
                let path = ctx.take_error_path();
                (Ok(owned), path)
            }
            Err(e) => {
                let path = ctx.take_error_path();
                (Err(e), path)
            }
        }
    }
}
