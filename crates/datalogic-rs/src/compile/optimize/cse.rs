//! Common-subexpression elimination (CSE).
//!
//! JSONLogic has no `let` bindings, so rule authors repeat pure aggregate
//! subexpressions verbatim (a checkout rule recomputes its subtotal
//! map+reduce everywhere the subtotal is referenced, making evaluation
//! O(items × ref-count)). This pass detects structurally identical pure
//! subtrees worth memoizing and wraps every occurrence of one equivalence
//! class in a [`CompiledNode::Cse`] carrying a shared memo-slot index. At
//! evaluation time `Engine::dispatch_cse` computes the first occurrence and
//! serves the rest from the per-evaluation slot table on `ContextStack`.
//!
//! # Soundness
//!
//! A memoized value may only be reused where re-evaluation would provably
//! produce it again. Two gates guarantee that:
//!
//! - **Purity (compile time):** only subtrees built entirely from pure
//!   builtin operators are candidates. `CustomOperator` (opaque, possibly
//!   re-entrant), `StructuredObject`, `Throw`/`Try` (error control flow),
//!   `Now` (time), and `Fractional`/`SemVer` (kept dynamic by policy, see
//!   `opcode_is_static`) disqualify a subtree. `Var`/`Missing`/`Exists`
//!   remain eligible — they read context, which the runtime gate pins.
//! - **Context (runtime):** the memo is consulted only at
//!   `ctx.depth() == 0`, where every context read resolves against the
//!   root (up-level `val`s clamp to root), so a pure subtree is a function
//!   of (root data, engine config) alone.
//!
//! # Compile-time cacheability prediction
//!
//! The runtime gate is authoritative for correctness, but the pass also
//! predicts at compile time which wrappers could actually pay off, so
//! rules never carry dead memo dispatch:
//!
//! - positions that evaluate under a pushed frame — iterator bodies and
//!   `try` catch arms — are neither wrapped nor counted toward the ≥ 2
//!   occurrence threshold ([`child_never_cacheable`]);
//! - classes whose occurrences are confined to mutually exclusive `if`
//!   value arms get no slot: only one arm runs per evaluation, so the
//!   memo would fill without ever being read ([`has_co_occurring_pair`]).
//!
//! Errors are never cached: the slot fills on `Ok` only, so re-evaluation
//! after a `try` caught the first occurrence's failure is deterministic.
//!
//! # Equivalence
//!
//! Occurrences are bucketed by a bottom-up structural hash (each node's
//! hash is built from its children's, in the same single post-order walk
//! that works out size, purity and iterator content) and verified with
//! strict structural equality: `1` and `1.0` are distinct (they
//! render differently), floats compare by bit pattern, object literals
//! compare order-sensitively, and the derived fields (`id`,
//! `predicate_hint`, `iter_arg_kind`, `lit`) are skipped — structurally
//! identical subtrees always carry different ids.

use std::collections::HashMap;

use std::hash::{Hash, Hasher};

use datavalue::{NumberValue, OwnedDataValue};

use crate::OpCode;
use crate::node::{
    CompiledMissingArg, CompiledMissingMin, CompiledMissingPaths, CompiledNode, CseData,
    SYNTHETIC_ID,
};

/// Minimum subtree size (node count) for a candidate that contains no
/// iterator opcode. Small repeated scalar expressions are cheaper to
/// recompute than to route through a memo slot.
const MIN_NODE_COUNT: usize = 8;

/// Run the pass over a finished compile tree. Returns the number of memo
/// slots assigned (0 = no `Cse` nodes were inserted).
pub(crate) fn apply(root: &mut CompiledNode) -> u16 {
    let mut walk = Walk::default();
    let mut choices = Vec::new();
    walk.visit(root, true, &mut choices);
    // Record in pre-order, as a top-down walk meets the candidates, so
    // slot numbering follows the rule's reading order.
    walk.candidates.sort_unstable_by_key(|c| c.preorder);
    let mut table = ClassTable::default();
    for candidate in walk.candidates {
        table.record(candidate.node, candidate.hash, candidate.choices);
    }
    table.assign_slots();
    if table.slot_count == 0 {
        return 0;
    }
    wrap(root, &table);
    table.slot_count
}

// ---------------------------------------------------------------------------
// Phase 1 — hash, bucket, and verify equivalence classes
// ---------------------------------------------------------------------------

/// An occurrence's position within the rule's exclusive-choice structure:
/// one `(if-node identity, value-arm index)` entry per enclosing `if`
/// value arm. Two occurrences can run in the same evaluation iff no
/// shared choice node maps them to different arms.
type ChoicePath = Box<[(usize, u32)]>;

/// One verified equivalence class: an owned exemplar (cloned so phase 2 can
/// mutate the tree while matching against it) plus the choice path of
/// every wrappable occurrence seen.
struct Class {
    exemplar: CompiledNode,
    occurrences: Vec<ChoicePath>,
    slot: Option<u16>,
}

#[derive(Default)]
struct ClassTable {
    /// Hash → classes with that hash (usually one; collisions are resolved
    /// by exemplar verification).
    buckets: HashMap<u64, Vec<Class>>,
    /// `(hash, index-in-bucket)` in first-seen order, so slot numbering is
    /// deterministic regardless of `HashMap` iteration order.
    order: Vec<(u64, usize)>,
    /// Each recorded occurrence, by node address, with its class. Phase 2
    /// looks occurrences up here rather than hashing every subtree again.
    /// Addresses are stable: phase 1 does not touch the tree, and phase 2
    /// moves only the node it wraps, after looking it up, while every
    /// child stays in its parent's heap allocation.
    by_node: HashMap<usize, (u64, usize)>,
    slot_count: u16,
}

impl ClassTable {
    fn record(&mut self, node: &CompiledNode, hash: u64, choices: ChoicePath) {
        let address = node as *const CompiledNode as usize;
        let bucket = self.buckets.entry(hash).or_default();
        let index = match bucket
            .iter()
            .position(|class| structural_eq(&class.exemplar, node))
        {
            Some(index) => {
                bucket[index].occurrences.push(choices);
                index
            }
            None => {
                bucket.push(Class {
                    exemplar: node.clone(),
                    occurrences: vec![choices],
                    slot: None,
                });
                self.order.push((hash, bucket.len() - 1));
                bucket.len() - 1
            }
        };
        self.by_node.insert(address, (hash, index));
    }

    fn assign_slots(&mut self) {
        for (hash, index) in &self.order {
            if self.slot_count == u16::MAX {
                break;
            }
            let class = &mut self.buckets.get_mut(hash).expect("recorded bucket")[*index];
            // A slot pays off only if two occurrences can run in the same
            // evaluation — occurrences confined to mutually exclusive `if`
            // arms would fill the memo without ever re-reading it. When a
            // co-occurring pair exists, every occurrence gets wrapped
            // (exclusive ones included: whichever arm runs still hits a
            // memo filled by a co-occurring occurrence outside the `if`).
            if class.occurrences.len() >= 2 && has_co_occurring_pair(&class.occurrences) {
                class.slot = Some(self.slot_count);
                self.slot_count += 1;
            }
        }
    }

    /// Slot for `node` if phase 1 recorded it as an occurrence of a shared
    /// class. Phase 2 visits exactly the positions phase 1 recorded from,
    /// on the untouched nodes, so the lookup by address finds them.
    fn match_slot(&self, node: &CompiledNode) -> Option<u16> {
        let (hash, index) = self.by_node.get(&(node as *const CompiledNode as usize))?;
        self.buckets.get(hash)?.get(*index)?.slot
    }
}

/// What phase 1 knows about a subtree once its children are done: the
/// facts the candidate rule and the class buckets need, each computed once
/// from the children's rather than by walking the subtree again.
#[derive(Clone, Copy)]
struct Meta {
    /// Bottom-up structural hash: the node's own fields and its children's
    /// hashes. See [`hash_local`].
    hash: u64,
    /// Node count, this node included.
    size: usize,
    /// Every node in the subtree passes [`is_cse_pure_local`].
    pure: bool,
    /// An iterator opcode appears somewhere in the subtree.
    has_iter: bool,
}

/// A candidate occurrence found by phase 1.
struct Candidate<'t> {
    node: &'t CompiledNode,
    hash: u64,
    choices: ChoicePath,
    /// Position in a pre-order walk, for recording in reading order.
    preorder: usize,
}

/// Phase 1: one post-order walk that computes every node's [`Meta`] and
/// collects the candidates.
#[derive(Default)]
struct Walk<'t> {
    candidates: Vec<Candidate<'t>>,
    preorder: usize,
    /// Child hashes of the nodes on the current path, consumed by each
    /// parent once its children are done.
    hashes: Vec<u64>,
}

impl<'t> Walk<'t> {
    /// Visit `node`, returning its [`Meta`]. `record` is false under a
    /// never-cacheable argument position: those occurrences are never
    /// wrapped, so they must not count toward the ≥ 2 threshold either,
    /// but their facts still feed the parent's. `choices` is the
    /// exclusive-choice path, so [`ClassTable::assign_slots`] can prune
    /// classes whose occurrences never co-run.
    fn visit(
        &mut self,
        node: &'t CompiledNode,
        record: bool,
        choices: &mut Vec<(usize, u32)>,
    ) -> Meta {
        // The memo wrapper is transparent; pristine trees have none.
        if let CompiledNode::Cse(data) = node {
            return self.visit(&data.inner, record, choices);
        }
        let preorder = self.preorder;
        self.preorder += 1;
        let base = self.hashes.len();
        let mut meta = Meta {
            hash: 0,
            size: 1,
            pure: is_cse_pure_local(node),
            has_iter: matches!(
                node,
                CompiledNode::BuiltinOperator { opcode, .. } if opcode.meta().is_iterator()
            ),
        };
        match node {
            CompiledNode::BuiltinOperator { opcode, args, .. } => {
                let identity = node as *const CompiledNode as usize;
                for (i, child) in args.iter().enumerate() {
                    let child_record = record && !child_never_cacheable(*opcode, i, args.len());
                    let is_choice_arm = if_value_arm(*opcode, i, args.len());
                    if is_choice_arm {
                        choices.push((identity, i as u32));
                    }
                    let child_meta = self.visit(child, child_record, choices);
                    if is_choice_arm {
                        choices.pop();
                    }
                    self.take(&mut meta, child_meta);
                }
            }
            _ => node.visit_indexed_children(&mut |_, child| {
                let child_meta = self.visit(child, record, choices);
                self.take(&mut meta, child_meta);
            }),
        }
        let mut hasher = NodeHasher::default();
        hash_local(node, &mut self.hashes[base..].iter(), &mut hasher);
        self.hashes.truncate(base);
        meta.hash = hasher.finish();
        if record && is_candidate(node, meta) {
            self.candidates.push(Candidate {
                node,
                hash: meta.hash,
                choices: choices.as_slice().into(),
                preorder,
            });
        }
        meta
    }

    /// Fold a child's facts into its parent's.
    #[inline]
    fn take(&mut self, parent: &mut Meta, child: Meta) {
        self.hashes.push(child.hash);
        parent.size += child.size;
        parent.pure &= child.pure;
        parent.has_iter |= child.has_iter;
    }
}

/// True when two occurrences with these choice paths can evaluate within
/// one evaluation: no shared `if` node routes them through different
/// value arms. (Conditions carry no arm entry — they co-occur with every
/// arm. `try` arms co-occur too: arm N runs after arm N-1 erred, and an
/// `Ok` memo filled before the error is legitimately reusable. `switch`
/// case results are conservatively treated as co-occurring.)
fn co_occurring(a: &[(usize, u32)], b: &[(usize, u32)]) -> bool {
    for (node_a, arm_a) in a {
        for (node_b, arm_b) in b {
            if node_a == node_b && arm_a != arm_b {
                return false;
            }
        }
    }
    true
}

fn has_co_occurring_pair(occurrences: &[ChoicePath]) -> bool {
    for (i, a) in occurrences.iter().enumerate() {
        for b in occurrences.iter().skip(i + 1) {
            if co_occurring(a, b) {
                return true;
            }
        }
    }
    false
}

/// Is `args[index]` of an `if` a *value* arm (as opposed to a condition)?
/// Layout: `[c1, v1, c2, v2, …, else?]` — values at odd indices plus the
/// trailing else at an even index when the arg count is odd; a single-arg
/// `if` returns its argument. Conditions evaluate on the way to whichever
/// arm is taken, so only value arms are mutually exclusive.
fn if_value_arm(opcode: OpCode, index: usize, len: usize) -> bool {
    if !matches!(opcode, OpCode::If) {
        return false;
    }
    len == 1 || index % 2 == 1 || (len % 2 == 1 && index == len - 1)
}

/// Candidate rule: a pure builtin operator that either contains an
/// iterator opcode (aggregates — the real-world target) or is at least
/// [`MIN_NODE_COUNT`] nodes.
fn is_candidate(node: &CompiledNode, meta: Meta) -> bool {
    matches!(node, CompiledNode::BuiltinOperator { .. })
        && meta.pure
        && (meta.has_iter || meta.size >= MIN_NODE_COUNT)
}

// ---------------------------------------------------------------------------
// Phase 2 — wrap occurrences
// ---------------------------------------------------------------------------

fn wrap(node: &mut CompiledNode, table: &ClassTable) {
    if let Some(slot) = table.match_slot(node) {
        // Any node works as the `mem::replace` swap-out; it is dropped
        // immediately. A Null `Value` allocates nothing, unlike the
        // `InvalidArgs` that used to sit here (which now owns a boxed
        // argument value).
        let placeholder = CompiledNode::Value {
            id: SYNTHETIC_ID,
            value: OwnedDataValue::Null,
            lit: None,
        };
        let inner = std::mem::replace(node, placeholder);
        *node = CompiledNode::Cse(Box::new(CseData { slot, inner }));
        // Keep descending so nested classes (total ⊃ net ⊃ subtotal) each
        // get their own slot inside the wrapped occurrence.
        if let CompiledNode::Cse(data) = node {
            wrap_children(&mut data.inner, table);
        }
        return;
    }
    wrap_children(node, table);
}

fn wrap_children(node: &mut CompiledNode, table: &ClassTable) {
    match node {
        CompiledNode::BuiltinOperator { opcode, args, .. } => {
            let len = args.len();
            for (i, child) in args.iter_mut().enumerate() {
                if !child_never_cacheable(*opcode, i, len) {
                    wrap(child, table);
                }
            }
        }
        _ => node.visit_children_mut(&mut |child| wrap(child, table)),
    }
}

/// Compile-time prediction of argument positions whose subtrees evaluate
/// at `depth() > 0` and could therefore never hit the runtime memo — a
/// wrapper there is pure dispatch overhead. Skipped by both phases (no
/// wrapping, and no occurrence counting toward the ≥ 2 threshold).
///
/// Delegates to [`crate::compile::scope::frames_pushed_for_child`], the
/// single source of truth for which child positions run under a pushed
/// context frame; see that function for the per-operator table and the
/// reasoning behind each entry.
///
/// The runtime `depth() == 0` gate remains authoritative — this predicate
/// is an overhead optimization, not a correctness gate.
fn child_never_cacheable(opcode: OpCode, index: usize, len: usize) -> bool {
    crate::compile::scope::frames_pushed_for_child(opcode, index, len) > 0
}

// ---------------------------------------------------------------------------
// Purity
// ---------------------------------------------------------------------------

/// The node's own part of the purity rule; a subtree is pure when every
/// node in it is. Pure means evaluating it at `depth() == 0` is a function
/// of (root data, engine config): deterministic, side-effect-free, and
/// safe to serve from a memo on repeat occurrences. Context readers
/// (`Var`, `Missing`, `Exists`) are pure *under the runtime depth gate* —
/// at depth 0 they resolve against the root.
fn is_cse_pure_local(node: &CompiledNode) -> bool {
    match node {
        CompiledNode::BuiltinOperator { opcode, .. } => opcode.meta().cse_pure(),
        // Opaque user code: may be non-deterministic, stateful, or
        // re-entrant. Never memoize.
        CompiledNode::CustomOperator(_) => false,
        // Conservative: templating output shape. Excluded per the Stage 1
        // design; can be relaxed to per-field purity later.
        #[cfg(feature = "templating")]
        CompiledNode::StructuredObject(_) => false,
        #[cfg(feature = "error-handling")]
        CompiledNode::Throw(_) => false,
        // Always errors — an Ok-only memo would never fill; wrapping is
        // pure overhead.
        CompiledNode::InvalidArgs { .. } => false,
        // Values, arrays, reads and `missing` are pure when their children
        // are; `Cse` is transparent.
        _ => true,
    }
}

// ---------------------------------------------------------------------------
// Structural hash + strict structural equality
// ---------------------------------------------------------------------------
//
/// The per-node hasher. Every node of every compiled rule is hashed once,
/// so it is a cheap multiply-rotate mix (the FxHash scheme) rather than
/// SipHash: the hashes only bucket candidates, and a collision costs one
/// [`structural_eq`] that tells the classes apart.
#[derive(Default)]
struct NodeHasher(u64);

impl NodeHasher {
    const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

    #[inline]
    fn mix(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(Self::SEED);
    }
}

impl Hasher for NodeHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            self.mix(u64::from_le_bytes(chunk.try_into().expect("8 bytes")));
        }
        let mut tail = [0u8; 8];
        let rest = chunks.remainder();
        tail[..rest.len()].copy_from_slice(rest);
        self.mix(u64::from_le_bytes(tail));
    }

    #[inline]
    fn write_u8(&mut self, n: u8) {
        self.mix(u64::from(n));
    }

    #[inline]
    fn write_u32(&mut self, n: u32) {
        self.mix(u64::from(n));
    }

    #[inline]
    fn write_u64(&mut self, n: u64) {
        self.mix(n);
    }

    #[inline]
    fn write_usize(&mut self, n: usize) {
        self.mix(n as u64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

// Invariant: `structural_eq(a, b)` ⇒ equal bottom-up hashes. Both skip the
// derived fields (`id`, `predicate_hint`, `iter_arg_kind`, `lit`) and both
// treat floats by bit pattern, so the pair stays in sync.

/// Hash `node`'s own fields, taking each child's hash from `kids` (in
/// [`CompiledNode::visit_indexed_children`] order) instead of hashing the
/// child's subtree again. A node's hash is therefore built once, from its
/// children's.
fn hash_local<H: Hasher>(node: &CompiledNode, kids: &mut std::slice::Iter<'_, u64>, h: &mut H) {
    let mut child = |h: &mut H| h.write_u64(*kids.next().expect("one hash per child"));
    match node {
        CompiledNode::Value { value, .. } => {
            h.write_u8(0);
            hash_owned(value, h);
        }
        CompiledNode::Array { nodes, .. } => {
            h.write_u8(1);
            h.write_usize(nodes.len());
            for _ in nodes.iter() {
                child(h);
            }
        }
        CompiledNode::BuiltinOperator { opcode, args, .. } => {
            h.write_u8(2);
            h.write_u8(*opcode as u8);
            h.write_usize(args.len());
            for _ in args.iter() {
                child(h);
            }
        }
        CompiledNode::CustomOperator(data) => {
            h.write_u8(3);
            data.name.hash(h);
            h.write_usize(data.args.len());
            for _ in data.args.iter() {
                child(h);
            }
        }
        // Transparent: the walk hashes the wrapped node in its place.
        CompiledNode::Cse(_) => child(h),
        #[cfg(feature = "templating")]
        CompiledNode::StructuredObject(data) => {
            h.write_u8(4);
            h.write_usize(data.fields.len());
            data.escape.hash(h);
            for (key, _) in data.fields.iter() {
                key.hash(h);
                child(h);
            }
        }
        CompiledNode::Var {
            scope_level,
            segments,
            reduce_hint,
            metadata_hint,
            default_value,
            ..
        } => {
            h.write_u8(5);
            h.write_u32(*scope_level);
            segments.hash(h);
            reduce_hint.hash(h);
            metadata_hint.hash(h);
            match default_value {
                Option::Some(_) => {
                    h.write_u8(1);
                    child(h);
                }
                Option::None => h.write_u8(0),
            }
        }
        #[cfg(feature = "ext-control")]
        CompiledNode::Exists(data) => {
            h.write_u8(6);
            h.write_u32(data.scope_level);
            data.segments.hash(h);
        }
        #[cfg(feature = "error-handling")]
        CompiledNode::Throw(data) => {
            h.write_u8(7);
            hash_owned(&data.error, h);
        }
        CompiledNode::Missing(data) => {
            h.write_u8(8);
            h.write_usize(data.args.len());
            for arg in data.args.iter() {
                match arg {
                    CompiledMissingArg::Now((path, _)) => {
                        h.write_u8(0);
                        path.hash(h);
                    }
                    CompiledMissingArg::Later(_) => {
                        h.write_u8(1);
                        child(h);
                    }
                }
            }
        }
        CompiledNode::MissingSome(data) => {
            h.write_u8(9);
            match &data.min_present {
                CompiledMissingMin::Now(n) => {
                    h.write_u8(0);
                    h.write_usize(*n);
                }
                CompiledMissingMin::Later(_) => {
                    h.write_u8(1);
                    child(h);
                }
            }
            match &data.paths {
                CompiledMissingPaths::Now(paths) => {
                    h.write_u8(0);
                    h.write_usize(paths.len());
                    for (path, _) in paths.iter() {
                        path.hash(h);
                    }
                }
                CompiledMissingPaths::Later(_) => {
                    h.write_u8(1);
                    child(h);
                }
            }
        }
        CompiledNode::InvalidArgs { op_name, args, .. } => {
            h.write_u8(10);
            op_name.hash(h);
            hash_owned(args, h);
        }
    }
}

/// Strict value hash: `Integer(1)` and `Float(1.0)` hash differently,
/// floats hash by bit pattern, objects hash in field order.
fn hash_owned<H: Hasher>(value: &OwnedDataValue, h: &mut H) {
    match value {
        OwnedDataValue::Null => h.write_u8(0),
        OwnedDataValue::Bool(b) => {
            h.write_u8(1);
            h.write_u8(u8::from(*b));
        }
        OwnedDataValue::Number(NumberValue::Integer(i)) => {
            h.write_u8(2);
            h.write_i64(*i);
        }
        OwnedDataValue::Number(NumberValue::Float(f)) => {
            h.write_u8(3);
            h.write_u64(f.to_bits());
        }
        OwnedDataValue::String(s) => {
            h.write_u8(4);
            s.hash(h);
        }
        OwnedDataValue::Array(items) => {
            h.write_u8(5);
            h.write_usize(items.len());
            for item in items {
                hash_owned(item, h);
            }
        }
        OwnedDataValue::Object(fields) => {
            h.write_u8(6);
            h.write_usize(fields.len());
            for (key, item) in fields {
                key.hash(h);
                hash_owned(item, h);
            }
        }
        #[cfg(feature = "datetime")]
        OwnedDataValue::DateTime(d) => {
            h.write_u8(7);
            format!("{d:?}").hash(h);
        }
        #[cfg(feature = "datetime")]
        OwnedDataValue::Duration(d) => {
            h.write_u8(8);
            format!("{d:?}").hash(h);
        }
        // `DataTensor`'s `Debug` is deliberately lossy (dtype, shape, byte
        // count — never the payload), so unlike the datetime arms above
        // this one cannot go through `format!`: two different tensors of
        // the same shape would collide. Hash what `PartialEq` compares.
        #[cfg(feature = "tensor")]
        OwnedDataValue::Tensor(t) => {
            h.write_u8(9);
            t.dtype().name().hash(h);
            t.shape().hash(h);
            t.data().hash(h);
        }
    }
}

fn structural_eq(a: &CompiledNode, b: &CompiledNode) -> bool {
    match (a, b) {
        (CompiledNode::Value { value: va, .. }, CompiledNode::Value { value: vb, .. }) => {
            owned_eq(va, vb)
        }
        (CompiledNode::Array { nodes: na, .. }, CompiledNode::Array { nodes: nb, .. }) => {
            na.len() == nb.len() && na.iter().zip(nb.iter()).all(|(x, y)| structural_eq(x, y))
        }
        (
            CompiledNode::BuiltinOperator {
                opcode: oa,
                args: aa,
                ..
            },
            CompiledNode::BuiltinOperator {
                opcode: ob,
                args: ab,
                ..
            },
        ) => {
            oa == ob
                && aa.len() == ab.len()
                && aa.iter().zip(ab.iter()).all(|(x, y)| structural_eq(x, y))
        }
        (CompiledNode::CustomOperator(da), CompiledNode::CustomOperator(db)) => {
            da.name == db.name
                && da.args.len() == db.args.len()
                && da
                    .args
                    .iter()
                    .zip(db.args.iter())
                    .all(|(x, y)| structural_eq(x, y))
        }
        (CompiledNode::Cse(da), CompiledNode::Cse(db)) => {
            da.slot == db.slot && structural_eq(&da.inner, &db.inner)
        }
        #[cfg(feature = "templating")]
        (CompiledNode::StructuredObject(da), CompiledNode::StructuredObject(db)) => {
            da.escape == db.escape
                && da.fields.len() == db.fields.len()
                && da
                    .fields
                    .iter()
                    .zip(db.fields.iter())
                    .all(|((ka, na), (kb, nb))| ka == kb && structural_eq(na, nb))
        }
        (
            CompiledNode::Var {
                scope_level: sa,
                segments: ga,
                reduce_hint: ra,
                metadata_hint: ma,
                default_value: da,
                ..
            },
            CompiledNode::Var {
                scope_level: sb,
                segments: gb,
                reduce_hint: rb,
                metadata_hint: mb,
                default_value: db,
                ..
            },
        ) => {
            sa == sb
                && ga == gb
                && ra == rb
                && ma == mb
                && match (da, db) {
                    (Option::None, Option::None) => true,
                    (Option::Some(x), Option::Some(y)) => structural_eq(x, y),
                    _ => false,
                }
        }
        #[cfg(feature = "ext-control")]
        (CompiledNode::Exists(da), CompiledNode::Exists(db)) => {
            da.scope_level == db.scope_level && da.segments == db.segments
        }
        #[cfg(feature = "error-handling")]
        (CompiledNode::Throw(da), CompiledNode::Throw(db)) => owned_eq(&da.error, &db.error),
        (CompiledNode::Missing(da), CompiledNode::Missing(db)) => {
            da.args.len() == db.args.len()
                && da
                    .args
                    .iter()
                    .zip(db.args.iter())
                    .all(|(x, y)| missing_arg_eq(x, y))
        }
        (CompiledNode::MissingSome(da), CompiledNode::MissingSome(db)) => {
            let min_eq = match (&da.min_present, &db.min_present) {
                (CompiledMissingMin::Now(x), CompiledMissingMin::Now(y)) => x == y,
                (CompiledMissingMin::Later(x), CompiledMissingMin::Later(y)) => structural_eq(x, y),
                _ => false,
            };
            let paths_eq = match (&da.paths, &db.paths) {
                (CompiledMissingPaths::Now(x), CompiledMissingPaths::Now(y)) => {
                    x.len() == y.len() && x.iter().zip(y.iter()).all(|((pa, _), (pb, _))| pa == pb)
                }
                (CompiledMissingPaths::Later(x), CompiledMissingPaths::Later(y)) => {
                    structural_eq(x, y)
                }
                _ => false,
            };
            min_eq && paths_eq
        }
        (
            CompiledNode::InvalidArgs {
                op_name: na,
                args: aa,
                ..
            },
            CompiledNode::InvalidArgs {
                op_name: nb,
                args: ab,
                ..
            },
        ) => na == nb && aa == ab,
        _ => false,
    }
}

fn missing_arg_eq(a: &CompiledMissingArg, b: &CompiledMissingArg) -> bool {
    match (a, b) {
        (CompiledMissingArg::Now((pa, _)), CompiledMissingArg::Now((pb, _))) => pa == pb,
        (CompiledMissingArg::Later(x), CompiledMissingArg::Later(y)) => structural_eq(x, y),
        _ => false,
    }
}

/// Strict value equality, mirroring [`hash_owned`]: `Integer(1)` ≠
/// `Float(1.0)`, floats compare by bit pattern, objects compare in field
/// order.
fn owned_eq(a: &OwnedDataValue, b: &OwnedDataValue) -> bool {
    match (a, b) {
        (OwnedDataValue::Null, OwnedDataValue::Null) => true,
        (OwnedDataValue::Bool(x), OwnedDataValue::Bool(y)) => x == y,
        (
            OwnedDataValue::Number(NumberValue::Integer(x)),
            OwnedDataValue::Number(NumberValue::Integer(y)),
        ) => x == y,
        (
            OwnedDataValue::Number(NumberValue::Float(x)),
            OwnedDataValue::Number(NumberValue::Float(y)),
        ) => x.to_bits() == y.to_bits(),
        (OwnedDataValue::String(x), OwnedDataValue::String(y)) => x == y,
        (OwnedDataValue::Array(x), OwnedDataValue::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(a, b)| owned_eq(a, b))
        }
        (OwnedDataValue::Object(x), OwnedDataValue::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y.iter())
                    .all(|((ka, va), (kb, vb))| ka == kb && owned_eq(va, vb))
        }
        #[cfg(feature = "datetime")]
        (OwnedDataValue::DateTime(x), OwnedDataValue::DateTime(y)) => {
            format!("{x:?}") == format!("{y:?}")
        }
        #[cfg(feature = "datetime")]
        (OwnedDataValue::Duration(x), OwnedDataValue::Duration(y)) => {
            format!("{x:?}") == format!("{y:?}")
        }
        // datavalue's `PartialEq` is structural (dtype, shape, bytes),
        // which is exactly the equality `hash_owned` above hashes.
        #[cfg(feature = "tensor")]
        (OwnedDataValue::Tensor(x), OwnedDataValue::Tensor(y)) => x == y,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;

    /// A pure aggregate over `items` — the canonical CSE candidate.
    const AGG: &str =
        r#"{"reduce": [{"var": "items"}, {"+": [{"var": "accumulator"}, {"var": "current"}]}, 0]}"#;

    fn slot_count(rule: &str) -> u16 {
        Engine::new().compile(rule).unwrap().cse_slot_count
    }

    #[test]
    fn repeated_aggregate_shares_one_slot() {
        let rule = format!(r#"{{"+": [{AGG}, {AGG}]}}"#);
        assert_eq!(slot_count(&rule), 1);
    }

    #[test]
    fn single_occurrence_gets_no_slot() {
        assert_eq!(slot_count(AGG), 0);
    }

    #[test]
    fn small_scalar_repeats_get_no_slot() {
        // Pure and repeated, but no iterator and < MIN_NODE_COUNT nodes.
        let rule = r#"{"+": [{"*": [{"var": "x"}, 2]}, {"*": [{"var": "x"}, 2]}]}"#;
        assert_eq!(slot_count(rule), 0);
    }

    #[test]
    fn integer_and_float_near_twins_do_not_share() {
        // Initial accumulator 0 vs 0.0: strict equality must keep the two
        // classes apart (they render — and can evaluate — differently).
        let agg_float = AGG.replace(", 0]", ", 0.0]");
        let rule = format!(r#"{{"+": [{AGG}, {AGG}, {agg_float}, {agg_float}]}}"#);
        assert_eq!(slot_count(&rule), 2);
    }

    #[test]
    fn iterator_body_occurrences_neither_count_nor_wrap() {
        // One occurrence at root, one inside a map body: the body one is
        // ineligible, so no class reaches the ≥ 2 threshold.
        let rule = format!(
            r#"{{"+": [{AGG}, {{"reduce": [{{"map": [{{"var": "xs"}}, {AGG}]}}, {{"+": [{{"var": "accumulator"}}, {{"var": "current"}}]}}, 0]}}]}}"#
        );
        assert_eq!(slot_count(&rule), 0);
    }

    /// `group_by` / keyed `distinct` key expressions run under per-item
    /// frames — same ineligibility as the iterator bodies above.
    #[cfg(feature = "ext-array")]
    #[test]
    fn key_expr_occurrences_neither_count_nor_wrap() {
        for op in ["group_by", "distinct"] {
            let rule = format!(
                r#"{{"+": [{AGG}, {{"reduce": [{{"{op}": [{{"var": "xs"}}, {AGG}]}}, {{"+": [{{"var": "accumulator"}}, 1]}}, 0]}}]}}"#
            );
            assert_eq!(slot_count(&rule), 0, "key expr of {op} must be ineligible");
        }
    }

    #[cfg(feature = "error-handling")]
    #[test]
    fn try_catch_arm_occurrences_neither_count_nor_wrap() {
        // The catch arm runs under the caught-error frame — a memo
        // wrapper there could never hit. One protected + one catch
        // occurrence must not form a class.
        let rule = format!(r#"{{"try": [{AGG}, {{"+": [{AGG}, 1]}}]}}"#);
        assert_eq!(slot_count(&rule), 0);
    }

    #[cfg(feature = "error-handling")]
    #[test]
    fn try_protected_arms_share() {
        // Occurrences inside a protected arm evaluate at depth 0 and
        // stay eligible.
        let rule = format!(r#"{{"try": [{{"+": [{AGG}, {AGG}]}}, 0]}}"#);
        assert_eq!(slot_count(&rule), 1);
    }

    #[test]
    fn mutually_exclusive_if_arms_get_no_slot() {
        // Only one value arm runs per evaluation — a shared slot would
        // fill without ever being read.
        let rule = format!(r#"{{"if": [{{"var": "c"}}, {AGG}, {AGG}]}}"#);
        assert_eq!(slot_count(&rule), 0);
    }

    #[test]
    fn if_condition_co_occurs_with_taken_arm() {
        // The condition evaluates on the way to the arm, so these two
        // occurrences can run in one evaluation and share.
        let rule = format!(r#"{{"if": [{{">": [{AGG}, 10]}}, {AGG}, 0]}}"#);
        assert_eq!(slot_count(&rule), 1);
    }

    #[test]
    fn exclusive_arms_still_share_with_a_root_occurrence() {
        // A co-occurring pair (root + either arm) keeps the class; the
        // exclusive arm occurrences are wrapped too and hit the memo the
        // root occurrence filled.
        let rule = format!(r#"{{"+": [{AGG}, {{"if": [{{"var": "c"}}, {AGG}, {AGG}]}}]}}"#);
        assert_eq!(slot_count(&rule), 1);
    }

    #[test]
    fn no_fold_compile_produces_no_slots() {
        let engine = Engine::builder().with_constant_folding(false).build();
        let rule = format!(r#"{{"+": [{AGG}, {AGG}]}}"#);
        assert_eq!(engine.compile(rule.as_str()).unwrap().cse_slot_count, 0);
    }

    #[test]
    fn custom_truthy_evaluator_disables_the_pass() {
        let config = crate::EvaluationConfig::default().with_truthy_evaluator(
            crate::TruthyEvaluator::Custom(std::sync::Arc::new(|_| true)),
        );
        let engine = Engine::builder().with_config(config).build();
        let rule = format!(r#"{{"+": [{AGG}, {AGG}]}}"#);
        assert_eq!(engine.compile(rule.as_str()).unwrap().cse_slot_count, 0);
    }

    #[test]
    fn impure_subtrees_are_not_candidates() {
        // A custom-operator name inside the subtree disqualifies it even
        // though the surrounding shape repeats.
        let agg = r#"{"reduce": [{"var": "items"}, {"my_op": [{"var": "current"}]}, 0]}"#;
        let rule = format!(r#"{{"+": [{agg}, {agg}]}}"#);
        assert_eq!(slot_count(&rule), 0);
    }

    #[test]
    fn nested_classes_get_their_own_slots() {
        // AGG (3 occurrences) and {"*":[AGG,2]} (2 occurrences) both share.
        let rule = format!(r#"{{"+": [{AGG}, {{"*": [{AGG}, 2]}}, {{"*": [{AGG}, 2]}}]}}"#);
        assert_eq!(slot_count(&rule), 2);
    }

    #[test]
    fn serialization_is_wrapper_transparent() {
        let engine = Engine::new();
        let rule = format!(r#"{{"+": [{AGG}, {AGG}]}}"#);
        let with_cse = engine.compile(rule.as_str()).unwrap();
        assert_eq!(with_cse.cse_slot_count, 1);
        let no_cse_engine = Engine::builder().with_constant_folding(false).build();
        let without_cse = no_cse_engine.compile(rule.as_str()).unwrap();
        assert_eq!(with_cse.to_json(), without_cse.to_json());
    }

    #[test]
    fn memo_does_not_leak_across_evaluations() {
        let engine = Engine::new();
        let rule = format!(r#"{{"+": [{AGG}, {AGG}]}}"#);
        let compiled = engine.compile(rule.as_str()).unwrap();
        assert_eq!(compiled.cse_slot_count, 1);
        let mut session = engine.session();
        assert_eq!(
            session
                .eval_str(&compiled, r#"{"items": [1, 2, 3]}"#)
                .unwrap(),
            "12"
        );
        session.reset();
        assert_eq!(
            session.eval_str(&compiled, r#"{"items": [10]}"#).unwrap(),
            "20"
        );
    }

    #[cfg(feature = "trace")]
    #[test]
    fn traced_eval_of_cse_compiled_logic_matches_plain_eval() {
        // A Logic compiled WITH Cse nodes can be run under a tracer
        // (`TracedSession::eval` does not recompile) — the runtime
        // `is_tracing` gate must bypass the memo and produce the same
        // value with full per-occurrence trace coverage.
        let engine = Engine::new();
        let rule = format!(r#"{{"+": [{AGG}, {AGG}]}}"#);
        let compiled = engine.compile(rule.as_str()).unwrap();
        assert_eq!(compiled.cse_slot_count, 1);
        let plain = engine.eval_str(rule.as_str(), r#"{"items": [1, 2, 3]}"#);
        let traced = engine.trace().eval(&compiled, r#"{"items": [1, 2, 3]}"#);
        assert_eq!(
            plain.unwrap(),
            traced.result.unwrap().to_json_string(),
            "traced and plain evaluation of a CSE'd tree must agree"
        );
    }

    #[test]
    fn object_literals_compare_order_sensitively() {
        use datavalue::OwnedDataValue as V;
        let ab = V::Object(vec![
            ("a".into(), V::Bool(true)),
            ("b".into(), V::Bool(false)),
        ]);
        let ba = V::Object(vec![
            ("b".into(), V::Bool(false)),
            ("a".into(), V::Bool(true)),
        ]);
        assert!(!super::owned_eq(&ab, &ba));
        assert!(super::owned_eq(&ab, &ab.clone()));
    }

    #[test]
    fn integer_and_float_values_are_distinct() {
        use datavalue::{NumberValue, OwnedDataValue as V};
        let int_one = V::Number(NumberValue::Integer(1));
        let float_one = V::Number(NumberValue::Float(1.0));
        assert!(!super::owned_eq(&int_one, &float_one));
    }

    /// Every `Cse` wrapper must sit at static frame depth 0. `dispatch_cse`
    /// relies on this to skip the runtime depth probe: a memo consulted under
    /// an iteration frame would serve a value resolved against a different
    /// context. `collect` and `wrap_children` both skip frame-pushing
    /// positions, so this holds by construction — pinned here so a future
    /// relaxation has to confront it.
    #[test]
    fn no_cse_node_sits_under_a_pushed_frame() {
        use crate::compile::scope::frames_pushed_for_child;
        use crate::node::CompiledNode;

        fn walk(node: &CompiledNode, depth: u32) {
            if let CompiledNode::Cse(data) = node {
                assert_eq!(depth, 0, "Cse wrapper found at static frame depth {depth}");
                walk(&data.inner, depth);
                return;
            }
            if let CompiledNode::BuiltinOperator { opcode, args, .. } = node {
                let len = args.len();
                for (i, child) in args.iter().enumerate() {
                    walk(child, depth + frames_pushed_for_child(*opcode, i, len));
                }
                return;
            }
            node.visit_indexed_children(&mut |_, child| walk(child, depth));
        }

        let engine = Engine::new();
        for rule in [
            AGG,
            r#"{"map": [{"val": "xs"}, {"reduce": [{"val": "ys"}, {"+": [{"val": "current"}, {"val": "accumulator"}]}, 0]}]}"#,
            r#"{"if": [{"val": "c"}, {"reduce": [{"val": "xs"}, {"+": [{"val": "current"}, {"val": "accumulator"}]}, 0]}, {"reduce": [{"val": "xs"}, {"+": [{"val": "current"}, {"val": "accumulator"}]}, 0]}]}"#,
        ] {
            let logic = engine.compile(rule).unwrap();
            walk(&logic.root, 0);
        }
    }
}
