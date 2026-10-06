//! The hand-written typings of the wire formats must match `schemas/`.
//!
//! The Node binding spells the TypeScript type of `check()`, `operators()`
//! and `Rule.facts()` by hand (`ts_return_type` in
//! `bindings/node/src/engine.rs`), and the Python stub
//! (`bindings/python/datalogic_py.pyi`) declares the same documents as
//! `TypedDict`s. Neither is generated from `schemas/*.v1.json`, so this test
//! reads all three and fails when a typing names a property the schema
//! lacks, misses one it has, or gives one a different JSON type (or allows
//! or forbids `null` differently). A string-literal type must name values
//! the schema's `enum` allows.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{ROOT}/{path}")).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// A JSON type as the schemas name them, `integer` folded into `number`
/// (neither TypeScript nor Python typing tells them apart here).
type Kinds = BTreeSet<&'static str>;

/// One property's type: its JSON kinds and, for a string-literal type, the
/// literals.
#[derive(Debug, PartialEq)]
struct Prop {
    kinds: Kinds,
    literals: BTreeSet<String>,
}

/// The properties of an object schema (or of an array schema's items).
fn schema_props(file: &str) -> BTreeMap<String, (Kinds, Option<BTreeSet<String>>)> {
    let schema: Value = serde_json::from_str(&read(&format!("schemas/{file}"))).unwrap();
    let object = if schema["type"] == "array" {
        &schema["items"]
    } else {
        &schema
    };
    let required: BTreeSet<&str> = object["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let props = object["properties"].as_object().unwrap();
    assert_eq!(
        required,
        props.keys().map(String::as_str).collect(),
        "{file}: every property is required, so a typing may declare them all"
    );
    props
        .iter()
        .map(|(name, p)| {
            let types: Vec<&str> = match &p["type"] {
                Value::String(t) => vec![t.as_str()],
                Value::Array(ts) => ts.iter().map(|t| t.as_str().unwrap()).collect(),
                other => panic!("{file}: {name}: type {other}"),
            };
            let kinds = types
                .into_iter()
                .map(|t| match t {
                    "integer" | "number" => "number",
                    "string" => "string",
                    "boolean" => "boolean",
                    "null" => "null",
                    "array" => "array",
                    "object" => "object",
                    other => panic!("{file}: {name}: type {other}"),
                })
                .collect();
            let literals = p["enum"].as_array().map(|values| {
                values
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect()
            });
            (name.clone(), (kinds, literals))
        })
        .collect()
}

/// Split on `sep` outside brackets, braces, parentheses and quotes.
fn split_top(text: &str, sep: char) -> Vec<&str> {
    let (mut out, mut depth, mut start, mut quote) = (Vec::new(), 0i32, 0, None);
    for (i, c) in text.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(c),
            (None, '<' | '[' | '{' | '(') => depth += 1,
            (None, '>' | ']' | '}' | ')') => depth -= 1,
            (None, c) if c == sep && depth == 0 => {
                out.push(text[start..i].trim());
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    let last = text[start..].trim();
    if !last.is_empty() {
        out.push(last);
    }
    out
}

/// Strip one level of the `open .. close` wrapper around `text`.
fn inside<'a>(text: &'a str, open: &str, close: &str) -> &'a str {
    let text = text.trim();
    text.strip_prefix(open)
        .and_then(|t| t.strip_suffix(close))
        .unwrap_or_else(|| panic!("`{text}` is not wrapped in {open}..{close}"))
}

/// The JSON kinds (and literals) a TypeScript type allows.
fn ts_prop(ty: &str) -> Prop {
    let mut prop = Prop {
        kinds: Kinds::new(),
        literals: BTreeSet::new(),
    };
    for alt in split_top(ty, '|') {
        let kind = match alt {
            "string" => "string",
            "number" => "number",
            "boolean" => "boolean",
            "null" => "null",
            a if a.ends_with("[]") || a.starts_with("Array<") => "array",
            a if a.starts_with('{') => "object",
            a if a.starts_with('\'') || a.starts_with('"') => {
                prop.literals.insert(a[1..a.len() - 1].to_string());
                "string"
            }
            other => panic!("unclassified TypeScript type `{other}`"),
        };
        prop.kinds.insert(kind);
    }
    prop
}

/// The properties of a TypeScript object type `{ a: T; b: U }`.
fn ts_object(ty: &str) -> BTreeMap<String, Prop> {
    split_top(inside(ty, "{", "}"), ';')
        .into_iter()
        .map(|field| {
            let (name, ty) = field.split_once(':').expect("`name: type`");
            (name.trim().to_string(), ts_prop(ty))
        })
        .collect()
}

/// The `ts_return_type` the Node binding gives method `name`.
fn node_return_type(source: &str, name: &str) -> String {
    let marker = "ts_return_type = \"";
    let mut from = 0;
    while let Some(rel) = source[from..].find(marker) {
        let start = from + rel + marker.len();
        let end = start + source[start..].find('"').expect("closing quote");
        from = end;
        let after = &source[end..];
        let fn_at = after.find("pub fn ").expect("a method follows") + "pub fn ".len();
        let method: String = after[fn_at..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if method == name {
            return source[start..end].to_string();
        }
    }
    panic!("no ts_return_type on `{name}`")
}

/// The JSON kinds (and literals) a Python annotation allows.
fn py_prop(ty: &str) -> Prop {
    let mut prop = Prop {
        kinds: Kinds::new(),
        literals: BTreeSet::new(),
    };
    for alt in split_top(ty, '|') {
        let kind = match alt {
            "str" => "string",
            "int" | "float" => "number",
            "bool" => "boolean",
            "None" => "null",
            a if a.starts_with("list[") => "array",
            a if a.starts_with("dict[") => "object",
            a if a.starts_with("Literal[") => {
                for lit in split_top(inside(a, "Literal[", "]"), ',') {
                    prop.literals.insert(lit[1..lit.len() - 1].to_string());
                }
                "string"
            }
            other => panic!("unclassified Python annotation `{other}`"),
        };
        prop.kinds.insert(kind);
    }
    prop
}

/// The fields of `class <name>(TypedDict):` in the stub.
fn py_typed_dict(stub: &str, name: &str) -> BTreeMap<String, Prop> {
    let header = format!("class {name}(TypedDict):");
    let at = stub
        .find(&header)
        .unwrap_or_else(|| panic!("no TypedDict {name}"));
    let mut out = BTreeMap::new();
    let mut in_docstring = false;
    for line in stub[at + header.len()..].lines().skip(1) {
        if !line.is_empty() && !line.starts_with(' ') {
            break; // the class body ended
        }
        let line = line.trim();
        let quotes = line.matches("\"\"\"").count();
        if in_docstring || quotes > 0 {
            if quotes % 2 == 1 {
                in_docstring = !in_docstring;
            }
            continue;
        }
        if let Some((field, ty)) = line.split_once(':') {
            out.insert(field.trim().to_string(), py_prop(ty));
        }
    }
    assert!(!out.is_empty(), "TypedDict {name} has no fields");
    out
}

/// Every typing property must match the schema property of that name.
fn compare(what: &str, file: &str, typing: &BTreeMap<String, Prop>) {
    let schema = schema_props(file);
    let mut problems = Vec::new();
    for name in schema.keys().filter(|k| !typing.contains_key(*k)) {
        problems.push(format!("missing `{name}`"));
    }
    for (name, prop) in typing {
        let Some((kinds, allowed)) = schema.get(name) else {
            problems.push(format!("`{name}` is not in {file}"));
            continue;
        };
        if &prop.kinds != kinds {
            problems.push(format!(
                "`{name}` allows {:?}, {file} says {kinds:?}",
                prop.kinds
            ));
        }
        if let Some(allowed) = allowed {
            let unknown: Vec<_> = prop.literals.difference(allowed).collect();
            if !unknown.is_empty() {
                problems.push(format!("`{name}` names {unknown:?}, not in its enum"));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "{what} disagrees with schemas/{file}:\n  {}",
        problems.join("\n  ")
    );
}

#[test]
fn node_typings_match_the_schemas() {
    let source = read("bindings/node/src/engine.rs");
    compare(
        "Node `check()`",
        "diagnostics.v1.json",
        &ts_object(inside(&node_return_type(&source, "check"), "Array<", ">")),
    );
    compare(
        "Node `operators()`",
        "operators.v1.json",
        &ts_object(inside(
            &node_return_type(&source, "operators"),
            "Array<",
            ">",
        )),
    );
    compare(
        "Node `Rule.facts()`",
        "facts.v1.json",
        &ts_object(&node_return_type(&source, "facts")),
    );
}

#[test]
fn python_stubs_match_the_schemas() {
    let stub = read("bindings/python/datalogic_py.pyi");
    compare(
        "Python `Diagnostic`",
        "diagnostics.v1.json",
        &py_typed_dict(&stub, "Diagnostic"),
    );
    compare(
        "Python `OperatorInfo`",
        "operators.v1.json",
        &py_typed_dict(&stub, "OperatorInfo"),
    );
    compare(
        "Python `RuleFacts`",
        "facts.v1.json",
        &py_typed_dict(&stub, "RuleFacts"),
    );
}

#[test]
fn the_type_readers_classify() {
    let p = ts_prop("number | 'last' | null");
    assert_eq!(p.kinds, ["null", "number", "string"].into_iter().collect());
    assert_eq!(p.literals, ["last".to_string()].into_iter().collect());
    let o = ts_object("{ a: string[][]; b: { c: number }; d: boolean }");
    assert_eq!(o["a"].kinds, ["array"].into_iter().collect());
    assert_eq!(o["b"].kinds, ["object"].into_iter().collect());
    let p = py_prop("int | Literal[\"a\", \"b\"] | None");
    assert_eq!(p.kinds, ["null", "number", "string"].into_iter().collect());
    assert_eq!(p.literals.len(), 2);
}
