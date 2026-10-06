//! Public description of the built-in operators, read from the operator
//! table. Backs [`crate::Engine::operators`].

use super::meta::{Frames, OpMeta};
use crate::OpCode;

/// One built-in operator, as its operator-table row declares it.
///
/// Returned by [`crate::Engine::operators`]. Every field is a build-time
/// fact: the same rows drive compilation, dispatch and the optimizer, so
/// this description cannot drift from what the engine does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct OperatorInfo {
    /// Canonical name (`"val"`, `"if"`, `"switch"`).
    pub name: &'static str,
    /// Other names the compiler accepts for the same operator (`"var"`,
    /// `"?:"`, `"match"`).
    pub aliases: &'static [&'static str],
    /// Operator family (`"Core"`, `"DateTime"`, `"ExtString"`, ...).
    pub family: &'static str,
    /// The Cargo feature that gates the family, or `None` for the core
    /// JSONLogic operators.
    pub feature: Option<&'static str>,
    /// Fewest arguments the operator's signature reads. Operators that
    /// check their own argument lists (variadics, lazy and control-flow
    /// operators, iterators) report `0`.
    pub min_args: usize,
    /// Most arguments the operator's signature reads, or `None` when it
    /// does not declare a maximum.
    pub max_args: Option<usize>,
    /// Whether the operator reads the data context (`val`, `missing`,
    /// `exists`, `fractional`).
    pub reads_context: bool,
    /// What evaluating the operator does besides computing a value:
    /// `"pure"`, `"clock"` (`now`), `"throws"` (`throw`) or `"catches"`
    /// (`try`).
    pub effect: &'static str,
    /// What the operator's work is proportional to: `"node"` (constant),
    /// `"bytes"`, `"per_item"`, `"n_log_n"`, `"quadratic"` or `"elements"`.
    pub cost: &'static str,
    /// Which argument, if any, runs under a pushed context frame (an
    /// iterator body, a key expression, a catch arm).
    pub scoped_arg: Option<ScopedArg>,
}

/// An argument position that runs under a pushed context frame, where
/// `val` paths resolve against the current element (or the caught error)
/// rather than the enclosing data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScopedArg {
    /// The argument at this index (`map`'s body is `Index(1)`, `sort`'s
    /// key expression `Index(2)`).
    Index(usize),
    /// The last argument, when there are at least two (`try`'s catch arm).
    LastOfMany,
}

impl OperatorInfo {
    pub(crate) fn of(op: OpCode) -> Option<Self> {
        let entry = op.catalogue_entry();
        let (name, aliases) = entry.names.split_first()?;
        let meta: &OpMeta = op.meta();
        let arity = op.arity();
        Some(OperatorInfo {
            name,
            aliases,
            family: entry.family,
            feature: entry.feature(),
            min_args: arity.min as usize,
            max_args: arity.max.map(usize::from),
            reads_context: meta.reads_context,
            effect: meta.effect.name(),
            cost: meta.cost.name(),
            scoped_arg: match meta.frames {
                Frames::None => None,
                Frames::At(i) => Some(ScopedArg::Index(i as usize)),
                Frames::LastIfMulti => Some(ScopedArg::LastOfMany),
            },
        })
    }
}

/// Every operator of the families in `families` (a set of family bits)
/// compiled into this build that has a name, in table order. Internal
/// opcodes (no name) are left out.
pub(crate) fn operators_in(families: u32) -> impl Iterator<Item = OperatorInfo> {
    OpCode::ALL
        .iter()
        .filter(move |op| families & op.family().bit() != 0)
        .filter_map(|op| OperatorInfo::of(*op))
}
