//! OpCode-based dispatch system for built-in operators.
//!
//! This module implements a high-performance dispatch mechanism using enum variants
//! instead of string matching or vtable lookups at runtime.
//!
//! # Performance Design
//!
//! The `OpCode` enum provides O(1) operator dispatch through:
//!
//! 1. **Compile-time resolution**: Operator strings are converted to `OpCode` variants
//!    during the compilation phase, not during evaluation
//! 2. **Direct dispatch**: [`crate::engine::dispatch::dispatch_node_inner`] is a
//!    table-driven match over `OpCode`; the compiler lowers it to a jump table
//! 3. **No boxing or vtables**: Direct function calls without trait object overhead
//! 4. **Cache-friendly**: The `#[repr(u8)]` attribute ensures compact memory layout
//!
//! # Operator Categories
//!
//! Operators are grouped by functionality and feature-gated:
//!
//! - **Core** (always available):
//!   - Variable Access: `val` (canonical; `var` is accepted as input and
//!     normalized to `val` at compile time)
//!   - Comparison: `==`, `===`, `!=`, `!==`, `>`, `>=`, `<`, `<=`
//!   - Logical: `!`, `!!`, `and`, `or`
//!   - Control Flow: `if` (canonical; `?:` is accepted as input and normalized
//!     to `if` at compile time)
//!   - Arithmetic: `+`, `-`, `*`, `/`, `%`, `max`, `min`
//!   - String: `cat`, `substr`, `in`
//!   - Array: `merge`, `filter`, `map`, `reduce`, `all`, `some`, `none`
//!   - Missing: `missing`, `missing_some`
//! - **datetime**: `datetime`, `timestamp`, `parse_date`, `format_date`, `date_diff`, `now`
//! - **ext-string**: `length`, `starts_with`, `ends_with`, `upper`, `lower`, `trim`, `split`
//! - **ext-array**: `sort`, `slice`, `group_by`, `distinct`
//! - **ext-object**: `keys`, `values`, `entries`
//! - **ext-control**: `exists`, `??`, `switch`/`match`, `type`
//! - **error-handling**: `try`, `throw`
//! - **ext-math**: `abs`, `ceil`, `floor`
//! - **tensor**: `tensor`, `zeros`, `full`, `scatter`, `rle_expand`,
//!   `one_hot`, `stack`, `concat`, `unstack`, `reshape`, `transpose`,
//!   `pad`, `crop`, `cast`, `normalize`, `argmax`, `gather`, `to_list`,
//!   `shape`, `dtype`
//! - **flagd** ([spec](https://flagd.dev/reference/custom-operations/)):
//!   `fractional` (murmurhash3 percentage bucketing), `sem_ver`
//!   (semantic-version comparison with flagd-spec normalizations)
//!
//! # Adding New Operators
//!
//! 1. Add a new variant to the [`OpCode`] enum
//! 2. Add an entry (canonical name first, then any aliases) to [`OPCODE_NAMES`]
//! 3. Add the dispatch arm in `src/engine/dispatch.rs`
//! 4. Implement the operator function in the appropriate `src/operators/` module
//!
//! No further step is needed for introspection:
//! [`crate::Engine::builtin_operator_names`] is derived from
//! [`OPCODE_NAMES`], so a new entry is reported automatically.

use std::str::FromStr;

/// OpCode enum for fast built-in operator lookup
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpCode {
    // === Core: Variable Access ===
    Val = 1,

    // === Core: Comparison Operators ===
    Equals = 2,
    StrictEquals = 3,
    NotEquals = 4,
    StrictNotEquals = 5,
    GreaterThan = 6,
    GreaterThanEqual = 7,
    LessThan = 8,
    LessThanEqual = 9,

    // === Core: Logical Operators ===
    Not = 10,
    BoolCast = 11,
    And = 12,
    Or = 13,

    // === Core: Control Flow ===
    If = 14,

    // === Core: Arithmetic Operators ===
    Add = 16,
    Subtract = 17,
    Multiply = 18,
    Divide = 19,
    Modulo = 20,
    Max = 21,
    Min = 22,

    // === Core: String Operations ===
    Concat = 23,
    Substr = 24,
    In = 25,

    // === Core: Array Operations ===
    Merge = 26,
    Filter = 27,
    Map = 28,
    Reduce = 29,
    All = 30,
    Some = 31,
    None = 32,

    // === Core: Missing Value Handling ===
    Missing = 33,
    MissingSome = 34,

    // === datetime ===
    #[cfg(feature = "datetime")]
    Datetime = 44,
    #[cfg(feature = "datetime")]
    Timestamp = 45,
    #[cfg(feature = "datetime")]
    ParseDate = 46,
    #[cfg(feature = "datetime")]
    FormatDate = 47,
    #[cfg(feature = "datetime")]
    DateDiff = 48,
    #[cfg(feature = "datetime")]
    Now = 58,

    // === ext-string ===
    #[cfg(feature = "ext-string")]
    Length = 53,
    #[cfg(feature = "ext-string")]
    StartsWith = 38,
    #[cfg(feature = "ext-string")]
    EndsWith = 39,
    #[cfg(feature = "ext-string")]
    Upper = 40,
    #[cfg(feature = "ext-string")]
    Lower = 41,
    #[cfg(feature = "ext-string")]
    Trim = 42,
    #[cfg(feature = "ext-string")]
    Split = 43,

    // === ext-array ===
    #[cfg(feature = "ext-array")]
    Sort = 54,
    #[cfg(feature = "ext-array")]
    Slice = 55,
    #[cfg(feature = "ext-array")]
    GroupBy = 62,
    #[cfg(feature = "ext-array")]
    Distinct = 63,

    // === ext-object ===
    #[cfg(feature = "ext-object")]
    Keys = 64,
    #[cfg(feature = "ext-object")]
    Values = 65,
    #[cfg(feature = "ext-object")]
    Entries = 66,

    // === ext-control ===
    #[cfg(feature = "ext-control")]
    Exists = 57,
    #[cfg(feature = "ext-control")]
    Coalesce = 56,
    #[cfg(feature = "ext-control")]
    Switch = 59,
    #[cfg(feature = "ext-control")]
    Type = 37,

    // === error-handling ===
    #[cfg(feature = "error-handling")]
    Try = 35,
    #[cfg(feature = "error-handling")]
    Throw = 36,

    // === ext-math ===
    #[cfg(feature = "ext-math")]
    Abs = 49,
    #[cfg(feature = "ext-math")]
    Ceil = 50,
    #[cfg(feature = "ext-math")]
    Floor = 51,

    // === flagd ===
    #[cfg(feature = "flagd")]
    Fractional = 60,
    #[cfg(feature = "flagd")]
    SemVer = 61,

    // === tensor ===
    // Variant names carry a `Tensor` prefix so they never collide with an
    // existing opcode (`Concat` is already `cat`, `Type` is already
    // `type`); the wire names below are the bare ones.
    #[cfg(feature = "tensor")]
    TensorMake = 67,
    #[cfg(feature = "tensor")]
    TensorZeros = 68,
    #[cfg(feature = "tensor")]
    TensorFull = 69,
    #[cfg(feature = "tensor")]
    TensorScatter = 70,
    #[cfg(feature = "tensor")]
    TensorRleExpand = 71,
    #[cfg(feature = "tensor")]
    TensorOneHot = 72,
    #[cfg(feature = "tensor")]
    TensorStack = 73,
    #[cfg(feature = "tensor")]
    TensorConcat = 74,
    #[cfg(feature = "tensor")]
    TensorUnstack = 75,
    #[cfg(feature = "tensor")]
    TensorReshape = 76,
    #[cfg(feature = "tensor")]
    TensorTranspose = 77,
    #[cfg(feature = "tensor")]
    TensorPad = 78,
    #[cfg(feature = "tensor")]
    TensorCrop = 79,
    #[cfg(feature = "tensor")]
    TensorCast = 80,
    #[cfg(feature = "tensor")]
    TensorNormalize = 81,
    #[cfg(feature = "tensor")]
    TensorArgmax = 82,
    #[cfg(feature = "tensor")]
    TensorGather = 83,
    #[cfg(feature = "tensor")]
    TensorToList = 84,
    #[cfg(feature = "tensor")]
    TensorShape = 85,
    #[cfg(feature = "tensor")]
    TensorDtype = 86,
}

/// Single source of truth for `(operator string, OpCode)` mappings.
///
/// The first entry per opcode is the canonical name returned by
/// [`OpCode::as_str`]; subsequent entries with the same opcode are accepted
/// as aliases by [`OpCode::from_str`]. Feature-gated entries follow the
/// same `#[cfg]` as their corresponding [`OpCode`] variant.
const OPCODE_NAMES: &[(&str, OpCode)] = &[
    // Core: variable access. `var` is accepted as a synonym of `val` —
    // both normalize to OpCode::Val. The compile pipeline (`try_specialised`)
    // dispatches the appropriate compile-time specialiser based on the source
    // operator name.
    ("val", OpCode::Val),
    ("var", OpCode::Val),
    // Core: comparison
    ("==", OpCode::Equals),
    ("===", OpCode::StrictEquals),
    ("!=", OpCode::NotEquals),
    ("!==", OpCode::StrictNotEquals),
    (">", OpCode::GreaterThan),
    (">=", OpCode::GreaterThanEqual),
    ("<", OpCode::LessThan),
    ("<=", OpCode::LessThanEqual),
    // Core: logical
    ("!", OpCode::Not),
    ("!!", OpCode::BoolCast),
    ("and", OpCode::And),
    ("or", OpCode::Or),
    // Core: control flow. `?:` is accepted as a synonym of `if` — both
    // normalize to OpCode::If. `evaluate_if` already handles the 3-arg case
    // identically to a ternary.
    ("if", OpCode::If),
    ("?:", OpCode::If),
    // Core: arithmetic
    ("+", OpCode::Add),
    ("-", OpCode::Subtract),
    ("*", OpCode::Multiply),
    ("/", OpCode::Divide),
    ("%", OpCode::Modulo),
    ("max", OpCode::Max),
    ("min", OpCode::Min),
    // Core: string
    ("cat", OpCode::Concat),
    ("substr", OpCode::Substr),
    ("in", OpCode::In),
    // Core: array
    ("merge", OpCode::Merge),
    ("filter", OpCode::Filter),
    ("map", OpCode::Map),
    ("reduce", OpCode::Reduce),
    ("all", OpCode::All),
    ("some", OpCode::Some),
    ("none", OpCode::None),
    // Core: missing
    ("missing", OpCode::Missing),
    ("missing_some", OpCode::MissingSome),
    // datetime
    #[cfg(feature = "datetime")]
    ("datetime", OpCode::Datetime),
    #[cfg(feature = "datetime")]
    ("timestamp", OpCode::Timestamp),
    #[cfg(feature = "datetime")]
    ("parse_date", OpCode::ParseDate),
    #[cfg(feature = "datetime")]
    ("format_date", OpCode::FormatDate),
    #[cfg(feature = "datetime")]
    ("date_diff", OpCode::DateDiff),
    #[cfg(feature = "datetime")]
    ("now", OpCode::Now),
    // ext-string
    #[cfg(feature = "ext-string")]
    ("length", OpCode::Length),
    #[cfg(feature = "ext-string")]
    ("starts_with", OpCode::StartsWith),
    #[cfg(feature = "ext-string")]
    ("ends_with", OpCode::EndsWith),
    #[cfg(feature = "ext-string")]
    ("upper", OpCode::Upper),
    #[cfg(feature = "ext-string")]
    ("lower", OpCode::Lower),
    #[cfg(feature = "ext-string")]
    ("trim", OpCode::Trim),
    #[cfg(feature = "ext-string")]
    ("split", OpCode::Split),
    // ext-array
    #[cfg(feature = "ext-array")]
    ("sort", OpCode::Sort),
    #[cfg(feature = "ext-array")]
    ("slice", OpCode::Slice),
    #[cfg(feature = "ext-array")]
    ("group_by", OpCode::GroupBy),
    #[cfg(feature = "ext-array")]
    ("distinct", OpCode::Distinct),
    // ext-object
    #[cfg(feature = "ext-object")]
    ("keys", OpCode::Keys),
    #[cfg(feature = "ext-object")]
    ("values", OpCode::Values),
    #[cfg(feature = "ext-object")]
    ("entries", OpCode::Entries),
    // ext-control
    #[cfg(feature = "ext-control")]
    ("exists", OpCode::Exists),
    #[cfg(feature = "ext-control")]
    ("??", OpCode::Coalesce),
    #[cfg(feature = "ext-control")]
    ("switch", OpCode::Switch),
    #[cfg(feature = "ext-control")]
    ("match", OpCode::Switch),
    #[cfg(feature = "ext-control")]
    ("type", OpCode::Type),
    // error-handling
    #[cfg(feature = "error-handling")]
    ("try", OpCode::Try),
    #[cfg(feature = "error-handling")]
    ("throw", OpCode::Throw),
    // ext-math
    #[cfg(feature = "ext-math")]
    ("abs", OpCode::Abs),
    #[cfg(feature = "ext-math")]
    ("ceil", OpCode::Ceil),
    #[cfg(feature = "ext-math")]
    ("floor", OpCode::Floor),
    // tensor
    #[cfg(feature = "tensor")]
    ("tensor", OpCode::TensorMake),
    #[cfg(feature = "tensor")]
    ("zeros", OpCode::TensorZeros),
    #[cfg(feature = "tensor")]
    ("full", OpCode::TensorFull),
    #[cfg(feature = "tensor")]
    ("scatter", OpCode::TensorScatter),
    #[cfg(feature = "tensor")]
    ("rle_expand", OpCode::TensorRleExpand),
    #[cfg(feature = "tensor")]
    ("one_hot", OpCode::TensorOneHot),
    #[cfg(feature = "tensor")]
    ("stack", OpCode::TensorStack),
    #[cfg(feature = "tensor")]
    ("concat", OpCode::TensorConcat),
    #[cfg(feature = "tensor")]
    ("unstack", OpCode::TensorUnstack),
    #[cfg(feature = "tensor")]
    ("reshape", OpCode::TensorReshape),
    #[cfg(feature = "tensor")]
    ("transpose", OpCode::TensorTranspose),
    #[cfg(feature = "tensor")]
    ("pad", OpCode::TensorPad),
    #[cfg(feature = "tensor")]
    ("crop", OpCode::TensorCrop),
    #[cfg(feature = "tensor")]
    ("cast", OpCode::TensorCast),
    #[cfg(feature = "tensor")]
    ("normalize", OpCode::TensorNormalize),
    #[cfg(feature = "tensor")]
    ("argmax", OpCode::TensorArgmax),
    #[cfg(feature = "tensor")]
    ("gather", OpCode::TensorGather),
    #[cfg(feature = "tensor")]
    ("to_list", OpCode::TensorToList),
    #[cfg(feature = "tensor")]
    ("shape", OpCode::TensorShape),
    #[cfg(feature = "tensor")]
    ("dtype", OpCode::TensorDtype),
    // flagd
    #[cfg(feature = "flagd")]
    ("fractional", OpCode::Fractional),
    #[cfg(feature = "flagd")]
    ("sem_ver", OpCode::SemVer),
];

impl FromStr for OpCode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Linear scan over OPCODE_NAMES. Compilation is cold (one-shot per
        // rule) and the table is small (~60 entries), so this is fine.
        for (name, op) in OPCODE_NAMES {
            if *name == s {
                return Ok(*op);
            }
        }
        Err(())
    }
}

/// Every name [`OpCode::from_str`] accepts in this build: canonical names
/// and their aliases, in table order (canonical first, then aliases for
/// the same opcode). Derived from [`OPCODE_NAMES`], so it cannot drift
/// from dispatch; the table's `#[cfg]` gates are what make the result
/// reflect the compiled feature set.
pub(crate) fn builtin_operator_names() -> impl Iterator<Item = &'static str> {
    OPCODE_NAMES.iter().map(|(name, _)| *name)
}

impl OpCode {
    /// Convert OpCode back to its canonical string form (for debugging /
    /// display / serialization).
    ///
    /// Direct `match` rather than a scan over [`OPCODE_NAMES`] — `as_str`
    /// is on the hot path for error formatting, tracing, and
    /// [`crate::CompiledNode::operator_name`]. The match compiles to a
    /// jump table on the `#[repr(u8)]` discriminant.
    ///
    /// When adding a new variant, add the canonical name here AND an entry
    /// to [`OPCODE_NAMES`] (the latter governs `from_str`).
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            // Core: variable access. `val` is canonical; `var` is an alias.
            OpCode::Val => "val",
            // Core: comparison
            OpCode::Equals => "==",
            OpCode::StrictEquals => "===",
            OpCode::NotEquals => "!=",
            OpCode::StrictNotEquals => "!==",
            OpCode::GreaterThan => ">",
            OpCode::GreaterThanEqual => ">=",
            OpCode::LessThan => "<",
            OpCode::LessThanEqual => "<=",
            // Core: logical
            OpCode::Not => "!",
            OpCode::BoolCast => "!!",
            OpCode::And => "and",
            OpCode::Or => "or",
            // Core: control flow. `if` is canonical; `?:` is an alias.
            OpCode::If => "if",
            // Core: arithmetic
            OpCode::Add => "+",
            OpCode::Subtract => "-",
            OpCode::Multiply => "*",
            OpCode::Divide => "/",
            OpCode::Modulo => "%",
            OpCode::Max => "max",
            OpCode::Min => "min",
            // Core: string
            OpCode::Concat => "cat",
            OpCode::Substr => "substr",
            OpCode::In => "in",
            // Core: array
            OpCode::Merge => "merge",
            OpCode::Filter => "filter",
            OpCode::Map => "map",
            OpCode::Reduce => "reduce",
            OpCode::All => "all",
            OpCode::Some => "some",
            OpCode::None => "none",
            // Core: missing
            OpCode::Missing => "missing",
            OpCode::MissingSome => "missing_some",
            // datetime
            #[cfg(feature = "datetime")]
            OpCode::Datetime => "datetime",
            #[cfg(feature = "datetime")]
            OpCode::Timestamp => "timestamp",
            #[cfg(feature = "datetime")]
            OpCode::ParseDate => "parse_date",
            #[cfg(feature = "datetime")]
            OpCode::FormatDate => "format_date",
            #[cfg(feature = "datetime")]
            OpCode::DateDiff => "date_diff",
            #[cfg(feature = "datetime")]
            OpCode::Now => "now",
            // ext-string
            #[cfg(feature = "ext-string")]
            OpCode::Length => "length",
            #[cfg(feature = "ext-string")]
            OpCode::StartsWith => "starts_with",
            #[cfg(feature = "ext-string")]
            OpCode::EndsWith => "ends_with",
            #[cfg(feature = "ext-string")]
            OpCode::Upper => "upper",
            #[cfg(feature = "ext-string")]
            OpCode::Lower => "lower",
            #[cfg(feature = "ext-string")]
            OpCode::Trim => "trim",
            #[cfg(feature = "ext-string")]
            OpCode::Split => "split",
            // ext-array
            #[cfg(feature = "ext-array")]
            OpCode::Sort => "sort",
            #[cfg(feature = "ext-array")]
            OpCode::Slice => "slice",
            #[cfg(feature = "ext-array")]
            OpCode::GroupBy => "group_by",
            #[cfg(feature = "ext-array")]
            OpCode::Distinct => "distinct",
            // ext-object
            #[cfg(feature = "ext-object")]
            OpCode::Keys => "keys",
            #[cfg(feature = "ext-object")]
            OpCode::Values => "values",
            #[cfg(feature = "ext-object")]
            OpCode::Entries => "entries",
            // ext-control. `switch` is canonical; `match` is an alias.
            #[cfg(feature = "ext-control")]
            OpCode::Exists => "exists",
            #[cfg(feature = "ext-control")]
            OpCode::Coalesce => "??",
            #[cfg(feature = "ext-control")]
            OpCode::Switch => "switch",
            #[cfg(feature = "ext-control")]
            OpCode::Type => "type",
            // error-handling
            #[cfg(feature = "error-handling")]
            OpCode::Try => "try",
            #[cfg(feature = "error-handling")]
            OpCode::Throw => "throw",
            // ext-math
            #[cfg(feature = "ext-math")]
            OpCode::Abs => "abs",
            #[cfg(feature = "ext-math")]
            OpCode::Ceil => "ceil",
            #[cfg(feature = "ext-math")]
            OpCode::Floor => "floor",
            // tensor
            #[cfg(feature = "tensor")]
            OpCode::TensorMake => "tensor",
            #[cfg(feature = "tensor")]
            OpCode::TensorZeros => "zeros",
            #[cfg(feature = "tensor")]
            OpCode::TensorFull => "full",
            #[cfg(feature = "tensor")]
            OpCode::TensorScatter => "scatter",
            #[cfg(feature = "tensor")]
            OpCode::TensorRleExpand => "rle_expand",
            #[cfg(feature = "tensor")]
            OpCode::TensorOneHot => "one_hot",
            #[cfg(feature = "tensor")]
            OpCode::TensorStack => "stack",
            #[cfg(feature = "tensor")]
            OpCode::TensorConcat => "concat",
            #[cfg(feature = "tensor")]
            OpCode::TensorUnstack => "unstack",
            #[cfg(feature = "tensor")]
            OpCode::TensorReshape => "reshape",
            #[cfg(feature = "tensor")]
            OpCode::TensorTranspose => "transpose",
            #[cfg(feature = "tensor")]
            OpCode::TensorPad => "pad",
            #[cfg(feature = "tensor")]
            OpCode::TensorCrop => "crop",
            #[cfg(feature = "tensor")]
            OpCode::TensorCast => "cast",
            #[cfg(feature = "tensor")]
            OpCode::TensorNormalize => "normalize",
            #[cfg(feature = "tensor")]
            OpCode::TensorArgmax => "argmax",
            #[cfg(feature = "tensor")]
            OpCode::TensorGather => "gather",
            #[cfg(feature = "tensor")]
            OpCode::TensorToList => "to_list",
            #[cfg(feature = "tensor")]
            OpCode::TensorShape => "shape",
            #[cfg(feature = "tensor")]
            OpCode::TensorDtype => "dtype",
            // flagd
            #[cfg(feature = "flagd")]
            OpCode::Fractional => "fractional",
            #[cfg(feature = "flagd")]
            OpCode::SemVer => "sem_ver",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every entry in OPCODE_NAMES must round-trip: parse the string back
    /// to its OpCode, then `as_str()` the opcode and feed that through
    /// `from_str` again — the second resolution must land on the same
    /// opcode. Catches drift where `as_str` returns a name that
    /// `from_str` doesn't recognise (a missed table edit).
    #[test]
    fn as_str_round_trips_through_from_str() {
        for (name, expected) in OPCODE_NAMES {
            let parsed = OpCode::from_str(name).expect("OPCODE_NAMES entry must parse");
            assert_eq!(
                parsed, *expected,
                "OPCODE_NAMES entry {name:?} parses to {parsed:?}, expected {expected:?}"
            );
            let canonical = parsed.as_str();
            let reparsed = OpCode::from_str(canonical)
                .expect("canonical name from `as_str` must parse via `from_str`");
            assert_eq!(
                reparsed, *expected,
                "canonical {canonical:?} for {expected:?} re-parses to {reparsed:?}"
            );
        }
    }

    /// `builtin_operator_names` is a public contract (via
    /// `Engine::builtin_operator_names`), so the table must never carry
    /// the same string twice: a duplicate would be silently shadowed by
    /// `from_str`'s first-match scan and double-reported by the iterator.
    #[test]
    fn builtin_operator_names_are_unique_and_match_table() {
        let names: Vec<&str> = builtin_operator_names().collect();
        assert_eq!(names.len(), OPCODE_NAMES.len());
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "duplicate name in OPCODE_NAMES");
        // Canonical name precedes its aliases: the first occurrence of each
        // opcode in table order must be what `as_str` reports.
        let mut seen = Vec::new();
        for (name, op) in OPCODE_NAMES {
            if !seen.contains(op) {
                seen.push(*op);
                assert_eq!(
                    op.as_str(),
                    *name,
                    "first entry for {op:?} is not canonical"
                );
            }
        }
    }
}
