//! Reading the JSONLogic conformance suites under `tests/suites/`.
//!
//! One copy of the loading rules, shared by the conformance runner
//! (`test_jsonlogic.rs`), the oracle check (`oracle_test.rs`) and the
//! benchmark (`tools/benchmark`, which includes this file by path), so all
//! three read `index.json`, split a suite into its entries and pick an
//! engine flavour the same way. It depends on `std` and `serde_json` only,
//! which every consumer has.
//!
//! Every reader fails loudly: a suite that cannot be read or parsed panics
//! with its path rather than being dropped, since a silently skipped suite
//! is coverage nobody knows is missing.

// Each consumer uses a different subset.
#![allow(dead_code)]

use std::fs;
use std::path::Path;

use serde_json::{Map, Value};

/// The suites listed in `<root>/index.json`, in run order.
///
/// # Panics
///
/// When the index cannot be read, or is not a JSON array of strings.
pub fn index(root: &Path) -> Vec<String> {
    let path = root.join("index.json");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()))
}

/// Every `.json` suite under `root` except `index.json`, relative to `root`
/// with `/` separators, sorted.
pub fn on_disk(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
        let entries = fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("failed to read suite dir {}: {e}", dir.display()));
        for entry in entries {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                walk(&path, root, out);
            } else if path.extension().is_some_and(|e| e == "json")
                && path.file_name().is_some_and(|n| n != "index.json")
            {
                let rel = path.strip_prefix(root).expect("under root");
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

/// The entries of one suite file: case objects, plus the strings that serve
/// as section headers. An entry's position in the returned list is the
/// index the runners report for it.
///
/// # Panics
///
/// When the file cannot be read or parsed, or is not a JSON array.
pub fn entries(path: &Path) -> Vec<Value> {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read suite {}: {e}", path.display()));
    let value: Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("failed to parse suite {}: {e}", path.display()));
    match value {
        Value::Array(entries) => entries,
        _ => panic!(
            "suite {} should contain an array of test cases",
            path.display()
        ),
    }
}

/// The engine flavour a case asks for: its `templating` flag and its
/// optional `template_key_escape` character. Both are compile-time
/// settings, so the runners build one engine per distinct flavour.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Flavour {
    /// `"templating": true`. Absent, or not a boolean, means off.
    pub templating: bool,
    /// `"template_key_escape": "<one char>"`. Combines with `templating`
    /// either way, so a suite can pin that the escape is inert outside
    /// templating mode.
    pub key_escape: Option<char>,
}

impl Flavour {
    /// Read a case's flavour fields.
    ///
    /// # Errors
    ///
    /// When `template_key_escape` is present but is not a string of exactly
    /// one character. The message names the field, for the caller to
    /// prefix with the case's position.
    pub fn of(case: &Map<String, Value>) -> Result<Self, String> {
        let templating = case
            .get("templating")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let key_escape = match case.get("template_key_escape") {
            None => None,
            Some(v) => {
                let s = v
                    .as_str()
                    .ok_or_else(|| "'template_key_escape' must be a string".to_string())?;
                let mut chars = s.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => Some(c),
                    _ => {
                        return Err(format!(
                            "'template_key_escape' must be exactly one character, got {s:?}"
                        ));
                    }
                }
            }
        };
        Ok(Self {
            templating,
            key_escape,
        })
    }
}
