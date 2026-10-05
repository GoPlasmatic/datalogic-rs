//! Static facts about a compiled rule: which data paths it reads, which
//! operators it uses, and whether its result is a function of its data.
//! Backs [`crate::Logic::facts`].
//!
//! The walk reads the operator table for everything it needs to know about
//! a built-in (`reads_context`, `effect`, `frames`), so a new row is
//! classified by what it declares. The frame arithmetic is the scope pass's
//! own ([`ScopeBinding::resolve`]), so a read is a root read here exactly
//! when the evaluator resolves it against the root.

use std::collections::BTreeSet;
use std::fmt;

use datavalue::OwnedDataValue;

use crate::OpCode;
use crate::compile::parse_path_segments;
use crate::compile::scope::frames_pushed_for_child;
use crate::node::{
    CompiledMissingArg, CompiledMissingMin, CompiledMissingPaths, CompiledNode, MetadataHint,
    PathSegment, ScopeBinding,
};
use crate::operators::meta::Effect;

/// A path into the data a rule is evaluated against, one segment per
/// object key or array index, from the root.
///
/// A read of a path observes the whole value there, everything under it
/// included. The empty path is the whole data context.
///
/// Segments are kept apart rather than joined, because `var` splits its
/// path on dots and `val` does not: `{"var": "a.b"}` reads `["a", "b"]`
/// while `{"val": "a.b"}` reads the single key `["a.b"]`. The
/// [`Display`](fmt::Display) form joins them with dots, so it is only
/// unambiguous for segments that contain none.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DataPath {
    segments: Vec<String>,
}

impl DataPath {
    /// The path's segments, root first. Numeric segments are kept as
    /// written (`"0"`): they index an array, or name a key on an object.
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// Whether this is the empty path, the whole data context.
    pub fn is_root(&self) -> bool {
        self.segments.is_empty()
    }

    /// Whether this path is `other` or an ancestor of it, so that a read of
    /// this path observes `other` too.
    pub fn covers(&self, other: &DataPath) -> bool {
        other.segments.starts_with(&self.segments)
    }

    fn from_segments(segments: &[PathSegment]) -> Self {
        DataPath {
            segments: segments
                .iter()
                .map(|s| match s {
                    PathSegment::Field(name) | PathSegment::FieldOrIndex(name, _) => {
                        name.to_string()
                    }
                })
                .collect(),
        }
    }
}

impl fmt::Display for DataPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.segments.join("."))
    }
}

/// What a compiled rule reads, which operators it uses, and whether its
/// result is a function of its data. Returned by [`crate::Logic::facts`].
///
/// The facts describe the **compiled** rule, after the optimizer: a branch
/// that constant folding removed is not read, and an operator it folded
/// away is not listed. An engine built
/// [`with_constant_folding(false)`](crate::EngineBuilder::with_constant_folding)
/// can therefore report more than the default engine does for the same
/// rule. Both answers are sound for the rule they describe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    reads: Vec<DataPath>,
    computed_reads: bool,
    operators: Vec<&'static str>,
    custom_operators: Vec<String>,
    deterministic: bool,
    context_readers: bool,
}

impl Facts {
    /// The data paths the rule reads from the root of its input, sorted,
    /// without duplicates, and without any path another listed path covers
    /// (reading `user` observes `user.name`).
    ///
    /// Only reads resolved against the root are listed. An iterator body
    /// reads the current element, and a `try` catch arm the caught error;
    /// those reads stay out, because the element came from the iterator's
    /// source, which is listed. A level marker that climbs back to the
    /// root (`{"val": [[1], "rate"]}` inside one `map`) is a root read and
    /// is listed.
    ///
    /// The list is complete only when [`Self::reads_complete`] holds.
    pub fn reads(&self) -> &[DataPath] {
        &self.reads
    }

    /// Whether the rule reads a path that is only known at evaluation
    /// time: a `var` / `val` path, or a `missing` / `missing_some` /
    /// `exists` path list, computed by an expression. Such a read can
    /// reach any part of the data, so [`Self::reads`] is then a lower
    /// bound. The expression that computes the path is itself listed.
    pub fn has_computed_reads(&self) -> bool {
        self.computed_reads
    }

    /// Whether [`Self::reads`] lists everything the rule can read: no
    /// computed path, and no custom operator that reads the context (one
    /// can read all of it through
    /// [`EvalContext::root_input`](crate::operator::EvalContext::root_input)).
    /// A custom operator that does not declares so in
    /// [`CustomOperator::info`](crate::CustomOperator::info).
    pub fn reads_complete(&self) -> bool {
        !self.computed_reads && !self.context_readers
    }

    /// Whether the rule can read its data at all. `false` means its result
    /// does not depend on the data (it can still depend on the clock; see
    /// [`Self::is_deterministic`]).
    pub fn reads_data(&self) -> bool {
        !self.reads_complete() || !self.reads.is_empty()
    }

    /// The built-in operators the compiled rule uses, by canonical name
    /// (`val` for `var`, `if` for `?:`), sorted and without duplicates.
    /// The names match [`crate::OperatorInfo::name`], so each can be looked
    /// up in [`crate::Engine::operators`].
    pub fn operators(&self) -> &[&'static str] {
        &self.operators
    }

    /// The custom operators the compiled rule calls, sorted and without
    /// duplicates.
    pub fn custom_operators(&self) -> &[String] {
        &self.custom_operators
    }

    /// Whether the result depends on nothing but the data: no operator
    /// that reads the clock (`now`), and no custom operator that does not
    /// declare itself deterministic in
    /// [`CustomOperator::info`](crate::CustomOperator::info). Raising an
    /// error (`throw`) is deterministic.
    pub fn is_deterministic(&self) -> bool {
        self.deterministic
    }
}

#[derive(Default)]
struct Collector {
    reads: BTreeSet<DataPath>,
    computed_reads: bool,
    operators: BTreeSet<&'static str>,
    custom_operators: BTreeSet<String>,
    nondeterministic: bool,
    context_readers: bool,
}

impl Collector {
    fn read(&mut self, depth: u32, scope_level: u32, segments: &[PathSegment]) {
        if ScopeBinding::resolve(depth, scope_level) == ScopeBinding::Root {
            self.reads.insert(DataPath::from_segments(segments));
        }
    }

    /// `missing` / `missing_some` look paths up in the current frame's
    /// data, which is the root only outside every pushed frame.
    fn missing_path(&mut self, depth: u32, segments: &[PathSegment]) {
        self.read(depth, 0, segments);
    }

    /// A `missing` / `missing_some` argument evaluated at runtime: a
    /// string path, or an array of them (anything else is ignored). A
    /// literal one is as static as a pre-parsed path.
    fn dynamic_missing_paths(&mut self, node: &CompiledNode, depth: u32) {
        let literal = |value: &OwnedDataValue, this: &mut Self| match value {
            OwnedDataValue::String(s) => this.missing_path(depth, &parse_path_segments(s)),
            OwnedDataValue::Array(items) => {
                for item in items {
                    if let OwnedDataValue::String(s) = item {
                        this.missing_path(depth, &parse_path_segments(s));
                    }
                }
            }
            _ => {}
        };
        match node {
            CompiledNode::Value { value, .. } => literal(value, self),
            // An unfolded literal array of literals.
            CompiledNode::Array { nodes, .. }
                if nodes
                    .iter()
                    .all(|n| matches!(n, CompiledNode::Value { .. })) =>
            {
                for n in nodes.iter() {
                    if let CompiledNode::Value {
                        value: OwnedDataValue::String(s),
                        ..
                    } = n
                    {
                        self.missing_path(depth, &parse_path_segments(s));
                    }
                }
            }
            _ => {
                self.computed_reads = true;
                self.walk(node, depth);
            }
        }
    }

    fn walk(&mut self, node: &CompiledNode, depth: u32) {
        match node {
            CompiledNode::Value { .. } => {}
            CompiledNode::Var {
                scope_level,
                segments,
                metadata_hint,
                default_value,
                ..
            } => {
                self.operators.insert(OpCode::Val.as_str());
                // `index` / `key` at a metadata level read the iteration
                // frame's metadata, not data.
                if *metadata_hint == MetadataHint::None {
                    self.read(depth, *scope_level, segments);
                }
                if let Some(default) = default_value {
                    self.walk(default, depth);
                }
            }
            #[cfg(feature = "ext-control")]
            CompiledNode::Exists(data) => {
                self.operators.insert(OpCode::Exists.as_str());
                // No path names the data itself, which always exists.
                if !data.segments.is_empty() {
                    self.read(depth, data.scope_level, &data.segments);
                }
            }
            #[cfg(feature = "error-handling")]
            CompiledNode::Throw(_) => {
                self.operators.insert(OpCode::Throw.as_str());
            }
            CompiledNode::Missing(data) => {
                self.operators.insert(OpCode::Missing.as_str());
                for arg in data.args.iter() {
                    match arg {
                        CompiledMissingArg::Now((_, segments)) => {
                            self.missing_path(depth, segments)
                        }
                        CompiledMissingArg::Later(node) => self.dynamic_missing_paths(node, depth),
                    }
                }
            }
            CompiledNode::MissingSome(data) => {
                self.operators.insert(OpCode::MissingSome.as_str());
                if let CompiledMissingMin::Later(node) = &data.min_present {
                    self.walk(node, depth);
                }
                match &data.paths {
                    CompiledMissingPaths::Now(paths) => {
                        for (_, segments) in paths.iter() {
                            self.missing_path(depth, segments);
                        }
                    }
                    CompiledMissingPaths::Later(node) => self.dynamic_missing_paths(node, depth),
                }
            }
            CompiledNode::InvalidArgs { op_name, .. } => {
                // Its arguments are never evaluated, so nothing is read.
                if let Ok(op) = op_name.parse::<OpCode>() {
                    self.operators.insert(canonical_name(op));
                }
            }
            CompiledNode::CustomOperator(data) => {
                self.custom_operators.insert(data.name.clone());
                self.nondeterministic |= !data.info.deterministic;
                self.context_readers |= data.info.reads_context;
                for arg in data.args.iter() {
                    self.walk(arg, depth);
                }
            }
            CompiledNode::BuiltinOperator { opcode, args, .. } => {
                self.builtin(*opcode, args, depth);
            }
            // Arrays, the CSE wrapper and output templates evaluate their
            // children at their own depth and read nothing themselves.
            _ => node.visit_indexed_children(&mut |_, child| self.walk(child, depth)),
        }
    }

    fn builtin(&mut self, opcode: OpCode, args: &[CompiledNode], depth: u32) {
        let meta = opcode.meta();
        self.operators.insert(canonical_name(opcode));
        if meta.effect == Effect::Clock {
            self.nondeterministic = true;
        }
        if meta.reads_context {
            match opcode {
                // Without a bucketing expression `fractional` hashes these
                // two fields of the root, at any depth. Which form a call
                // takes is only known at runtime, so both are reported.
                #[cfg(feature = "flagd")]
                OpCode::Fractional => {
                    for path in [&["targetingKey"][..], &["$flagd", "flagKey"]] {
                        self.reads.insert(DataPath {
                            segments: path.iter().map(|s| s.to_string()).collect(),
                        });
                    }
                }
                // Every other context reader that is still a generic node
                // (a `val` / `var` / `exists` whose path or level is an
                // expression) reads a path the compiler could not resolve.
                _ => self.computed_reads = true,
            }
        }
        let len = args.len();
        for (index, arg) in args.iter().enumerate() {
            self.walk(arg, depth + frames_pushed_for_child(opcode, index, len));
        }
    }

    fn finish(self) -> Facts {
        // A sorted set puts every path right after its ancestors, so one
        // pass against the last kept path drops every covered one.
        let mut reads: Vec<DataPath> = Vec::with_capacity(self.reads.len());
        for path in self.reads {
            if reads.last().is_none_or(|kept| !kept.covers(&path)) {
                reads.push(path);
            }
        }
        Facts {
            reads,
            computed_reads: self.computed_reads,
            operators: self.operators.into_iter().collect(),
            custom_operators: self.custom_operators.into_iter().collect(),
            context_readers: self.context_readers,
            deterministic: !self.nondeterministic,
        }
    }
}

/// The name [`crate::Engine::operators`] lists the opcode under. An
/// internal opcode reports the operator it renders as (`VarDefault` is
/// `val`).
fn canonical_name(op: OpCode) -> &'static str {
    match op.catalogue_entry().names.first() {
        Some(name) => name,
        None => op
            .as_str()
            .parse::<OpCode>()
            .map_or(op.as_str(), OpCode::as_str),
    }
}

pub(crate) fn collect(root: &CompiledNode) -> Facts {
    let mut collector = Collector::default();
    collector.walk(root, 0);
    collector.finish()
}
