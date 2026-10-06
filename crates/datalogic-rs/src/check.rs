//! [`Engine::check`](crate::Engine::check): the problems the engine can see
//! in a rule before it runs, collected in one pass.
//!
//! The walk mirrors the compiler's (`compile::walker`): the same routing for
//! template keys, escaped keys, built-ins, custom operators and unknown
//! keys, and the same exceptions (an argument form the compiler does not
//! compile, the tensor wire form). What it reports is read from the
//! operator table, so an argument count is an error exactly when the
//! operator would reject it.

use std::fmt;

use datavalue::OwnedDataValue;
use serde::Serialize;

use crate::operators::meta::{ArgsForm, Extra, Miss};
use crate::{Engine, OpCode};

/// Which mode a rule is checked in. See [`Engine::check`](crate::Engine::check).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CheckMode {
    /// The engine's own mode, as [`Engine::compile`](crate::Engine::compile)
    /// would compile the rule.
    Engine,
    /// Outside templating mode, as
    /// [`Engine::compile_strict`](crate::Engine::compile_strict) would.
    Strict,
    /// In templating mode, as `Engine::compile_template` would (that method
    /// needs the `templating` feature; the check does not).
    Template,
}

/// How serious a [`Diagnostic`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Severity {
    /// The rule fails to compile, or fails whenever this part of it runs.
    Error,
    /// The rule runs, but probably not as its author meant.
    Warning,
}

/// What a [`Diagnostic`] is about. Serialised as the variant name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[non_exhaustive]
pub enum DiagnosticCode {
    /// The rule is not valid JSON.
    Unparsable,
    /// A single-key object whose key is no operator this engine knows.
    UnknownOperator,
    /// An object with several keys outside templating mode, where only an
    /// operator call (one key) is valid.
    NotAnOperator,
    /// `and`, `or` or `if` given a single argument instead of an array.
    ArgumentForm,
    /// An argument count the operator rejects (an error), or arguments it
    /// never evaluates (a warning).
    ArgumentCount,
    /// A literal timezone name that is not in the timezone database.
    InvalidTimezone,
    /// In a template, a key that is an output field but is one edit away
    /// from an operator name.
    SimilarToOperator,
    /// The compiler rejected the rule for a reason the checks above do not
    /// cover.
    Compile,
    /// A custom operator's own [`check`](crate::CustomOperator::check)
    /// rejected the call.
    OperatorCheck,
}

/// One problem in a rule, located by an RFC 6901 JSON Pointer into it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Diagnostic {
    /// What the problem is.
    pub code: DiagnosticCode,
    /// Whether the rule can run as written.
    pub severity: Severity,
    /// A sentence describing the problem, naming the operator involved.
    pub message: String,
    /// Where the problem is: an [RFC 6901](https://www.rfc-editor.org/rfc/rfc6901)
    /// JSON Pointer into the rule, to the object (or argument) at fault.
    /// `""` is the whole rule. `{"if": [x, {"vr": 1}]}` reports the
    /// unknown `vr` at `/if/1`.
    pub pointer: String,
    /// The operator (or key) the problem is about, when there is one.
    pub operator: Option<String>,
}

impl Diagnostic {
    /// An error from a custom operator's
    /// [`check`](crate::CustomOperator::check): the call will fail, so
    /// [`Engine::compile_checked`](crate::Engine::compile_checked) refuses
    /// the rule. It points at the whole call until
    /// [`Self::at_argument`] narrows it.
    pub fn error(message: impl Into<String>) -> Self {
        Self::from_operator(Severity::Error, message.into())
    }

    /// A warning from a custom operator's
    /// [`check`](crate::CustomOperator::check): the call runs, but probably
    /// not as meant.
    pub fn warning(message: impl Into<String>) -> Self {
        Self::from_operator(Severity::Warning, message.into())
    }

    /// Point at argument `index` of the call instead of the whole call. In
    /// a diagnostic an operator returns, `pointer` is relative to the call
    /// (`"/1"` for argument 1); the checker places it in the rule.
    #[must_use]
    pub fn at_argument(mut self, index: usize) -> Self {
        self.pointer = format!("/{index}");
        self
    }

    fn from_operator(severity: Severity, message: String) -> Self {
        Diagnostic {
            code: DiagnosticCode::OperatorCheck,
            severity,
            message,
            pointer: String::new(),
            operator: None,
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let severity = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        let at = if self.pointer.is_empty() {
            "/"
        } else {
            &self.pointer
        };
        write!(f, "{severity} at {at}: {}", self.message)
    }
}

/// A rule [`Engine::compile_checked`](crate::Engine::compile_checked)
/// refused: every diagnostic the check found, errors and warnings, in rule
/// order.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct CompileError {
    /// At least one of them is an error.
    pub diagnostics: Vec<Diagnostic>,
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let errors = self
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count();
        write!(f, "rule has {errors} error(s)")?;
        for d in &self.diagnostics {
            write!(f, "\n  {d}")?;
        }
        Ok(())
    }
}

impl std::error::Error for CompileError {}

impl CompileError {
    pub(crate) fn from_error(err: crate::Error) -> Self {
        let code = match err.kind {
            crate::ErrorKind::ParseError(_) => DiagnosticCode::Unparsable,
            _ => DiagnosticCode::Compile,
        };
        CompileError {
            diagnostics: vec![Diagnostic {
                code,
                severity: Severity::Error,
                message: crate::Error::new(err.kind).to_string(),
                pointer: String::new(),
                operator: None,
            }],
        }
    }
}

/// Check `rule` on `engine`, in templating mode when `templating` holds.
pub(crate) fn check(engine: &Engine, rule: &OwnedDataValue, templating: bool) -> Vec<Diagnostic> {
    let mut checker = Checker {
        engine,
        templating,
        escape: if templating {
            engine.template_key_escape()
        } else {
            None
        },
        out: Vec::new(),
    };
    checker.node(rule, &mut String::new());
    checker.out
}

struct Checker<'e> {
    engine: &'e Engine,
    templating: bool,
    escape: Option<char>,
    out: Vec<Diagnostic>,
}

/// Run `f` with `token` appended to `pointer`, then restore it.
fn under(pointer: &mut String, token: &str, f: impl FnOnce(&mut String)) {
    let len = pointer.len();
    crate::node::push_pointer_token(pointer, token);
    f(pointer);
    pointer.truncate(len);
}

impl Checker<'_> {
    fn report(
        &mut self,
        code: DiagnosticCode,
        severity: Severity,
        pointer: &str,
        operator: &str,
        message: String,
    ) {
        self.out.push(Diagnostic {
            code,
            severity,
            message,
            pointer: pointer.to_string(),
            operator: Some(operator.to_string()),
        });
    }

    fn node(&mut self, node: &OwnedDataValue, pointer: &mut String) {
        match node {
            OwnedDataValue::Object(pairs) if pairs.len() > 1 => {
                if self.templating {
                    for (key, value) in pairs {
                        under(pointer, key, |p| self.node(value, p));
                    }
                } else {
                    self.out.push(Diagnostic {
                        code: DiagnosticCode::NotAnOperator,
                        severity: Severity::Error,
                        message: format!(
                            "an object with {} keys is not an operator call; it is only valid as an output template",
                            pairs.len()
                        ),
                        pointer: pointer.clone(),
                        operator: None,
                    });
                }
            }
            OwnedDataValue::Object(pairs) if pairs.len() == 1 => {
                let (op, argv) = &pairs[0];
                self.call(op, argv, pointer);
            }
            OwnedDataValue::Array(items) => {
                for (i, item) in items.iter().enumerate() {
                    under(pointer, &i.to_string(), |p| self.node(item, p));
                }
            }
            _ => {}
        }
    }

    /// Check each argument, where the compiler would compile it.
    fn args(&mut self, op: &str, argv: &OwnedDataValue, pointer: &mut String) {
        under(pointer, op, |p| match argv {
            OwnedDataValue::Array(items) => {
                for (i, item) in items.iter().enumerate() {
                    under(p, &i.to_string(), |p| self.node(item, p));
                }
            }
            other => self.node(other, p),
        });
    }

    fn call(&mut self, op: &str, argv: &OwnedDataValue, pointer: &mut String) {
        // An escaped key is an output field, checked before any operator.
        // `escape` is `None` outside templating mode.
        if self.escape.is_some_and(|c| op.starts_with(c)) {
            under(pointer, op, |p| self.node(argv, p));
            return;
        }
        let count = match argv {
            OwnedDataValue::Array(items) => items.len(),
            _ => 1,
        };
        if let Some(opcode) = self.engine.builtin(op) {
            self.builtin(op, opcode, argv, count, pointer);
            return;
        }
        if let Some(custom) = self.engine.custom_operator(op) {
            let info = custom.info();
            if !info.accepts(count) {
                let message = format!(
                    "`{op}` takes {}, not {count}",
                    describe_count(info.min_args, info.max_args)
                );
                self.report(
                    DiagnosticCode::ArgumentCount,
                    Severity::Error,
                    pointer,
                    op,
                    message,
                );
            } else {
                let args = match argv {
                    OwnedDataValue::Array(items) => items.as_slice(),
                    other => std::slice::from_ref(other),
                };
                if let Err(d) = custom.check(args) {
                    self.operator_diagnostic(d, op, argv, pointer);
                }
            }
            self.args(op, argv, pointer);
            return;
        }
        if self.templating {
            if let Some(near) = self.similar_operator(op) {
                let message =
                    format!("`{op}` is an output field here; did you mean the operator `{near}`?");
                self.report(
                    DiagnosticCode::SimilarToOperator,
                    Severity::Warning,
                    pointer,
                    op,
                    message,
                );
            }
            under(pointer, op, |p| self.node(argv, p));
            return;
        }
        let message = match self.similar_operator(op) {
            Some(near) => format!("unknown operator `{op}`; did you mean `{near}`?"),
            None => format!("unknown operator `{op}`"),
        };
        self.report(
            DiagnosticCode::UnknownOperator,
            Severity::Error,
            pointer,
            op,
            message,
        );
        self.args(op, argv, pointer);
    }

    /// Place a diagnostic a custom operator returned: its pointer is
    /// relative to the call (empty, or `"/i"` for argument `i`).
    fn operator_diagnostic(
        &mut self,
        mut d: Diagnostic,
        op: &str,
        argv: &OwnedDataValue,
        pointer: &str,
    ) {
        let relative = std::mem::take(&mut d.pointer);
        let mut placed = pointer.to_string();
        if !relative.is_empty() {
            crate::node::push_pointer_token(&mut placed, op);
            // A lone argument that is not an array is the argument list's
            // only member, written in place: argument 0 is the call's value.
            match (argv, relative.strip_prefix("/0")) {
                (OwnedDataValue::Array(_), _) => placed.push_str(&relative),
                (_, Some(rest)) if rest.is_empty() || rest.starts_with('/') => {
                    placed.push_str(rest)
                }
                _ => placed.push_str(&relative),
            }
        }
        d.pointer = placed;
        d.operator.get_or_insert_with(|| op.to_string());
        self.out.push(d);
    }

    fn builtin(
        &mut self,
        op: &str,
        opcode: OpCode,
        argv: &OwnedDataValue,
        count: usize,
        pointer: &mut String,
    ) {
        let meta = opcode.meta();
        if meta.args_form == ArgsForm::ArrayOnly && !matches!(argv, OwnedDataValue::Array(_)) {
            let message = format!("`{op}` takes its arguments as an array");
            self.report(
                DiagnosticCode::ArgumentForm,
                Severity::Error,
                pointer,
                op,
                message,
            );
            return;
        }
        #[cfg(feature = "tensor")]
        if opcode == OpCode::TensorMake
            && let OwnedDataValue::Object(fields) = argv
            && crate::compile::hooks::is_tensor_wire_body(fields)
        {
            return;
        }

        let arity = opcode.arity();
        let (min, max) = (arity.min as usize, arity.max.map(usize::from));
        if count < min {
            if matches!(meta.on_missing, Miss::InvalidArgs | Miss::Err(_)) {
                let message = format!("`{op}` takes {}, not {count}", describe_count(min, max));
                self.report(
                    DiagnosticCode::ArgumentCount,
                    Severity::Error,
                    pointer,
                    op,
                    message,
                );
            }
        } else if let Some(max) = max
            && count > max
        {
            let (severity, message) = match meta.on_extra {
                Extra::InvalidArgs | Extra::Err(_) => (
                    Severity::Error,
                    format!(
                        "`{op}` takes {}, not {count}",
                        describe_count(min, Some(max))
                    ),
                ),
                Extra::Ignore => (
                    Severity::Warning,
                    format!(
                        "`{op}` reads {}; the other {} never evaluated",
                        describe_count(min, Some(max)),
                        plural(count - max, "argument is", "arguments are")
                    ),
                ),
                Extra::Return(_) => (
                    Severity::Warning,
                    format!(
                        "`{op}` takes {}; with {count} it returns a fixed value without evaluating them",
                        describe_count(min, Some(max))
                    ),
                ),
            };
            self.report(
                DiagnosticCode::ArgumentCount,
                severity,
                pointer,
                op,
                message,
            );
        }

        #[cfg(feature = "datetime")]
        if matches!(opcode, OpCode::ParseDate | OpCode::FormatDate)
            && let OwnedDataValue::Array(items) = argv
            && let Some(OwnedDataValue::String(zone)) = items.get(2)
            && zone.parse::<chrono_tz::Tz>().is_err()
        {
            let mut at = pointer.clone();
            crate::node::push_pointer_token(&mut at, op);
            crate::node::push_pointer_token(&mut at, "2");
            let message = format!("unknown timezone `{zone}`");
            self.report(
                DiagnosticCode::InvalidTimezone,
                Severity::Error,
                &at,
                op,
                message,
            );
        }

        self.args(op, argv, pointer);
    }

    /// An operator name one edit from `key`, if any: built-in names first,
    /// then the engine's custom operators. Short names are left out, since
    /// almost any short key is one edit from `+` or `in`.
    fn similar_operator(&self, key: &str) -> Option<String> {
        let close = |name: &str| name.chars().count() >= 3 && one_edit_apart(key, name);
        if let Some(name) = self.engine.builtin_operator_names().find(|n| close(n)) {
            return Some(name.to_string());
        }
        let mut custom: Vec<&str> = self
            .engine
            .custom_operator_names()
            .filter(|n| close(n))
            .collect();
        custom.sort_unstable();
        custom.first().map(|n| n.to_string())
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

/// `exactly 2 arguments`, `at least 1 argument`, `1 to 3 arguments`.
fn describe_count(min: usize, max: Option<usize>) -> String {
    let noun = |n: usize| if n == 1 { "argument" } else { "arguments" };
    match max {
        Some(max) if max == min => format!("exactly {min} {}", noun(min)),
        Some(max) => format!("{min} to {max} {}", noun(max)),
        None => format!("at least {min} {}", noun(min)),
    }
}

/// Whether `a` and `b` are exactly one edit apart (an insertion, a
/// deletion or a substitution of one char): Levenshtein distance 1,
/// without the table.
fn one_edit_apart(a: &str, b: &str) -> bool {
    let (la, lb) = (a.chars().count(), b.chars().count());
    match la.abs_diff(lb) {
        0 => a.chars().zip(b.chars()).filter(|(x, y)| x != y).count() == 1,
        1 => {
            let (long, short) = if la > lb { (a, b) } else { (b, a) };
            let mut long = long.chars();
            let mut skipped = false;
            for c in short.chars() {
                loop {
                    match long.next() {
                        Some(x) if x == c => break,
                        Some(_) if !skipped => skipped = true,
                        _ => return false,
                    }
                }
            }
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{describe_count, one_edit_apart};

    #[test]
    fn distances() {
        assert!(one_edit_apart("vr", "var"));
        assert!(one_edit_apart("var", "vr"));
        assert!(one_edit_apart("mapp", "map"));
        assert!(one_edit_apart("mop", "map"));
        assert!(one_edit_apart("vars", "var"));
        assert!(!one_edit_apart("map", "map"));
        assert!(!one_edit_apart("name", "none"));
        assert!(!one_edit_apart("ab", "ba"));
        assert!(!one_edit_apart("", "abc"));
        assert!(!one_edit_apart("mapxy", "map"));
    }

    #[test]
    fn counts() {
        assert_eq!(describe_count(2, Some(2)), "exactly 2 arguments");
        assert_eq!(describe_count(1, Some(1)), "exactly 1 argument");
        assert_eq!(describe_count(1, None), "at least 1 argument");
        assert_eq!(describe_count(1, Some(3)), "1 to 3 arguments");
    }
}
