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
    depth: usize,
    /// The JSON Pointer of the value being compiled, while pointers are
    /// recorded ([`Self::recording_pointers`]); `None` otherwise.
    pointer: Option<String>,
    /// `(node id, pointer)` for every id handed out while recording, in id
    /// order.
    pointers: Vec<(u32, Box<str>)>,
    /// Whether a fold or an optimizer rewrite consulted the engine's
    /// settings; see [`Self::note_config_fold`].
    config_folds: bool,
}

/// `(node id, JSON Pointer)` pairs in id order: what a traced compile
/// records (see [`crate::Logic::pointer`]).
pub(crate) type NodePointers = Box<[(u32, Box<str>)]>;

const ID_ONE: NonZeroU32 = match NonZeroU32::new(1) {
    Some(n) => n,
    None => unreachable!(),
};

/// Maximum rule-tree nesting accepted at compile time. Mirrors the JSON
/// parser's own depth cap so a programmatically-built `OwnedDataValue` rule
/// (which reaches the compiler via `IntoLogic` without going through the
/// string parser) can't drive unbounded recursion in `compile_node`,
/// dispatch, or the recursive `Drop` of the resulting `CompiledNode` tree.
pub(crate) const MAX_COMPILE_DEPTH: usize = 256;

impl CompileCtx {
    pub(crate) fn new() -> Self {
        Self {
            next_id: ID_ONE,
            skip_fold: false,
            depth: 0,
            pointer: None,
            pointers: Vec::new(),
            config_folds: false,
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
            depth: 0,
            pointer: None,
            pointers: Vec::new(),
            config_folds: false,
        }
    }

    /// Record, for every node id handed out, the JSON Pointer of the source
    /// value being compiled at the time (see [`crate::Logic::pointer`]).
    #[cfg(feature = "trace")]
    pub(crate) fn recording_pointers(mut self) -> Self {
        self.pointer = Some(String::new());
        self
    }

    /// Descend into `token` (an operator key, a template key or an array
    /// index) while recording. Returns the mark to [`Self::ascend`] to.
    #[inline]
    pub(crate) fn descend(&mut self, token: &str) -> usize {
        let Some(pointer) = &mut self.pointer else {
            return 0;
        };
        let mark = pointer.len();
        push_pointer_token(pointer, token);
        mark
    }

    /// [`Self::descend`] into array index `index`.
    #[inline]
    pub(crate) fn descend_index(&mut self, index: usize) -> usize {
        use std::fmt::Write;
        let Some(pointer) = &mut self.pointer else {
            return 0;
        };
        let mark = pointer.len();
        // Digits need no escaping.
        let _ = write!(pointer, "/{index}");
        mark
    }

    /// Return to the pointer [`Self::descend`] left.
    #[inline]
    pub(crate) fn ascend(&mut self, mark: usize) {
        if let Some(pointer) = &mut self.pointer {
            pointer.truncate(mark);
        }
    }

    /// The recorded `(id, pointer)` pairs, in id order; `None` when not
    /// recording.
    pub(crate) fn take_pointers(&mut self) -> Option<NodePointers> {
        self.pointer
            .is_some()
            .then(|| std::mem::take(&mut self.pointers).into_boxed_slice())
    }

    /// Enter one level of rule nesting during compilation. Errors once
    /// nesting passes [`MAX_COMPILE_DEPTH`], bounding recursion for
    /// programmatically-built rules that skip the JSON parser's own cap.
    /// Every successful `enter()` must be paired with a [`Self::leave`] so
    /// sibling subtrees are accounted from the correct depth.
    #[inline]
    pub(crate) fn enter(&mut self) -> crate::Result<()> {
        self.depth += 1;
        if self.depth > MAX_COMPILE_DEPTH {
            return Err(crate::Error::configuration_error(format!(
                "rule nesting exceeds the maximum compile depth of {MAX_COMPILE_DEPTH}"
            )));
        }
        Ok(())
    }

    /// Leave one level of rule nesting. Pairs with [`Self::enter`].
    #[inline]
    pub(crate) fn leave(&mut self) {
        self.depth -= 1;
    }

    /// Allocate a fresh node id. Returns the bare [`NonZeroU32`] — callers
    /// wrap it in `Some(...)` at the construction site, making the
    /// real-vs-synthetic choice explicit and forcing a type error if the
    /// id field is left unassigned.
    #[inline]
    pub(crate) fn next_id(&mut self) -> NonZeroU32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        if let Some(pointer) = &self.pointer {
            self.pointers.push((id.get(), pointer.as_str().into()));
        }
        id
    }

    /// Record that a fold or an optimizer rewrite evaluated something
    /// under the engine's settings (truthiness, number coercion, NaN and
    /// division handling, loose equality), so the compiled tree is only
    /// right for engines that share them.
    #[inline]
    pub(crate) fn note_config_fold(&mut self) {
        self.config_folds = true;
    }

    /// Whether [`Self::note_config_fold`] was called.
    pub(crate) fn has_config_folds(&self) -> bool {
        self.config_folds
    }

    /// Whether to skip the optimizer + constant-fold passes during compile.
    #[inline]
    pub(crate) fn skip_fold(&self) -> bool {
        self.skip_fold
    }
}

/// Append `token` to a JSON Pointer, escaped per RFC 6901. Trace pointers
/// ([`CompileCtx::descend`]) and diagnostic pointers (`check`) share it, so
/// the two agree byte for byte.
pub(crate) fn push_pointer_token(pointer: &mut String, token: &str) {
    pointer.push('/');
    for c in token.chars() {
        match c {
            '~' => pointer.push_str("~0"),
            '/' => pointer.push_str("~1"),
            c => pointer.push(c),
        }
    }
}
