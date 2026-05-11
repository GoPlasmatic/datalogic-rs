use std::num::NonZeroU32;

/// Compile-time id assigned to every [`super::CompiledNode`].
///
/// `Some(n)` for nodes produced by the compile pipeline (where the counter
/// starts at 1). `None` for synthetic nodes built outside the pipeline —
/// test helpers, optimizer literal-replacement folds, `eager_apply` value
/// wrappers — which are never observed by tracing or error reporting.
///
/// Encoding the synthetic case as `None` (rather than the previous
/// `u32 = 0`) lets the type system catch the "forgot to bump the counter"
/// bug at construction sites: `id: ctx.next_id()` no longer compiles
/// against `Option<NonZeroU32>`, forcing the writer to choose between
/// `Some(ctx.next_id())` (real) and `SYNTHETIC_ID` (synthetic).
pub(crate) type NodeId = Option<NonZeroU32>;

/// Sentinel id used for synthetic nodes built outside the compile pipeline
/// (test helpers, run-time value wrappers in `eager_apply`, etc.). Real ids
/// are `Some(NonZeroU32)` since `CompileCtx` starts the counter at 1.
pub(crate) const SYNTHETIC_ID: NodeId = None;

/// Compile-time context for assigning unique node ids and threading the
/// "skip optimization" flag through the recursive descent.
///
/// `next_id` ensures every node constructed during compilation gets a fresh,
/// monotonically increasing id. The counter is [`NonZeroU32`] starting at 1;
/// the synthetic case is encoded as `None` (see [`SYNTHETIC_ID`]) and never
/// flows through this counter.
///
/// `skip_fold` is set by the trace path so the constant-fold + optimizer
/// passes are bypassed and every operator survives in the compiled tree.
#[derive(Debug)]
pub(crate) struct CompileCtx {
    next_id: NonZeroU32,
    skip_fold: bool,
}

const ID_ONE: NonZeroU32 = match NonZeroU32::new(1) {
    Some(n) => n,
    None => unreachable!(),
};

impl CompileCtx {
    pub(crate) fn new() -> Self {
        Self {
            next_id: ID_ONE,
            skip_fold: false,
        }
    }

    /// Construct a context that skips the optimizer + constant-fold passes.
    /// Used by the internal trace compile path (so traced rules retain
    /// every operator as a step source) and by `Engine::compile` when
    /// the engine was built with
    /// [`crate::EngineBuilder::with_constant_folding(false)`].
    pub(crate) fn no_fold() -> Self {
        Self {
            next_id: ID_ONE,
            skip_fold: true,
        }
    }

    /// Allocate a fresh node id. Returns the bare [`NonZeroU32`] — callers
    /// wrap it in `Some(...)` at the construction site, making the
    /// real-vs-synthetic choice explicit and forcing a type error if the
    /// id field is left unassigned.
    #[inline]
    pub(crate) fn next_id(&mut self) -> NonZeroU32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Whether to skip the optimizer + constant-fold passes during compile.
    #[inline]
    pub(crate) fn skip_fold(&self) -> bool {
        self.skip_fold
    }
}
