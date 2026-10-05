//! The single-table rule: a fact about one operator belongs in its row of
//! `src/operators/table.rs`, where scoping, folding, CSE, arity and the
//! docs read it, not in a `match` on that opcode somewhere else.
//!
//! This scans the crate's non-test source for `OpCode::<Variant>` outside
//! the table and compares what it finds with the list below, where every
//! remaining reference says why it is not a table fact. A new reference
//! fails until it is either moved into the table or listed here with a
//! reason; a listed one that disappears fails until it is removed from the
//! list.

use std::collections::BTreeSet;
use std::path::Path;

/// `(file under src/, opcode variant, why it stays outside the table)`.
const ALLOWED: &[(&str, &str, &str)] = &[
    // The `var` / `val` node family is compiled to its own node kinds; these
    // build or recognise them.
    (
        "compile/operator.rs",
        "VarDefault",
        "emits the internal computed-path var",
    ),
    ("compile/scope.rs", "Val", "binds var/val reads to frames"),
    (
        "compile/scope.rs",
        "VarDefault",
        "binds var/val reads to frames",
    ),
    // Facts name the operators that compile to their own node kinds and so
    // no longer carry an opcode.
    ("facts.rs", "Val", "names a compiled Var node"),
    ("facts.rs", "Exists", "names a compiled Exists node"),
    ("facts.rs", "Throw", "names a compiled Throw node"),
    ("facts.rs", "Missing", "names a compiled Missing node"),
    (
        "facts.rs",
        "MissingSome",
        "names a compiled MissingSome node",
    ),
    (
        "facts.rs",
        "Fractional",
        "the root paths fractional reads implicitly",
    ),
    // A pass whose subject is the operator's own semantics.
    (
        "compile/optimize/dead_code.rs",
        "If",
        "removes untaken if branches",
    ),
    (
        "compile/optimize/cse.rs",
        "If",
        "if value arms are mutually exclusive",
    ),
    // Literal forms the compiler recognises before the operator runs.
    ("compile/hooks.rs", "TensorMake", "the tensor wire form"),
    (
        "check.rs",
        "TensorMake",
        "the tensor wire form, as the hook reads it",
    ),
    (
        "check.rs",
        "ParseDate",
        "the literal timezone, as the hook checks it",
    ),
    (
        "check.rs",
        "FormatDate",
        "the literal timezone, as the hook checks it",
    ),
    // Fast-path shapes (see operators/array/fast_paths.rs).
    (
        "operators/array/helpers.rs",
        "In",
        "the FastPredicate `in` leaf",
    ),
    (
        "operators/array/reduce.rs",
        "Map",
        "the reduce-over-map fusion",
    ),
];

/// Not variants: associated items of `OpCode`.
const NOT_VARIANTS: &[&str] = &["ALL", "FromStr"];

/// Every `(file, variant)` referenced before the file's first test module.
fn references(src: &Path) -> BTreeSet<(String, String)> {
    let mut out = BTreeSet::new();
    let mut stack = vec![src.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let rel = path
                .strip_prefix(src)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if !rel.ends_with(".rs")
                || rel == "operators/table.rs"
                || rel.ends_with("table_tests.rs")
            {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            let lines: Vec<&str> = text.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                let t = line.trim_start();
                // A test module ends the production source.
                if t.starts_with("#[cfg(")
                    && t.contains("test")
                    && lines[i + 1..]
                        .iter()
                        .map(|l| l.trim_start())
                        .find(|l| !l.is_empty() && !l.starts_with("#["))
                        .is_some_and(|l| l.starts_with("mod "))
                {
                    break;
                }
                if t.starts_with("//") {
                    continue;
                }
                let mut rest: &str = line;
                while let Some(at) = rest.find("OpCode::") {
                    rest = &rest[at + "OpCode::".len()..];
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    if name.starts_with(|c: char| c.is_ascii_uppercase())
                        && !NOT_VARIANTS.contains(&name.as_str())
                    {
                        out.insert((rel.clone(), name));
                    }
                }
            }
        }
    }
    out
}

#[test]
fn per_operator_facts_live_in_the_table() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let found = references(&src);
    let allowed: BTreeSet<(String, String)> = ALLOWED
        .iter()
        .map(|(f, v, _)| (f.to_string(), v.to_string()))
        .collect();
    let new: Vec<_> = found.difference(&allowed).collect();
    let gone: Vec<_> = allowed.difference(&found).collect();
    assert!(
        new.is_empty(),
        "opcode-specific code outside operators/table.rs: {new:?}. Declare the fact on the \
         operator's row, or add it to ALLOWED with the reason it cannot be."
    );
    assert!(
        gone.is_empty(),
        "ALLOWED lists references that no longer exist: {gone:?}"
    );
}
