# Tech-debt register

Baseline: **5.8.0** (branch `fix/review-findings`, 2026-10-06).
Scope: the whole repo. That covers the core crate, `datalogic-bind`, all eight bindings, the React UI, tests, CI, build, release and docs.

This register sorts every item by the release it can ship in:

- **Part 1: 5.x.** Items that can ship in a 5.x minor or patch release with no breaking change.
- **Part 2: v6.** Items that need a breaking change, held for v6. Many have a non-breaking first step, listed in §1.8.

`proposal-v6.md` already describes the large v6 restructuring (O1–O12). This register does not repeat it. Where an item overlaps the proposal it says so, and the proposal's own debt table (§2) still applies.

## How to read an entry

| Field | Values |
|---|---|
| **Severity** | **High**: crash, UB, memory unsafety, silently wrong results, or a supply-chain or release risk. **Medium**: a correctness edge, inconsistency or drift risk with real user impact. **Low**: maintainability, docs, small performance gains. |
| **Effort** | **S**: under 1 day. **M**: 1–5 days. **L**: more than 1 week. |
| **Target** | **5.x**: non-breaking. **v6**: breaking. **v6 (5.x opt-in)**: changes an observable result, so the default flips only in v6. A 5.x release can offer it behind a config flag. |

Each entry gives the file and line it applies to. Each section names its path prefix.

**How the findings were checked.** Every finding was checked by reading the code. Many were also reproduced with throwaway probe binaries built against this tree in a scratch directory; no repo file was changed. The headline items were checked by hand a second time: CORE-01/02/03/17, OPS-01/03, BIND-01, HOST-01/02, INFRA-01/04/07, UI-03 and V6-OPS-01.

**Semver rule used.** An item is breaking if it does any of the following:
- removes or renames a public item, or changes its signature;
- adds a trait method without a default;
- adds a variant to an exhaustive enum or a field to an all-public struct;
- changes a wire or JSON shape;
- tightens what compiles;
- changes the result for an input that succeeds today.

Turning a crash, abort or nonsense output into an error counts as a bug fix, so it can go in 5.x. Budget op counts are treated as non-contractual, following the 5.7 pricing precedent.

---

## Top priorities

These are the items to fix first. All of them can ship in 5.x except V6-OPS-01.

| # | ID | Item | Why now |
|---|----|------|---------|
| 1 | CORE-01 ✅ | A rule can build a value nested deep enough to abort the process with a stack overflow | A rule and data the host doesn't trust can kill the host process, and `catch_unwind` cannot recover. |
| 2 | OPS-01 ✅ | `format_date` panics on an invalid format specifier | A panic reachable from user input. It aborts under wasm and crosses the FFI boundary in the C hosts. |
| 3 | BIND-02 | A C ABI custom-operator callback that re-enters its own session is UB | Memory corruption in Go, JVM, .NET and PHP. |
| 4 | HOST-01 ✅ | Go: an Engine's finalizer deletes callback handles that live Rules still use | Process crash in a common pattern. |
| 5 | HOST-02 ✅ | .NET: an Engine's finalizer frees GCHandles that live Rules still use | Wrong delegate called, or a failure. |
| 6 | BIND-01 ✅ | A panic in the Node binding aborts the Node process | No `catch_unwind` anywhere. |
| 7 | CORE-02, CORE-03 | `Logic::to_json` emits invalid JSON and rewrites `val` as `var` | Saving a rule and loading it back silently changes what it reads. Trace output is broken too. |
| 8 | OPS-02, OPS-03 | Tensor constructors allocate unbounded memory; `fractional` weights overflow | OOM from a 40-byte rule; wrong variant chosen in release builds. |
| 9 | CORE-04 ✅ | Constant folding bakes the compiling engine's config into a `Logic` | Wrong results when a rule is evaluated on a different engine, which is common with hot reload. |
| 10 | UI-01, UI-02 | The published CJS build of the UI cannot start WASM; keyboard shortcuts take over the host page | Broken for every `require` consumer and every page that embeds the editor. |
| 11 | INFRA-01/02/03 | No committed `Cargo.lock`, no semver check, no `cargo-deny` | Release artifacts for six registries are not reproducible or audited. |
| 12 | INFRA-07 | `scripts/check-stats.sh` fails on this branch now | CI will go red. |
| 13 | HOST-03 | Each Go module tag ships about 6 × 80 MB of static libraries | Close to the Go module proxy's 500 MiB limit. |
| 14 | V6-OPS-01 ✅ | Any object with a `datetime` or `timestamp` key compares equal to other such objects | Silently wrong `===`, `in` and `distinct` results on ordinary records. Consider shipping the fix in 5.x behind an opt-in. |

---

## Progress

These items are fixed in the working tree (see `CHANGELOG.md` → Unreleased) and not yet released:

| ID | Fix |
|----|-----|
| CORE-01 | A `reduce` accumulator may nest at most 1,024 levels. Passing that is an `InvalidArguments` error, not a stack-overflow abort. The check measures only the parts each step built, and runs every few steps when the body is plain built-ins, so the `merge` list-building pattern runs at baseline speed (5.0 ms vs 5.0 ms for 2,000 records). Code: `operators/array/nesting.rs`. Tests: `tests/reduce_depth_test.rs`. |
| OPS-01 | `format_date` renders through `fmt::Write`, so an unknown specifier is `InvalidArguments("Invalid date format")`. Covered by 4 suite cases. |
| CORE-04 | Fully fixed in 5.x, not just the first step. A rule whose compile folded something under the engine's settings keeps its source, and is compiled again on an engine with a different settings fingerprint. That recompile is cached on the rule, with up to 4 slots. Tests: `tests/cross_engine_fold_test.rs`; 10 of the 11 fail without the fix. |
| V6-OPS-01 | Shipped in 5.x as a correctness fix: only a single-key `{"datetime": ...}` / `{"timestamp": ...}` object counts as a datetime. Covered by 10 suite cases. |
| BIND-01 | Node: every export has `#[napi(catch_unwind)]`, every call into the engine runs through `guard()`, and the async task's `compute` is wrapped. A panic surfaces as `InternalError`. |
| HOST-01 | Go: the operator handles live in a shared `opRegistry` that Engine, Rule, Session and TracedSession all hold, and a C-allocated box carries the handle in place of a Go pointer. The new tests crashed before the fix. |
| HOST-02 | .NET: `CallbackRoots` holds the GCHandles and is shared by Engine, Rule, Session and TracedSession. The new tests failed before the fix. |

### Batch 2 (5.x)

All of these are committed and are listed in `CHANGELOG.md` under Unreleased.

The rule applied: bug fixes are in, even when they change output. So are refactors that keep behaviour identical, performance, tests, docs and CI. New APIs or options, deprecations, default changes and design changes are out.

**Done**
- **Core:** CORE-02, 03, 06, 08, 13, 15, 16, 17, 19, 20, 22, 23, 24; INFRA-34.
- **Core, partial:**
  - CORE-05: `PathStep.operator` still says `var` for `val`, and a lone argument written without its array is still reported as `/0`.
  - CORE-07: `try_build` now refuses a depth of 0 and the docs explain the setting. No new error kinds.
  - CORE-14: one internal body now serves every entry point. `evaluate` and `evaluate_metered` stay written out by hand, because the closure form cost ~1.2 ns per call.
- **Operators:** OPS-02, 03, 04, 05, 06, 10, 12.
- **Operators, partial:**
  - OPS-07: the predicate code stays in `helpers.rs`, because `operator_table_rule_test` pins that path.
  - OPS-08: the strict-eq field fast path is not folded into `FastPredicate`.
  - OPS-11: literal formats are still translated at run time.
  - OPS-14: there is no feature-off-only pinning.
- **Fixed in 5.x as plain bugs:**
  - V6-OPS-03, 09, 12, 14.
  - V6-OPS-04, partly: `CoerceToZero` and the non-finite strings. Whether `- / % min max` should honour `NanHandling` is still open.
- **Rust bindings:** BIND-02, 03, 04 (no wire-shape change), 10, 11 (docs only), 12, 15, 16, 17, 18, 19, 21, 22; HOST-15.
- **Rust bindings, partial:**
  - BIND-09: covers only the call types every runner already has; WASM now runs the conformance suites.
  - BIND-20: the WASM `require` condition is kept on purpose.
  - BIND-23: each binding pins its own behaviour.
- **Host bindings:** HOST-03, 04, 05, 06, 09, 11, 12, 14 (docs), 16, 17, 19, 20; INFRA-22. Also fixed: PHP failed to find its native library on Windows (a case-sensitive `AMD64` match).
- **Host bindings, partial:**
  - HOST-10: per-host null-element handling is kept.
  - HOST-22: close racing evaluate is not tested.
- **Infra:** INFRA-01, 02, 03, 04, 06, 07, 09, 11, 12, 13, 14, 15, 16, 19, 21, 23, 24, 25, 27, 28, 29, 30, 31, 32; HOST-13, 18; UI-03.
- **Infra, partial:**
  - INFRA-08: the release runs all of CI on the tag; publishing auth is unchanged.
  - INFRA-17: the README only.
  - INFRA-26: `join_all` is still used.
  - HOST-08: the smoke job is not a publish gate yet.
- **UI:** UI-01, 02, 05, 09, 11, 12, 14, 15, 17, 18, 21.
- **UI, partial:**
  - UI-04: the bundled dependencies are now devDependencies and the stray `sourceMappingURL` comments are gone; how the WASM ships is unchanged.
  - UI-10: one palette now. Defining the undefined tokens needs a design decision.
  - UI-13: no tests against the peer-version floors.
  - UI-16: `design-system.html` and the playground token copies are kept.
  - UI-19: the per-node `expression` stays, because it is exported.

**Deferred, for v6 or a decision.** Each of these is a new API, a behaviour or design change, or work outside the repo.

| Area | Items |
|---|---|
| Core | CORE-09, 10, 11, 12, 18, 21, 25, 26 |
| Operators | OPS-09, OPS-13 |
| Rust bindings | BIND-05, 06, 07, 08, 13, 14 |
| Host bindings | HOST-07, HOST-21 |
| Infra | INFRA-05, 10, 18, 20, 33 |
| UI | UI-06, 07, 08, 20 |
| Part 2 | Every V6 item not listed above |

**Verified at the batch head**

| Check | Result |
|---|---|
| Core, all features | 838 tests, 0 failures |
| Core, no default features | 278 tests, 0 failures |
| CI conformance feature combinations | all 10 pass |
| C ABI | 47 |
| Go | passes, including under `-race` |
| .NET | 114 |
| PHP | 113 |
| Node | 171 |
| Python | 2,926 |
| WASM | 66 |
| UI | 1,337 |
| `cargo deny` | 5 workspaces pass |
| clippy `-D warnings` | clean everywhere |

The JVM was not compiled: only JDK 17 is installed, and the binding needs 22 or newer.

**Benchmarks against 5.8.0 (`c30cd5d`)**
- On the 43 suites whose cases did not change, the geomean is +1.0%.
- Folded rules are back at baseline after `50155fb`.
- The macro suites are within +3% (checkout-40 +1.4%).

**WASM size gate:** +4.05% over its recorded baseline. That is a warning (the gate fails at +5%), and the growth comes from this batch's fixes.

**For the maintainer**
1. OIDC Trusted Publishing for crates.io and npm.
2. Make `release-smoke-hosts` a publish gate once it has passed on a real release.
3. Decide whether to refresh the WASM size baseline.

New findings from this work:
- **CORE-26:** a debug build overflows a 2 MiB thread when it evaluates a rule nested about 100 deep. Release builds are fine, and the behaviour predates this work. Dispatch frames are large in debug builds, and the compile-depth cap is 256. Severity Low, effort M, target 5.x.
- **INFRA-34:** `cargo clippy -p datalogic-rs --all-targets` fails with default features, because test code in `src/path.rs:215` calls `engine.trace()` without `cfg(feature = "trace")`. Severity Low, effort S, target 5.x.
- **New:** .NET calls `GC.KeepAlive(this)` only after the throw paths. On an error path a finalizer could run mid-call (latent, left alone).
- **New:** `to_json` of a folded literal object re-parses as an operator call outside templating mode (JSONLogic v5 has no literal-object syntax), and a folded datetime literal serializes as a plain string.
- **New:** `min`, `max`, `abs`, `ceil` and `floor` still go through `f64` for integers above 2^53.
- **BIND-24:** now that the core reports `format_date "%Q"` as an error, the Node panic-safety tests no longer reach a real panic, so `guard()` is only exercised indirectly. Its before and after behaviour was checked against 5.8.0. A panic probe built only under a test feature would restore coverage. Severity Low, effort S, target 5.x.

---

# Part 1: Can ship in 5.x (non-breaking)

## 1.1 Core crate: engine, compile, runtime

Paths are relative to `crates/datalogic-rs/src/`.

**✅ CORE-01: A rule can build a value nested deep enough to overflow the stack** · High · M
- **Where:** `engine/dispatch.rs:198-247` (array and structured-object literals). The output conversions that recurse are `result_output.rs:36-58`, `arena/value/conversion.rs:27-50`, `serde_bridge.rs:26-41`, and datavalue's `Display` / `to_owned`.
- **Problem:** `{"reduce":[{"var":"xs"},[{"var":"accumulator"}],null]}` over 1M items evaluates fine. `eval_str` then aborts with a stack overflow, exit 134. `MAX_COMPILE_DEPTH` limits how deep a rule can nest, not how deep a value can get at runtime. `ops_budget` is off by default.
- **Fix:** Track nesting depth while building arrays and objects, or check depth with an iterative walk at the output boundary. Return an error past 256. Alternatively, make datavalue's serialization and `to_owned` iterative. A new error kind is fine because `ErrorKind` is `#[non_exhaustive]`.

**CORE-02: `Logic::to_json` and the trace `ExpressionNode.expression` emit unescaped JSON** · High · S
- **Where:** `node_serialize.rs:41-176`. About 15 sites build strings with `format!("\"{}\"", …)`.
- **Problem:** `{"var":"a\"b"}` serializes to `{"var": "a"b"}`. The same happens for `missing` and `exists` paths, template keys and `throw` strings. The output cannot be re-parsed, although the module doc promises a round trip. The UI falls back to placeholder nodes because of this (see UI-07).
- **Fix:** Write every string through a JSON escaper (datavalue's, or `serde_json::to_string`). Add a round-trip proptest over arbitrary strings.

**CORE-03: `to_json` silently changes the meaning of `val` with dotted keys** · High · S
- **Where:** `node_serialize.rs:128-150`.
- **Problem:** `{"val":"a.b"}` and `{"val":["a.b"]}` both serialize to `{"var":"a.b"}`. Against `{"a.b":1,"a":{"b":2}}`, the original rule returns 1 and the round-tripped rule returns 2.
- **Fix:** Record on `CompiledNode::Var` whether the source was `val` or `var`, or emit `{"val":[...]}` whenever a segment contains `.`. Pin it with a round-trip test (see CORE-13).

**✅ CORE-04: Constant folding bakes the compiling engine's config into the `Logic`** · High · S (5.x step)
- **Where:**
  - `compile/optimize/constant_fold.rs:217-225`
  - `compile/walker.rs:229-237, 340-350`
  - `compile/optimize/dead_code.rs`
  - the CSE and strength-reduction gates at `compile/mod.rs:90-98` and `compile/optimize/mod.rs:54-57`
- **Problem:** `{"/":[1.5,0]}` is compiled on the default engine and evaluated on a `DivisionByZeroHandling::ReturnNull` engine. It returns `1.797e308`. The same rule with a non-constant operand returns `null` on that engine. Truthiness, NaN handling, loose equality and coercion are all frozen the same way. Hot reload through `to_builder()` makes evaluating on a different engine common. This goes beyond proposal O3, which covers only custom-operator lookup.
- **Fix (5.x):** Store a fingerprint of the folded config, or the engine id, in `Logic`. On a mismatch, recompile, or warn in debug builds. Document it next to `Logic::compiled_on`.
- **v6 follow-up:** V6-CORE-01.

**CORE-05: `Error::resolve_path` returns the wrong pointer and argument index when the rule was not compiled for tracing** · Medium · M
- **Where:**
  - `node/mod.rs:274-283`: a `var` default is visited as index 0, and template fields are visited by index, not by key.
  - `path.rs:93-145`: `val` is rendered as `/var/…`.
- **Problem:** `{"var":["x",{"+":[1,"a"]}]}` resolves to `/var/0`. A traced compile correctly gives `/var/1`. A template field `b` resolves to `/1` instead of `/b`. That makes two pointer systems that disagree.
- **Fix:** Have `visit_indexed_children` report the child's real position, as `ChildPos::{Index, Key}`. Alternatively, always record pointers at compile time (`CompileCtx::recording_pointers` exists) and delete the reconstruction in `path.rs`.

**CORE-06: `Error::operator` names a different operator on the plain and traced paths** · Medium · S
- **Where:** `error/mod.rs:334-347` (the `prefer_existing_op` flag), `engine/mod.rs:880,1028`, `trace.rs:527`, `engine/dispatch.rs:282`.
- **Problem:** For a failing custom operator nested in `+` inside `if`, the plain path reports `+` and the traced path reports `if`. Neither reports the custom operator that actually failed.
- **Fix:** Use one rule everywhere: the innermost failing operator, including custom operators. Delete the flag. Note the change in the CHANGELOG.

**CORE-07: Depth and structural failures are reported under the wrong error kind** · Medium · S (5.x step)
- **Where:**
  - `node/compile_ctx.rs:143-149` and `engine/mod.rs:1247-1257`: reported as `ConfigurationError`.
  - `compile/walker.rs:91`: a multi-key object gives `InvalidOperator("Unknown Operator")`, but that variant's payload is meant to be the operator name.
  - `config.rs:103-116, 411-414, 650-661`.
- **Problem:**
  - A 300-deep rule passed as a `serde_json` value gives `ConfigurationError`. The same rule as text gives `ParseError`.
  - `max_recursion_depth` limits only how often custom operators re-enter the engine, not how deep a rule nests.
  - The builder accepts `0`, but the JSON config rejects it.
- **Fix (5.x):** Add `ErrorKind`/`ErrorCode` variants `DepthExceeded` and `NotAnOperator`. Both enums are non-exhaustive, so this is additive. Validate `max_recursion_depth >= 1` in the builder, and document what the setting actually limits.
- **v6 follow-up:** V6-CORE-08.

**CORE-08: Truthiness is implemented three times, kept in step by hand** · Medium · S
- **Where:** `arena/value/mod.rs:37-53` with `arena/value/strings.rs:122-142`, `operators/truthy.rs:18-50` (used by folding), and `truthy_input.rs:58-81`.
- **Problem:** The code carries "must stay in lockstep" comments. The serde copy treats every object as truthy, but the arena copy treats an empty tensor as falsy. When the copies drift, folded results stop matching runtime results.
- **Fix:** Write one generic `truthy_by` over a small value-shape trait, with the config match written once.

**CORE-09: Tracing costs quadratic time and memory** · Medium · M
- **Where:**
  - `engine/mod.rs:1289-1313`: every dispatched node deep-copies the current frame into `serde_json::Value`, via `arena/context/mod.rs:300-323`.
  - `trace.rs:43-97`: each node renders the JSON of its whole subtree.
  - `trace.rs:505`: the node tree is rebuilt on every call.
- **Problem:** Cost is O(steps × |data|) plus O(n × depth), so the debugger is unusable on realistic payloads. Separately, the `cfg(all(trace, serde_json))` guards are redundant, because `trace` already implies `serde_json`.
- **Fix:** Snapshot each frame once when it is pushed and reference it by id from each step. Cache the `ExpressionNode` tree on `Logic`. Any new field on `TracedRun` must wait for v6 (see V6-CORE-04), so expose the frame table as a new type.

**CORE-10: `Session` and `SharedSession` never free arena memory on their own** · Medium · S
- **Where:** `session.rs:160-162, 213-321`.
- **Problem:** `SharedSession` is meant to live for a long time, but a host that never calls `reset()` grows without bound.
- **Fix:** The owned-result methods hold no borrow when they return, so they can reset the arena on entry and keep its chunks. Alternatively, add an opt-in `auto_reset` or a memory high-water mark.

**CORE-11: `ParsedData` is not `Sync`, and two bindings work around it with stale `unsafe impl Sync`** · Medium · M
- **Where:** `parsed_data.rs:18-36`; `bindings/c/src/data.rs:26-36`; `bindings/python/src/data.rs:45-60`.
- **Problem:** The two SAFETY comments say "verified against datavalue 0.2.2". The project now uses 0.3.1, which adds `Tensor`. A re-audit found the impl still sound, but the core crate's `forbid(unsafe_code)` has pushed the unsafe code into the bindings.
- **Fix:** Freeze `ParsedData` into a `Sync` owner after construction, using a PreLit-style owned value plus a boxed spine. Adding `Sync` is additive. Then delete the `unsafe impl`s in the bindings.

**CORE-12: `cfg(feature)` sprawl, and the node-shape match is copied about 13 times** · Medium · M
- **Where:**
  - 369 cfg sites across the crate, 256 of them outside `operators/`. The busiest files are `arena/context/mod.rs` (39), `engine/mod.rs` (27), `cse.rs` (19) and `node/mod.rs` (17).
  - Exhaustive `CompiledNode` matches: `node/mod.rs:220,250,308,410`, `node/logic.rs:339`, `engine/dispatch.rs:40`, `node_serialize.rs:17`, `trace.rs:43`, `cse.rs:395,579`, `facts.rs`, `compile/scope.rs`, `path.rs`.
  - Trace cfgs sit on the hot path: `engine/mod.rs:1289-1334`.
- **Problem:** Every new node kind or feature touches about 13 sites. `visit_children_mut` is a hand-written mirror of `visit_indexed_children`. Proposal O8 gives the 5.8.0 baseline as 318 cfg sites, which is already out of date.
- **Fix:** Give children uniform storage: `children()` and `children_mut()` returning slices plus a `ChildPos`. Add the proposal's §5 "no cfg in `engine/`" grep to CI now. Update O8's baseline.

**CORE-13: Four tree walks must agree, and two of them have no differential test** · Medium · S
- **Where:** `compile/walker.rs`, `check.rs:243-420`, `facts.rs`, `node_serialize.rs` + `path.rs`.
- **Problem:** `check_and_compile_agree` exists, but nothing tests that a rule survives a `to_json` round trip (CORE-02, CORE-03), or that `resolve_path` matches the recorded pointers (CORE-05).
- **Fix:** Add two proptests:
  - `compile(to_json(compile(r)))` evaluates the same as `r`;
  - `resolve_path` returns the same pointer as a traced compile.

**CORE-14: Read projection is applied on some entry points and not others** · Low–Medium · M
- **Where:**
  - Applied: `engine/mod.rs:876`, `session.rs:247`.
  - Not applied: `trace.rs:513`, the one-shot `Engine::eval*` and top-level `eval*` (`engine/mod.rs:1207`, `eval_input.rs:264-310`, `roots.rs:298-307`), and evaluation on a different engine (`node/logic.rs:176-178`).
- **Problem:** The same rule and data have different cost cliffs depending on the entry point. The near-identical bodies are at `engine/mod.rs:869-882,1012-1030`, `session.rs:242-361` and `trace.rs:497-533`.
- **Fix:** Route every entry point through one internal `run(compiled, input, arena, meter, tracer)`. This is the non-breaking half of proposal O1/O8.

**CORE-15: The CSE pass is quadratic** · Low–Medium · M
- **Where:** `compile/optimize/cse.rs:115-395`.
- **Problem:** `is_cse_pure`, `contains_iterator_op`, `node_count` and `subtree_hash` each re-walk the subtree at every node. The module doc claims a bottom-up hash, which the code does not do.
- **Fix:** Do one post-order pass that memoizes `(hash, size, pure, has_iter)` per node.

**CORE-16: Projection searches keys linearly** · Low–Medium · S
- **Where:** `projection.rs:76-110`, `roots.rs:228-232`.
- **Problem:** Cost is O(input keys × read keys) per object level, which hurts wide contexts.
- **Fix:** Sort the children at build time and binary-search them, or use a map once there are more than 8 children.

**CORE-17: Template keys are copied into the arena on every evaluation** · Low · S
- **Where:** `engine/dispatch.rs:219-224`.
- **Problem:** The key is already a `&'a str` that lives long enough, so `arena.alloc_str(key)` is unnecessary.
- **Fix:** Drop the copy.

**CORE-18: Some builder settings silently do nothing** · Low · S
- **Where:** `engine/mod.rs:448`; `builder.rs:96-101, 147-152`.
- **Problem:** `with_templating(true)` without the `templating` feature quietly gives a strict engine. `with_template_key_escape` has no effect without templating.
- **Fix:** Make `try_build` return `ConfigurationError` for these. Leave `build()` unchanged.

**CORE-19: Compiling from an `&OwnedDataValue` deep-clones the rule** · Low · S
- **Where:** `logic_input.rs:9-10, 57-63`.
- **Problem:** The doc calls the clone "cheap; usually just an Arc bump", but `OwnedDataValue` contains no `Arc`, so the whole rule is copied.
- **Fix:** Add an internal method on the sealed trait that returns a `Cow`, and correct the doc.

**CORE-20: The compile walker takes a vestigial `Option<&Engine>`** · Low · S
- **Where:** `compile/walker.rs:25-457`. The `None` branches at `:123-126` and `:154-156` are dead.
- **Problem:** The only caller always passes `Some`. The comment at `walker.rs:114` cites an MSRV of 1.85, but the crate requires 1.98.
- **Fix:** Take `&Engine`, delete the dead branches and the stale comment.

**CORE-21: Every literal is boxed, and nested composite literals are stored repeatedly** · Low · M
- **Where:** `node/prelit.rs:30-112`; `node/populate.rs:36-39`.
- **Problem:** Each scalar literal costs a heap allocation. Nested arrays take O(depth × size) memory. Tensor literals get no `PreLit`, so they are re-viewed on every evaluation.
- **Fix:** Store `Static` values inline, and share one `Arc` spine across levels. Re-measure the 48-byte layout test afterwards.

**CORE-22: Doctests teach comparing `err.tag()` strings** · Low · S
- **Where:** `engine/mod.rs:1001`, `config.rs:431`, `builder.rs:245`.
- **Problem:** The examples match on tag strings, which contradicts the guidance to match on `ErrorCode`.
- **Fix:** Rewrite them as `err.code() == ErrorCode::…`.

**CORE-23: Stale docs and comments** · Low · S
- **Where:**
  - `node/mod.rs:95-97` says "64 canonical operators"; there are 84.
  - `node/mod.rs:118` says `Engine::add_operator`; the method is on the builder.
  - `engine/mod.rs:97-99` mentions a "preserve-structure flag".
  - `error/path.rs:3` describes a boxed layout the type no longer has.
  - `compile/optimize/mod.rs:9-37` describes a pass API that doesn't exist.
  - `compile/mod.rs:62-66`: a cfg attribute splits a doc comment in two.
  - `node/populate.rs:79-86` describes a re-populate step that clones don't do.
  - `engine/mod.rs:347-359`: `Engine`'s `Debug` output omits `families` and `constant_folding`.
- **Fix:** Correct the text, and include every setting in `Debug`.

**CORE-24: Settings are duplicated between `EngineBuilder` and `Engine`** · Low · S
- **Where:** `builder.rs:51-61, 289-308, 384-394`; `engine/mod.rs:196-223, 396-454`.
- **Problem:** Six settings are passed positionally in both directions, including two adjacent `bool`s that are easy to swap. `checked_names` is lost on `to_builder()`.
- **Fix:** Have both own a private `EngineSettings` struct, and carry `checked_names` across.

**CORE-25: Three files are oversized** · Low · M
- **Where:** `engine/mod.rs` (1,338 LOC, mostly rustdoc tables duplicated through `cfg_attr` doc fragments), `arena/context/mod.rs` (1,085), `compile/optimize/cse.rs` (999).
- **Fix:**
  - Split `engine/` into `eval`, `compile` and `introspect` modules.
  - Replace the paired `cfg_attr(doc=…)` blocks with `doc(cfg(..))`.
  - Move CSE's structural hash and equality into `cse/structural.rs`.

## 1.2 Core crate: operators

Paths are relative to `crates/datalogic-rs/src/operators/`.

**✅ OPS-01: `format_date` panics on an invalid or raw format specifier** · High · S
- **Where:** `datetime/mod.rs:211` (`%` pass-through), `:357`, `:368`, and datavalue's `DataDateTime::format`.
- **Problem:** `{"format_date":["2024-01-01T00:00:00Z","%Q"]}` panics, and so does the format `"abc%"`: chrono returns `fmt::Error` and `to_string()` panics on it.
- **Fix:** Validate the format with `StrftimeItems` (no `Item::Error`), or `write!` into a buffer and map the failure to `InvalidArguments`. Fix datavalue too, and add a suite case.

**OPS-02: Tensor constructors allocate unbounded memory when no budget is set** · Medium · S
- **Where:** `tensor/construct.rs:96-131` (`zeros`, `full`), plus `scatter`, `one_hot` and `pad`; `tensor/mod.rs:91`.
- **Problem:** `{"zeros":[[1000000,1000000],"f64"]}` asks for about 8 TB. `numel_of` checks only for `usize` overflow, and engines have no budget by default.
- **Fix:** Add a hard cap on elements and bytes, as engine config with a sane default, and check it in `numel_of`.

**OPS-03: `fractional` weight arithmetic overflows** · Medium · S
- **Where:** `flagd.rs:192-209`.
- **Problem:** Weights `[i64::MAX, 1]` give `null`, because the sum wraps. Weights `[2^40, 2^40]` pick a different variant than `[1,1]`. Debug builds panic. The comment assumes weights never exceed 2^31, but nothing clamps them.
- **Fix:** Clamp weights to `i32` as flagd does, use `checked_add`, and multiply in `u128`.

**OPS-04: Metered op counts depend on data types, tracing and config** · Medium · M
- **Where:** `array/helpers.rs:842-860`, `array/reduce.rs:42-47, 200-204`, `engine/mod.rs:1298`, `array/fast_paths.rs:43`.
- **Problem:** Changing one element from `4` to `"4"` changes the cost: `filter` goes from 5 to 13 ops, and `reduce(map)` from 5 to 26. When a fused fast path bails out it charges twice. Tracing and `MissingVar::Error` also change the count. This is a concrete case of proposal O9.
- **Fix:** Charge the fast path what the general path would, precomputing the body's node count at populate time, and don't re-charge after a bail. Add an oracle property test that checks fast and general paths give the same count.

**OPS-05: The `cost` column in `table.rs`, published through `OperatorInfo.cost`, contradicts what the code charges** · Medium · S
- **Where:** `table.rs:485-611` vs `array/length.rs:15`, `array/group_by.rs:52`, `arithmetic/basic.rs:215,402`, `div_mod.rs:141`, `comparison/mod.rs:374,386`, `missing.rs:66,265`.
- **Problem:**
  - `length` declares `Node` but charges per byte.
  - `group_by` declares `PerItem` but charges n·g.
  - Arithmetic, comparisons, `min`/`max` and `missing*` declare `Node` but charge per item.
- **Fix:** Correct the rows. Add a test that meters each row at n and 2n and checks the declared growth class.

**OPS-06: Datetime arithmetic works only with two arguments, and in one order** · Low · S–M
- **Where:** `arithmetic/basic.rs:79-84, 272-277`, `arithmetic/helpers.rs:237`, `datetime/arith.rs:79-91`.
- **Problem:** `{"+":[dt,"1d","1d"]}` and `{"+":["1d",dt]}` raise NaN today. Making them work is a bug fix.
- **Fix:** Fold datetimes in the variadic path and make `+` commutative.

**OPS-07: `array/helpers.rs` (1,292 LOC) mixes four concerns** · Low · S
- **Where:** `array/helpers.rs`.
- **Fix:** Split it into `predicate.rs`, `input.rs` and `fused.rs`. The split changes no behavior, and it turns the proposal's later move of fast paths into `plan::specialise` into a file move.

**OPS-08: Fast-path detection is duplicated** · Low · S
- **Where:**
  - The "plain scope-0 var" check is written out seven times: `array/helpers.rs:72,215,253,277,775,1108` and `array/sort.rs:148`.
  - `filter.rs:52` has a second strict-equality fast path, which makes `FastPredicate::StrictEq` dead for `filter`.
  - `combine` and `combine_ints` (`helpers.rs:1001,1067`) copy `arithmetic/helpers.rs:65`.
- **Fix:** Reuse `plain_var_segments`, fold the field fast path into `FastPredicate`, and share one integer-op helper.

**OPS-09: Numeric coercion is implemented about six ways (refactor only)** · Low–Medium · M
- **Where:** `comparison/loose.rs:60`, `extract.rs:117,131`, `datetime/arith.rs:6-126`, `arithmetic/min_max.rs:85`, and `coerce_to_number_cfg`.
- **Fix:** Create one coercion module with named modes, keeping every current behavior.
- **v6 follow-up:** V6-OPS-10 unifies the semantics.

**OPS-10: NaN errors come from two different constructors** · Medium · M (5.x step)
- **Where:** `comparison/loose.rs:162` and `array/slice.rs:144-154` use `InvalidArguments("NaN")`; arithmetic and ordering use `Thrown{"type":"NaN"}`.
- **Problem:** The same failure surfaces as two different errors.
- **Fix (5.x):** Route every NaN site through one internal constructor, with no change to any message text.
- **v6 follow-up:** V6-OPS-11.

**OPS-11: Avoidable allocations** · Low · S–M
- **Where:**
  - `string.rs:303,311`: `split` copies parts that are already in the arena.
  - `array/slice.rs:122`: stepped string slices go through a heap `Vec<char>`.
  - `datetime/mod.rs:276-416`: results are copied from a heap `String`, and the format string is re-translated on every call.
  - `missing.rs:242-275`: arena strings are copied again.
  - `variable/val.rs:211,292,307,392`: `ok_or(Error…)` builds an error even on success.
  - `comparison/mod.rs:229-236`: literal datetimes are re-parsed on every comparison.
- **Fix:** Borrow sub-slices, use `ok_or_else`, translate literal formats and parse literal datetimes once at compile time.

**OPS-12: Three copies of the type-name table** · Low · S
- **Where:** `error_handling.rs:106,126`, `inspect.rs:65-79`.
- **Problem:** The copies differ from what the `type` operator returns.
- **Fix:** Keep one `type_name()`, with an owned-value twin.

**OPS-13: `parse_date` mirrors datavalue's parser** · Low · S
- **Where:** `datetime/mod.rs:299-308`.
- **Fix:** Expose one naive-parse helper from datavalue and call it.

**OPS-14: The suites miss known edge cases** · Low–Medium · S
- **Where:** `crates/datalogic-rs/tests/suites/`.
- **Problem:** Nothing pins these cases:
  - an invalid `format_date` specifier;
  - an object carrying a `datetime` key under `===` or `distinct`;
  - results with the datetime feature on vs off;
  - integers above 2^53;
  - `CoerceToZero` with `*` (currently pinned wrong, in `tests/config_test.rs`);
  - fast-path vs general-path op counts;
  - large `fractional` weights.
- **Fix:** Add the cases. Where a current result is intended to change in v6, pin it with a comment saying so.

## 1.3 Rust bindings, C ABI and `datalogic-bind`

Paths are relative to the repo root.

**✅ BIND-01: A panic in the Node binding aborts the whole process** · High · S
- **Where:** `bindings/node/src/*.rs` (no `#[napi(catch_unwind)]` anywhere); `bindings/node/src/engine.rs:534-553` (`AsyncTask::compute`).
- **Problem:** Any panic in core, such as OPS-01, takes down the host Node process. The C ABI guards every entry (`bindings/c/src/lib.rs:178-201`), and pyo3 converts panics to `PanicException`. WASM's `panic = "abort"` turns a panic into `RuntimeError: unreachable`, which leaves the in-flight object's borrow flag set.
- **Fix:** Add `catch_unwind` to every napi method and to `compute`, rejecting with `InternalError`. Document WASM's abort behavior.

**BIND-02: A C ABI callback that re-enters its own session is undefined behavior** · High · S
- **Where:** `bindings/c/src/session.rs:47-61, 142-149`, `bindings/c/src/builder.rs:471`, `bindings/c/include/datalogic.h:14-17`.
- **Problem:** A custom-operator callback that calls `session_evaluate*` or `session_reset` on the session currently evaluating creates a second `&mut Session` and resets the arena while it is still borrowed. The pooled-arena path guards against this (`rule.rs:30-34`); the session path does not, and the header doesn't mention it.
- **Fix:** Add a `busy: Cell<bool>` to the session and return `InvalidArg` on re-entry. Document the rule in the header's threading section.

**BIND-03: Plumbing that belongs in `datalogic-bind` is duplicated across bindings** · Medium · M
- **Where:**
  - Engine-option assembly, three copies: `bindings/node/src/engine.rs:126-194`, `bindings/python/src/engine.rs:84-151`, `bindings/wasm/src/lib.rs:550-583, 1312-1396`. WASM applies families in a different order from the other two.
  - Error-path serialization, three copies with different key casing: `c/src/error.rs:163`, `node/src/error.rs:203`, `python/src/error.rs:162`.
  - Typed-result extraction, four copies whose messages have already drifted: "not an integer number" vs "not a safe integer".
  - C re-implements the custom-operator args and result handling: `c/src/builder.rs:454-502`.
  - The same-engine check is copied: `c/src/session.rs:55`, `python/src/session.rs:44`.
- **Fix:** Add `datalogic_bind::{EngineOptions::apply, path_value, typed::{bool,int,float}}` and use them in every binding.

**BIND-04: Batch items are built by four separate formatters** · Medium · M (5.x step)
- **Where:** `bindings/c/src/error.rs:146-160`, `bindings/c/src/session.rs:420-608`, `bindings/node/src/session.rs:31-42`, `bindings/wasm/src/lib.rs:1209-1241`, `bindings/python/src/session.rs:91-160`.
- **Fix (5.x):** Move `ItemError::from_engine` and its serializer into `datalogic-bind` without changing any shape. Add `node_ids` and `path` to the item JSON, which is additive.
- **v6 follow-up:** V6-BIND-02. Overlaps proposal §3.9.

**BIND-05: Error objects carry different field names per binding** · Medium · M (5.x step)
- **Where:** `bindings/node/src/error.rs:116-201`, `bindings/wasm/src/lib.rs:103-162`, `bindings/python/src/error.rs:131-172`, `bindings/c/src/error.rs:163-177`.
- **Problem:**
  - The kind field is `errorType` in Node and `type` in WASM; node ids are `nodeIds` in Node and `node_ids` elsewhere.
  - WASM errors have no `path`.
  - A typed mismatch is `EvaluateError` in Node but `TypeMismatch` in WASM.
  - A `ConfigurationError` at construction surfaces as `EvaluateError`.
  - Node uses snake_case in facts but camelCase in errors.
- **Fix (5.x):** Additive aliases only. WASM gains `errorType`, `nodeIds` and `path`; Node gains `code` and `type`. Build all of them from one `datalogic_bind::error_wire(err, Option<&Logic>)`. Add a status accessor for JVM, PHP and Go (see HOST-10).
- **v6 follow-up:** V6-BIND-01.

**BIND-06: A rule compiled on another engine is handled four different ways** · Medium · S (5.x step)
- **Where:**
  - C checks every path: `bindings/c/src/session.rs:55-59`.
  - Python checks only some paths: `bindings/python/src/session.rs:165-269`.
  - Node and WASM never check: `node/src/session.rs:63-306`, `wasm/src/lib.rs:860-1096`.
- **Problem:** Where nothing checks, custom operators are resolved by name, the "name fallback" in proposal §2.
- **Fix (5.x):** Emit a deprecation warning when the engines differ.
- **v6 follow-up:** V6-BIND-04.

**BIND-07: C ABI errors drop details the other bindings expose** · Medium · S
- **Where:** `bindings/c/src/error.rs:43-60`.
- **Problem:** Go, JVM, .NET and PHP cannot read a `throw` payload, `budget` or `spent`. Node and Python keep `Thrown` only inside the message.
- **Fix:** Add `datalogic_error_json` (ABI minor 2) returning the serialized `Error`. Attach `detail` in Node and Python.

**BIND-08: C custom operators have no destructor for `user_data`** · Medium · S
- **Where:** `bindings/c/src/builder.rs:359-437`.
- **Problem:** Freeing the engine never tells the host, so host wrappers must keep their own registry or leak. This is the root cause of HOST-01 and HOST-02.
- **Fix:** Add `datalogic_engine_builder_add_operator_ex(..., free_fn)`, called when the last `Arc<Engine>` drops. Bump the ABI minor.

**BIND-09: The scenario suite skips exactly the areas where bindings diverge** · Medium · M
- **Where:** `bindings/scenarios/api.json` (46 cases); `bindings/wasm/tests/*.rs`; `bindings/node/__test__/object-bridge.test.mjs:12-30`.
- **Problem:**
  - No cases for batch, typed results, `DataHandle`, custom operators, engine mismatch, invalid budget or mode, or the error field shape.
  - WASM never runs the JSONLogic conformance suites.
  - Node compares the object path with the string path on only a slice of cases.
- **Fix:** Add scenario call types for these areas, and a full conformance runner for WASM.

**BIND-10: The header-sync test checks parameter counts only for PHP** · Medium · M
- **Where:** `bindings/c/tests/header_sync.rs:1-134`.
- **Problem:** About 50 JVM `FunctionDescriptor`s and about 50 .NET `LibraryImport`s are checked by name only. A changed C signature would pass the test and corrupt the stack at runtime. The JVM also hard-codes `SIZE_T = JAVA_LONG`.
- **Fix:** Parse parameter counts and type tokens for JVM and .NET. Alternatively, generate the bindings (jextract, or csbindgen / ClangSharp).

**BIND-11: Aliases that v6 removes were never deprecated** · Low · S
- **Where:** `bindings/node/src/lib.rs:33-51`, `bindings/python/src/lib.rs:29-34`, `bindings/python/datalogic_py.pyi:99`, `bindings/wasm/src/lib.rs:271`, `bindings/node/src/data.rs:30`.
- **Problem:** `apply` has no `@deprecated` and no `DeprecationWarning`. It, and `builtinOperatorNames`, build a fresh `Engine` on every call. The WASM deprecations exist only as JSDoc. The `DataHandle` docs still recommend `evaluateNumber`.
- **Fix:** Add the deprecation markers plus a one-time runtime warning, and correct the docs.

**BIND-12: The trace result re-orders object keys** · Low · S
- **Where:** `crates/datalogic-bind/src/lib.rs:118-121`.
- **Problem:** The result is re-parsed into a `serde_json::Value` without `preserve_order`, which sorts the keys.
- **Fix:** Splice the result in as a `RawValue`. The broader ordering policy is V6-BIND-07.

**BIND-13: Custom-operator exceptions lose the original error** · Low · M
- **Where:** `bindings/python/src/engine.rs:516-518`, `bindings/node/src/engine.rs:627-629`, `bindings/wasm/src/lib.rs:463-468`.
- **Problem:** The host exception is flattened into a string, so its class, traceback and `cause` are gone.
- **Fix:** Attach the original exception as `__cause__` in Python and `cause` in JS.

**BIND-14: Metered evaluation is missing from several tiers** · Low · M
- **Where:** the Node and Python `Session` classes; the C metered entry point (`bindings/c/src/session.rs:173`).
- **Problem:** Node and Python sessions have no metered method; WASM's does. The C ABI has a metered call for JSON text only, with no data-handle, typed or batch variant.
- **Fix:** Add the missing methods, which is additive.

**BIND-15: Binding docs disagree with the code** · Low · S
- **Where:**
  - `bindings/wasm/src/lib.rs:373,523`: the documented error is wrong.
  - Config-key lists omit `missing_var` and `ops_budget`: `c/src/builder.rs:299`, `node/src/engine.rs:36`, `python/src/engine.rs:59`, `wasm/src/lib.rs:360,527`.
  - `node/src/engine.rs:110-116` claims instances can be shared across workers, which `node/src/data.rs:14-18` says napi cannot do.
  - Stale comments in the Node, C and Python `Cargo.toml`s.
- **Fix:** Correct the text, and generate the config-key list from one constant.

**BIND-16: C builder setters fail silently** · Low · S (5.x step)
- **Where:** `bindings/c/src/builder.rs:169-392`.
- **Problem:** Setters on an already-built builder return `Ok` and do nothing. `set_templating` and `set_strict_operator_names` return `void`. `try_add_operator(..).ok()` can leave the builder empty.
- **Fix (5.x):** Return `InvalidArg` on a drained builder.
- **v6 follow-up:** V6-BIND-06.

**BIND-17: Python holds the GIL while compiling** · Low · S
- **Where:** `bindings/python/src/engine.rs:177-204, 535-550`.
- **Problem:** Only evaluation releases the GIL; compiling a large rule blocks every other Python thread.
- **Fix:** Detach the GIL during the compile step. Plan a free-threaded (3.13t) build.

**BIND-18: Node's `allocatedBytes` wraps above 4 GiB** · Low · S
- **Where:** `bindings/node/src/data.rs:79`, `bindings/node/src/session.rs:319`.
- **Problem:** The value is cast `as u32`.
- **Fix:** Return `f64`.

**BIND-19: `datalogic-bind` helpers swallow errors and misreport types** · Low · S
- **Where:** `crates/datalogic-bind/src/lib.rs:29-43, 131, 278`.
- **Problem:** `unwrap_or_default()` returns `""`, which is not valid JSON. `type_of` reports DateTime and Duration as `"object"`, although they serialize as strings.
- **Fix:** Fall back to valid JSON or return an error, and map DateTime and Duration to `"string"`.

**BIND-20: Package and build metadata problems** · Low · S
- **Where:** `bindings/wasm/build.sh`, `bindings/python/pyproject.toml`, `bindings/python/Cargo.toml`, `bindings/*/Cargo.toml`.
- **Problem:**
  - The WASM `package.json` exists only as a heredoc in `build.sh`. It sets `engines.node >=16` while the Node package requires 18, and its `require` condition points at the ESM build.
  - The Python classifiers stop at 3.13.
  - pyo3's `extension-module` feature is enabled twice, which blocks `cargo test`.
  - The binding crates declare `datalogic-rs = "5.0"` but use 5.8 APIs, and depend on the unpublished `datalogic-bind`.
- **Fix:** Check in a `package.json` template and align `engines`. Set `publish = false` and `"5.8"` on the binding crates. Let `datalogic-bind` own the feature list.

**BIND-21: Hand-written typings are not checked against the schemas** · Low · M
- **Where:** `bindings/node/src/engine.rs:234-435` (`ts_return_type` strings), `bindings/python/datalogic_py.pyi`, `crates/datalogic-bind/src/lib.rs:213`.
- **Problem:** `scoped_arg` can be emitted as a Debug-formatted string that the TypeScript type doesn't allow. The generated `index.d.ts` is never type-checked in CI.
- **Fix:** Generate the typings from `schemas/*.v1.json`, or add a test that compares them.

**BIND-22: The C ABI error-status mapping has an unreachable branch** · Low · S (5.x step)
- **Where:** `bindings/c/src/error.rs:69-94`.
- **Problem:** The `"InternalError" => Internal` branch can never run, because `ErrorCode` has no such variant. `CompileError` maps to `Parse`, and `ConfigurationError` and `BudgetExceeded` map to `Eval`.
- **Fix (5.x):** Delete the dead branch.
- **v6 follow-up:** V6-BIND-06.

**BIND-23: The same bad input fails with a different error tag per binding and per call** · Medium · S (5.x step)
- **Where:**
  - `"InvalidArgument"`: `bindings/c/src/error.rs:110`, `bindings/python/src/engine.rs:663`, `bindings/wasm/src/lib.rs:1228`.
  - `"InvalidArguments"`: `bindings/node/src/engine.rs:160-697`.
  - WASM throws `ParseError` instead: `bindings/wasm/src/lib.rs:178,640,736`.
- **Problem:** `budget = 0` gives three different tags in three bindings, and Python emits both spellings depending on the call.
- **Fix (5.x):** Pin the current behavior with scenario cases.
- **v6 follow-up:** V6-BIND-03.

## 1.4 Host bindings (Go, JVM, .NET, PHP)

Paths are relative to `bindings/`.

**✅ HOST-01: Go: closing or finalizing an Engine breaks custom operators in live Rules and Sessions** · High · S–M
- **Where:** `go/datalogic.go:148-161`, `go/operator.go:220-279`.
- **Problem:** `Close()`, which is also the finalizer, deletes every `cgo.Handle`, while its own doc says Rules keep working after Close. Rules don't reference the Engine, so the GC can collect it. The trampoline then calls `hb.h.Value()` outside its `recover`, so the panic unwinds through C frames and crashes the process. Passing `unsafe.Pointer(hb)` to C also breaks the cgo pointer rules.
- **Fix:** Pass the handle value through a box allocated with C malloc, not a Go pointer. Move handle ownership into a shared registry that Engine, Rule and Session all reference, freed by the registry's finalizer (or BIND-08's destructor). Port the JVM test `rule_with_custom_operator_survives_engine_close_and_gc`.

**✅ HOST-02: .NET: Engine Dispose and its finalizer free GCHandles that live Rules still call** · High · S
- **Where:** `dotnet/src/Datalogic/Engine.cs:28,119,147,281,298,308-324`, `dotnet/src/Datalogic/EngineBuilder.cs:217,297`.
- **Problem:** `Rule`, `Session` and `TracedSession` hold no reference to the Engine. After the Engine is finalized, the trampoline reads a freed `GCHandle`, which either fails or calls the wrong delegate.
- **Fix:** Have Rule and Session hold a reference to the Engine or a callback holder, as the JVM's `Rule.owner` does. Add the matching test.

**HOST-03: Each Go module tag ships six static libraries of about 80 MB each** · High · M
- **Where:** `.github/workflows/release-build-go.yml:32-90`, the `publish-go` job in `release.yml`, `c/Cargo.toml` `[profile.release]`.
- **Problem:** `lib/darwin_arm64/libdatalogic_c.a` is 81 MB. The Go module zip format caps a module at 500 MiB, and every `go get` downloads about 0.5 GB.
- **Fix:** Use a dedicated profile with `strip = "debuginfo"` / `debug = 0`, and add a CI guard on the total size of `lib/`. Longer term, use per-platform submodules or download the native library at build time.

**HOST-04: A builder that never reaches Build leaks, in all four hosts** · Medium · S
- **Where:** `go/operator.go:101`, `jvm/.../EngineBuilder.java:23-46`, `dotnet/.../EngineBuilder.cs:39-47`, `php/src/EngineBuilder.php:20-32`.
- **Problem:** When a setter throws, the caller drops the builder. The native builder, upcall stubs and GCHandles are never freed.
- **Fix:** Add `AutoCloseable` (JVM), `IDisposable` plus a finalizer (.NET), `__destruct` (PHP) and `SetFinalizer` (Go).

**HOST-05: The JVM has no Cleaner, so an unclosed handle leaks for the life of the process** · Medium · S
- **Where:** `jvm/src/main/java/com/goplasmatic/datalogic/{Engine,Rule,Session,DataHandle,TracedSession}.java`.
- **Problem:** The other three hosts have finalizers; the JVM relies entirely on try-with-resources.
- **Fix:** Register each handle with a shared `java.lang.ref.Cleaner`, keeping explicit `close()` as the fast path.

**HOST-06: Close and Dispose are not atomic, though the handles are documented as thread-safe** · Medium · M
- **Where:** JVM `Engine.java:294-300`, `Rule.java:109-117`; .NET `Engine.cs:308`, `Rule.cs:109`, `DataHandle.cs:85`; Go `datalogic.go:150-242`, `data.go:52-60`.
- **Problem:** Two threads closing at once can double-free, and a close racing an evaluate is a use-after-free.
- **Fix:**
  - JVM: `VarHandle.getAndSet`.
  - .NET: `SafeHandle` subclasses, which add a reference during each call.
  - Go: `atomic.Pointer`.
  - Document what happens when close races another call.

**HOST-07: No musl builds and no glibc floor for the C-ABI natives** · Medium · M
- **Where:** `.github/workflows/release-build-c-cdylib.yml:28`, `release-build-go.yml:37`, `jvm/.../NativeLibrary.java:110-114`, `php/src/Native.php:263-277`, `release-build-dotnet.yml` (RID map).
- **Problem:** Python and Node ship musl builds; JVM, .NET, PHP and Go do not, so they fail on Alpine images. The glibc floor quietly follows whatever `ubuntu-latest` has.
- **Fix:** Build `*-linux-musl` artifacts and detect musl in the JVM and PHP loaders. Pin a glibc baseline with zigbuild `.2.17` or a manylinux container.

**HOST-08: JVM, .NET and PHP native loading is only exercised on linux-x64** · Medium · M
- **Where:** `release-build-{jvm,dotnet,php}.yml`; the `jvm-test`, `dotnet-test` and `php-test` jobs in `ci.yml`.
- **Problem:** The macOS and Windows branches of each loader, and the staged runtime directories, never run in CI.
- **Fix:** Add a small matrix that installs each packaged artifact on macOS and Windows (plus arm64) and runs one evaluate and one custom operator.

**HOST-09: .NET targets only net8.0, which reaches end of support on 2026-11-10** · Medium · S (5.x step)
- **Where:** `dotnet/src/Datalogic/Datalogic.csproj:4,58`, `dotnet/tests/Datalogic.Tests/Datalogic.Tests.csproj`.
- **Problem:** CI tests on the .NET 10 SDK through roll-forward only. The package pins `System.Text.Json` 10.0.12, which forces net8 consumers off the in-box version.
- **Fix (5.x):** Multi-target `net8.0;net10.0`, and add the STJ reference only for net8.
- **v6 follow-up:** V6-HOST-07.

**HOST-10: Error and batch decoding is written four times, and the four copies already differ** · Medium · M (5.x step)
- **Where:** Go `error.go:58-86`, `data.go:300-313`; JVM `DatalogicException.java:82-116`, `EvalResult.java:42-53`; .NET `DatalogicException.cs:54-85`, `Session.cs:342-367`; PHP `DatalogicException.php:41-66`, `Session.php` (`collectItems`).
- **Problem:**
  - Malformed items fall back differently: Go sets an empty Type, JVM and PHP use `"InternalError"`, .NET leaves the tag null.
  - A null element is a per-item error in Go, `NullPointerException` in the JVM and `ArgumentException` in .NET.
  - The header documents `{"tag","message"}`, but the item JSON also carries `operator`.
  - Only .NET exposes the coarse status.
- **Fix (5.x):**
  - Align the fallbacks and null handling.
  - Fix the header doc.
  - Add a status accessor to Go, JVM and PHP.
  - Document the exception-mapping table in `BINDINGS.md`.
- **v6 follow-up:** V6-HOST-01, V6-HOST-02.

**HOST-11: PHP Rules and Sessions don't keep their Engine alive, and raw-handle constructors are public** · Medium–Low · S (5.x step)
- **Where:** `php/src/Rule.php:23-27`, `php/src/Session.php:29-33`, `php/src/TracedSession.php:25-29`, `php/src/Engine.php:47-62, 259-266`.
- **Problem:** `new Rule($other->handle())` creates two owners of one native rule, and both free it.
- **Fix (5.x):** Have Rule, Session and TracedSession hold their Engine, and check native-pointer ownership.
- **v6 follow-up:** V6-HOST-03.

**HOST-12: The JVM jar has no module name** · Low–Medium · S (5.x step)
- **Where:** `jvm/pom.xml`; `jvm/.../internal/DatalogicNative.java:43,119`.
- **Problem:** JPMS users can only grant `--enable-native-access=ALL-UNNAMED`, and the public `MethodHandle`s in `internal` let any classpath code free native memory.
- **Fix (5.x):** Add `Automatic-Module-Name` to the jar manifest.
- **v6 follow-up:** V6-HOST-04.

**HOST-13: The Java 22 floor is never tested** · Low · S (5.x step)
- **Where:** `jvm/pom.xml:38-41`; the `jvm-test` job in `ci.yml` (JDK 25 only).
- **Problem:** Java 22 is a non-LTS release that is now end-of-life, and CI runs only JDK 25.
- **Fix (5.x):** Add a JDK 22 CI leg, or document 25 as the tested baseline.
- **v6 follow-up:** V6-HOST-06.

**HOST-14: Naming differences that the `BINDINGS.md` table doesn't cover** · Low · S (5.x step)
- **Where:** Go `datalogic.go`, `operator.go:198`, `introspect.go:73`; JVM `Engine.java`; .NET (`OpenSession`, `ApplyJson`); PHP `DataHandle.php:40`, `Session.php:90`.
- **Problem:**
  - Opening a session is `Session()` in Go, `openSession()` in JVM, .NET and PHP, and `session()` in the Rust bindings.
  - Parsing a data handle: Go `ParseData`, JVM `DataHandle.parse`, .NET `DataHandle.Parse`, PHP `new DataHandle()`.
  - Metered results come back in four different shapes.
  - Go `SetConfigJSON` returns an error; every other Go setter chains.
  - PHP takes the mode as an `int`.
  - `Check` requires a mode in Go and the JVM but defaults it elsewhere.
- **Fix (5.x):** Add rows to the table, plus additive aliases: Go `ConfigJSON(..) *EngineBuilder`, a PHP `CompileMode` enum, PHP `DataHandle::parse`.
- **v6 follow-up:** V6-HOST-08.

**HOST-15: The header-sync test forces declarations no host calls** · Low · S
- **Where:** `jvm/.../DatalogicNative.java:260`; the .NET and PHP declarations of `datalogic_traced_session_evaluate`; `c/tests/header_sync.rs` (`NOT_WRAPPED` is empty).
- **Problem:** Every host calls only the `_mode` variant, and the JVM's `ERROR_STATUS` handle is unused.
- **Fix:** List these in `NOT_WRAPPED` with a reason. V6-BIND-06 removes them from the ABI.

**HOST-16: Docs advertise a "NaN" error tag the engine never produces** · Low · S
- **Where:** JVM `DatalogicException.java:45`, `EvalResult.java:16`; .NET `DatalogicException.cs:16`, `EvaluationResult.cs:9-72`.
- **Problem:** NaN surfaces as a `Thrown` error with payload `{"type":"NaN"}`, not as a tag.
- **Fix:** Correct the docs. Go's `error.go:20-26` already gets this right.

**HOST-17: Version strings in the docs are stale** · Low · S
- **Where:** `docs/src/jvm.md:15,22` (5.3.0), `bindings/jvm/README.md:357` (5.6.0).
- **Fix:** Update them, and extend `bump-version.sh` or `check-stats.sh` to cover the docs.

**HOST-18: Test toolchains lag the declared ranges** · Low · S
- **Where:** `php/phpunit.xml:3` (schema 10.5, while composer requires `^13`), the `php-test` job in `ci.yml` (PHP 8.4 only), `.github/workflows/release-build-go.yml:59` (Go 1.25).
- **Fix:** Run `phpunit --migrate-configuration`, and add PHP 8.5 and Go `stable` legs.

**HOST-19: Four platform-naming schemes, each mapped by hand, and a missing platform is skipped silently** · Low · S
- **Where:** `scripts/stage-jvm-natives.sh:23-36`, `.github/workflows/release-build-php.yml:36-50`, `release-build-dotnet.yml:41-55`, Go `cgo_*_*.go`, `NativeLibrary.java:91-125`, `Native.php:263-277`.
- **Problem:** Each mapper skips a missing platform with an `if [ -d … ]` check instead of failing.
- **Fix:** Use one shared mapping script that fails when a platform is missing.

**HOST-20: The JVM extracts the native library to a fresh temp directory on every start** · Low · S
- **Where:** `jvm/.../NativeLibrary.java:132-147`.
- **Problem:** `deleteOnExit` fails on Windows because the DLL is locked, so temp directories pile up.
- **Fix:** Extract to a cache directory named by version and hash, and reuse it.

**HOST-21: PHP trace decoding loses JSON fidelity** · Low · S (5.x step)
- **Where:** `php/src/TracedSession.php:68-76`.
- **Problem:** `json_decode(assoc: true)` turns `{}` and `[]` into the same array, and large integers become floats.
- **Fix (5.x):** Add a raw-payload accessor.
- **v6 follow-up:** V6-HOST-09.

**HOST-22: Smaller test gaps** · Low · S
- **Where:** `go/*_test.go`, `jvm/src/test`, `dotnet/tests`.
- **Problem:**
  - Go never checks `NodeIDsJSON`.
  - JVM and .NET never test `Session.reset()`.
  - Only the JVM tests a Rule with a custom operator outliving its Engine.
  - No host tests close racing an evaluate.
- **Fix:** Add these cases to each runner, and keep a lifecycle checklist per host in `BINDINGS.md`.

## 1.5 React UI (`@goplasmatic/datalogic-ui`)

Paths are relative to `ui/`.

**UI-01: The published CJS build cannot start the WASM engine, and the editor hides the failure** · High · S–M
- **Where:** `vite.lib.config.ts:33`, `package.json` `exports["."].require`, `src/components/logic-editor/DataLogicEditor.tsx:607`.
- **Problem:** In the CJS output, `import.meta.url` is replaced with `{}`, so `new URL("data:…", "[object Object]")` throws. `vite.embed.config.ts:37-46` already fixes this for the embed build only. The editor reads only `ready` from the hook and ignores `error`.
- **Fix:** Polyfill `import.meta.url` in the CJS build or use an `initSync` path. Show load errors in a banner. Add a smoke test that `require`s `dist/index.cjs` and evaluates a rule.

**UI-02: Keyboard shortcuts are attached to `window` and take over the host page** · High · S
- **Where:** `src/components/logic-editor/debugger-controls/DebuggerControls.tsx:55-96`, `KeyboardHandler.tsx:76-123`, `EditorToolbar.tsx:62-84`.
- **Problem:** Space, the arrow keys, Home and End are page-wide while a trace is loaded, and Cmd-A, Z, Y, C, V, D, Backspace and Escape are page-wide in edit mode. Every instance on a page reacts to every key, and the docs mount many instances.
- **Fix:** Scope the listener to the editor root, or check focus containment. Let buttons and links keep their own keys. Replace the deprecated `navigator.platform`.

**UI-03: PR CI never runs the UI tests or `tsc -b`** · High · S
- **Where:** `.github/workflows/ci.yml:553-585`; the `package.json` scripts.
- **Problem:** The `ui-build` job runs only lint and `build:lib`. Guard tests such as `catalogue.test.ts` (the `operators.json` pin), `registry.test.ts` and `wasm-typings.test.ts` never run before merge. `npm test` doesn't sync the vendored WASM first.
- **Fix:** Add `npm ci && npm test && npx tsc -b` to the job, and add `"pretest": "npm run sync-wasm"`.

**UI-04: The bundle carries about 6.9 MB of base64 WASM per format, and runtime deps are both bundled and declared** · High · M (5.x step)
- **Where:** `vite.lib.config.ts:38-44`; `package.json` `files` and `dependencies`.
- **Problem:**
  - The `.wasm` is inlined twice, once each for ESM and CJS, so the tarball is about 14 MB.
  - `uuid`, `lucide-react` and `@dagrejs/dagre` are bundled into the output and also installed as dependencies.
  - `sourceMappingURL` points at `.js.map` files that are not shipped.
  - A host cannot pass in a WASM engine it already loads.
- **Fix (5.x):**
  - Emit the `.wasm` as a separate asset.
  - Externalize the three deps, or drop them from `dependencies`; `uuid` can become `crypto.randomUUID`.
  - Ship the maps or turn sourcemaps off.
  - Accept an optional engine or WASM URL.
- **v6 follow-up:** V6-UI-04.

**UI-05: Every edit rebuilds the node graph with fresh ids, remounting the canvas and losing the selection** · Medium · M
- **Where:** `src/utils/converters/*-converter.ts` (32 `uuidv4` call sites), `DataLogicEditor.tsx:413-440`, `src/hooks/useLogicEditor.ts:84-96`, `src/context/editor/EditorContext.tsx:107-116`.
- **Problem:** The selection and properties panel close 300 ms after each edit. Undo snapshots refer to ids that no longer exist.
- **Fix:** Derive node ids deterministically from the JSON Pointer, or skip re-conversion when the incoming `value` equals the last value emitted.

**UI-06: Two parallel JSON→node builders, static and trace, that already disagree** · Medium · L
- **Where:** `src/utils/converters/` (1,580 LOC) vs `src/utils/trace/node-creators/` (1,014 LOC).
- **Problem:** `?:` renders as `if` when data is present. Every display fix has to be made twice.
- **Fix:** Build nodes once from the rule JSON, keyed by pointer, and overlay trace steps through `pointers`. This can start now against the 5.8 pointers, ahead of proposal O12.

**UI-07: The UI renders nodes from engine expression strings** · Medium · M
- **Where:** `src/utils/trace/trace-to-nodes.ts:207-217`, `src/utils/trace/pointer-matching.ts:113-121`.
- **Problem:** The UI parses the expression strings and re-stringifies children to compare text. The engine's escaping bug (CORE-02) makes rules containing quotes fall back to placeholder nodes.
- **Fix:** Read sub-expressions from `originalValue` at `pointers[id]` instead of parsing strings. The AST-v2 rendering is V6-UI-05.

**UI-08: Engine facts are copied by hand although the engine exposes them** · Medium · M
- **Where:** `src/config/operators/*` (6,337 LOC), `src/components/logic-editor/debug-panel/engine-config.ts:35` (`PRESET_BASES`), `src/types/editor.ts:122-160`, `src/hooks/useLogicEditor.ts:54`.
- **Problem:** The UI calls neither `Engine.operators()` nor `Engine.check()`. The UI caps nesting at 100; the engine's default limit is 256. Compile diagnostics are never placed on nodes.
- **Fix:** Read arity and `scoped_arg` from `operators()`, and overlay `check()` diagnostics by pointer. Add a test comparing `PRESET_BASES` to the engine's presets.

**UI-09: The debugger re-renders every node on each step, and playback is quadratic** · Medium · M
- **Where:** `src/context/debugger/hooks.ts:23-64`, `src/hooks/useDebugClassName.ts:9`, `src/components/logic-editor/nodes/shared/NodeDebugBubble.tsx:20`, `src/context/debugger/DebuggerProvider.tsx:87-112`, `src/components/logic-editor/debugger-controls/StepTimeline.tsx:44,94`.
- **Problem:** Every node subscribes to the whole debugger context, so `memo()` doesn't help. The executed and error node sets are recomputed from step 0 on every step. The timeline renders every step without virtualization.
- **Fix:** Use per-node selectors, precompute cumulative sets once per trace, and virtualize the timeline.

**UI-10: Theming is inconsistent: 38 CSS tokens are used but never defined** · Medium · M
- **Where:** `src/components/logic-editor/properties-panel/properties-panel.css`, `panel-inputs/panel-inputs.css`, `src/styles/nodes.css`, `edges/edges.css`, `src/constants/colors.ts` vs `src/config/categories.ts`.
- **Problem:** The CSS uses 30 undefined `--dl-*` tokens plus several legacy names, and has 483 hex literals. The documented tokens therefore don't re-theme the panel, the inputs or the canvas. There are two copies of the category palette.
- **Fix:** Map every token onto the documented set in `theme.css`, remove the per-selector dark-mode overrides, and keep one palette.

**UI-11: The hand-copied React Flow CSS has drifted from upstream and leaks globally** · Medium · S (5.x step)
- **Where:** `src/styles/reactflow-base.css:1-6` (116 unscoped `.react-flow*` rules).
- **Problem:** The copy is missing `touch-action: none` and its pattern colours differ. It overrides the styles of a host app that also uses React Flow.
- **Fix (5.x):** Generate the file from the peer dependency at build time and scope it under `.logic-editor`.
- **v6 follow-up:** V6-UI-06.

**UI-12: Fixed global DOM ids in the properties panel** · Medium · S
- **Where:** `src/components/logic-editor/panel-inputs/FieldRenderer.tsx:34-170`.
- **Problem:** Ids like `panel-field-path` are fixed, and focus uses `document.getElementById`. With two editors on a page, labels bind to the wrong input and focus lands in the other editor.
- **Fix:** Prefix ids with `useId()` and focus through refs.

**UI-13: Test coverage gaps** · Medium · M
- **Where:** `tests/` and `src/**/__tests__`.
- **Problem:**
  - None of the 58 components is rendered in a test, and there is no jsdom setup.
  - The debugger reducer, history, `KeyboardHandler` and the JSON tokenizer are untested.
  - The registry check exists three times, with 87 hard-coded.
  - Tests call the deprecated free `wasm.evaluate`.
  - Nothing tests against the React 18 or `@xyflow/react` 12.0 peer floors.
- **Fix:** Add jsdom and testing-library smoke tests, unit-test the reducer and history, merge the registry tests, and move to `Engine#evalStr`.

**UI-14: README, docs and comments have drifted from the code** · Low · S
- **Where:** `README.md`, `docs/src/react-ui/props-api.md:381`, `src/config/__tests__/registry.test.ts:12`, the `vite.*.config.ts` comments, `.gitignore`, `.github/workflows/docs.yml:74,91`, `release-build-ui.yml`.
- **Problem:**
  - The README lists types that don't exist.
  - The WASM package is declared nowhere in `package.json`; CI injects it, as a dependency in one workflow and a devDependency in another.
  - Comments point to stale paths and contradict each other.
- **Fix:** Correct the docs and declare the WASM dependency in one place.

**UI-15: `DataLogicEditor.tsx` (643 LOC) duplicates itself and carries a dead prop** · Low · S
- **Where:** `src/components/logic-editor/DataLogicEditor.tsx:113-343, 478-576, 620`.
- **Problem:** The read-only and editable inner components are about 80% the same. `showDebugger` is always false, so the floating `DebuggerControls` variant never renders. The engine key is computed in two places.
- **Fix:** Use one inner canvas with an `editable` flag, and delete the floating variant.

**UI-16: Copied hooks, token tables and design files** · Low · S
- **Where:** `src/hooks/useSystemTheme.ts`, `src/hooks/useIsMobile.ts`, `src/index.css`, `src/embed.css`, `design-system.html` (152 KB, referenced nowhere), `src/components/logic-editor/index.ts`.
- **Problem:** The two hooks are identical to the copies under `components/logic-editor/hooks/`, the app re-declares all 100 `theme.css` tokens, and the second barrel file could be mistaken for the public entry.
- **Fix:** Deduplicate, move `design-system.html` to `docs/` or delete it, and trim the barrel.

**UI-17: Build config is duplicated, and some config files are never type-checked** · Low · S
- **Where:** the four `vite*.config.ts`, `tsconfig.node.json`, `eslint.config.js`, the `package.json` `sync-wasm` script.
- **Problem:**
  - The alias block is repeated six times.
  - The UMD `globals` setting is dead.
  - The lib, embed and vitest configs and `examples/` are never type-checked.
  - ESLint targets `ecmaVersion: 2020` against an ES2022 build.
  - `sync-wasm` uses `rm -rf` and `cp -R`, which fail on Windows, and has no version check.
- **Fix:** Share one `aliases.ts`, type-check every config, and turn `sync-wasm` into a Node script that checks the version.

**UI-18: Dead compatibility shims in the WASM evaluator** · Low · S
- **Where:** `src/hooks/useWasmEvaluator.ts:81-110, 403`.
- **Problem:** The engine is vendored and bundled, so an old WASM build can never be loaded. The fallbacks for one can never run.
- **Fix:** Remove them.

**UI-19: Per-node subtree copies and deep-cloned undo history** · Low · M
- **Where:** `src/types/editor.ts:15`, `src/context/editor/useHistoryState.ts:10-55`, `src/hooks/useWasmEvaluator.ts:384`.
- **Problem:** Each node stores its whole subtree, which is O(n × depth) memory. Every undo step deep-clones the full node array, up to 50 times. `getEngine` depends on object identity, so every parent render re-stringifies the inputs.
- **Fix:** Store pointers instead of subtrees, keep history as rule JSON, and key the engine on `engineKey`.

**UI-20: The component has no result output, so hosts evaluate twice** · Low · S
- **Where:** `src/App.tsx:87-120`, `src/embed/Widget.tsx`, `src/embed/Playground.tsx`.
- **Problem:** Each host runs its own `useWasmEvaluator` to get the result. `FlowDirection` is exported, but the component has no `direction` prop.
- **Fix:** Add optional `onResult` and controlled `direction` props, and pull the shared JSON-pane hook out of the three hosts.

**UI-21: Small type-hygiene items** · Low · S
- **Where:** `src/context/editor/hooks.ts:11-57`, `src/context/editor/EditorContext.tsx:106-116`, `src/hooks/useLogicEditor.ts:81`.
- **Problem:** 20 of the 31 `as unknown as` casts are unnecessary. Most of the 10 set-state-in-effect disables compute derived state. The prop-sync check compares only the node count and the first node's id.
- **Fix:** Drop the casts, compute derived state with `useMemo`, and use an explicit revision counter for prop sync.

## 1.6 Tests, CI, build, release and docs

Paths are relative to the repo root.

**INFRA-01: No `Cargo.lock` is committed anywhere, though binaries ship to six registries** · High · S
- **Where:** `.gitignore:8`; the five Cargo workspaces (root, `bindings/{c,node,python,wasm}`).
- **Problem:** Release artifacts are not reproducible, and a dependency published minutes before a tag goes straight into every registry. CI cache keys hash only the manifests. Dependabot cannot see the resolved versions.
- **Fix:** Commit the lockfiles, build releases with `--locked`, and add `Cargo.lock` to the cache keys.

**INFRA-02: No semver-compatibility gate** · High · S
- **Where:** `.github/workflows/ci.yml`, the release `validate` job.
- **Problem:** Seven minor releases shipped in 80 days over a large public surface. Nothing would catch an accidental breaking change.
- **Fix:** Run `cargo semver-checks check-release -p datalogic-rs` on PRs and in `validate`.

**INFRA-03: No advisory, license or source policy for Rust dependencies** · High · S
- **Where:** no `deny.toml`, and no audit step in any workflow.
- **Problem:** The bindings redistribute every transitive dependency inside their binaries. The one RUSTSEC fix so far (the smallvec floor) was done by hand.
- **Fix:** Add `deny.toml` (advisories, a license allowlist, crates.io as the only source, warn on duplicates), plus a `make deny` fan-out, run in CI and on a weekly cron.

**INFRA-04: The MSRV is documented as 1.85 in four places, but the real floor is 1.98** · Medium · S
- **Where:** `crates/datalogic-rs/Cargo.toml:16`, against `CONTRIBUTING.md:18`, `DEVELOPMENT.md:11`, the badge in `crates/datalogic-rs/README.md:5`, and `docs/src/getting-started/installation.md:126`.
- **Fix:** Update all four, and add an MSRV string check to `check-stats.sh`.

**INFRA-05: The MSRV equals current stable, forced by a sibling crate** · Medium · S–M
- **Where:** `crates/datalogic-rs/Cargo.toml:3-16`; `datavalue-rs` 0.3.1 (`rust-version = "1.98"`).
- **Problem:** The Cargo.toml comment says nothing in the crate needs 1.98 directly. Consumers pinned even one release behind stable cannot build 5.8.
- **Fix:** Lower `datavalue-rs`'s `rust-version` to what it actually needs and publish 0.3.2, then lower the floor here. Write down an MSRV policy in the CHANGELOG.

**INFRA-06: Docs-only PRs skip CI entirely, and mdBook builds only after merge** · Medium · S
- **Where:** `.github/workflows/ci.yml:3-17` (`paths-ignore`), `.github/workflows/docs.yml:3-11, 34` (`mdbook latest`).
- **Problem:** A README-only PR never runs the stats gate, so the next unrelated code PR fails instead. A broken mdBook surfaces only on main. A change to the core crate doesn't redeploy the playground.
- **Fix:** Add a `docs-check` job on PRs that runs `check-stats.sh`, `mdbook build` and an offline link check. Pin the mdBook version, and add `crates/datalogic-rs/src/**` to the `docs.yml` paths.

**INFRA-07: The conformance-count gate ties every new test case to a README edit, and it fails now** · Medium · S
- **Where:** `scripts/check-stats.sh:24-40`; `README.md:12,31,61`.
- **Problem:** On HEAD the script fails: the README still says 65 suites / 1,974 cases, but the suites now have 66 / 2,020.
- **Fix:** Fix the README on this branch. Generate the badge and prose numbers (`check-stats.sh --write`), or gate only at release and warn on PRs.

**INFRA-08: The release `validate` job is narrower than PR CI, and publishing uses long-lived tokens** · Medium · M
- **Where:** `.github/workflows/release.yml:85-275`.
- **Problem:** `validate` skips the feature combos, the MSRV check, minimal versions, `cargo doc -D warnings`, the no-default-features tests and the examples build. `workflow_dispatch` can release any tag. The crates.io and npm jobs have no `environment:` protection.
- **Fix:** Reuse `ci.yml` through `workflow_call`, or check that CI passed on the tagged SHA. Move crates.io and npm to OIDC Trusted Publishing, and protect those jobs with an environment.

**INFRA-09: The fuzz crate is never built or run in CI** · Medium · S/M
- **Where:** `Makefile` (`clippy-fuzz`), `crates/datalogic-rs/fuzz/Cargo.toml:27-38`, `crates/datalogic-rs/fuzz/.gitignore`.
- **Problem:**
  - The fuzz crate lints fine on stable, despite the Makefile comment; only `cargo fuzz run` needs nightly.
  - Its feature list omits `ext-object`, `tensor` and `budget`.
  - There is one target, and the corpus exists only locally.
- **Fix:**
  - Lint the crate on stable and switch it to `all-operators`.
  - Add a nightly cron run seeded from the suites.
  - Add `eval_tensor` and `session_reuse` targets.

**INFRA-10: No performance-regression gate and no committed baseline** · Medium · M
- **Where:** `tools/benchmark/src/bin/self.rs`, `.gitignore`, `scripts/check-stats.sh:30`.
- **Problem:** The geomean is hand-maintained in a script, and no workflow runs any benchmark. Proposal §6 assumes a bench gate exists; none does yet.
- **Fix:** Add an instruction-count bench (iai-callgrind or cachegrind) compared against a committed baseline. Warn above 2% and fail above 5%.

**INFRA-11: No WASM bundle-size gate** · Medium · S
- **Where:** `bindings/wasm/build.sh:92-97`, `.github/workflows/release-build-wasm.yml:27` (binaryen unpinned).
- **Problem:** Proposal §7 S3 accepts at most 5% growth, but nothing measures it.
- **Fix:** Commit the expected gzip size and fail CI above +3–5%. Pin binaryen.

**INFRA-12: Integration tests compile to nothing without features** · Medium · S
- **Where:** 38 of the 46 files in `crates/datalogic-rs/tests/` start with `#![cfg(feature…)]`; `DEVELOPMENT.md:98` recommends a default-feature run.
- **Problem:** A plain `cargo test` runs little more than the unit tests and reports green.
- **Fix:** Add a self dev-dependency with the test features, or use `[[test]] required-features` so a missing feature errors loudly. Delete the misleading line in `DEVELOPMENT.md`.

**INFRA-13: CI installs can drift from lockfiles, and toolchains float** · Medium · S
- **Where:** `ci.yml:466,527,579`, `docs.yml:94`, `release-build-ui.yml:74`, `release-build-node.yml:70,135,139`, `release-build-python.yml:130`.
- **Problem:** `npm ci || npm install` turns a lockfile mismatch into a fresh resolve, and the UI uses plain `npm install`. pip installs are unpinned, and release Rust builds use floating `stable`.
- **Fix:** Use strict `npm ci`, add a `requirements-ci.txt`, and pin the toolchain with `rust-toolchain.toml`.

**INFRA-14: CI cache keys miss the sources and the lockfile** · Medium · S
- **Where:** in `ci.yml`, the `wasm-build`, `python-test`, `feature-matrix` and `feature-combos` jobs; `.github/actions/rust-lint/action.yml`.
- **Problem:** The keys hash only manifests, so the first cache saved after a manifest change is restored forever and `target/` grows without bound.
- **Fix:** Use `Swatinem/rust-cache`, or add the lockfile and sources to every key.

**INFRA-15: Suite-loading logic is duplicated, and the bench loader ignores suite flags** · Medium · M
- **Where:** `crates/datalogic-rs/tests/test_jsonlogic.rs:29-160`, `crates/datalogic-rs/tests/oracle_test.rs:56-125`, `tools/benchmark/src/lib.rs:63-96`.
- **Problem:** Three Rust copies read `index.json` and pick engines. The bench copy drops `templating` and `template_key_escape`, so about 59 cases are timed on the wrong engine. It also silently drops suites that fail to parse.
- **Fix:** Share one `tests/common/suite.rs`, or a `publish = false` crate. Make the bench honor the suite flags and fail on parse errors.

**INFRA-16: The crates.io CHANGELOG stub lists shipped features as unreleased** · Low–Medium · S
- **Where:** `crates/datalogic-rs/CHANGELOG.md:15-30`.
- **Problem:** Its `[Unreleased]` section lists features that shipped in 5.4.0, so every crates.io release since then has packaged a misleading changelog.
- **Fix:** Make the stub link-only, or diff it against the root CHANGELOG in `validate`.

**INFRA-17: Rust code in the docs is never compiled** · Low · M
- **Where:** 125 Rust blocks under `docs/src/`, and 16 in `crates/datalogic-rs/README.md`.
- **Problem:** One README block fails today (`:388`, missing import), and 74 mdBook fragments don't compile standalone. The next API rename will leave stale docs with nothing to catch it.
- **Fix:** Add `#[cfg(doctest)] #[doc = include_str!("../README.md")]`. Give mdBook snippets hidden imports and run them through a doctest shim in CI, marking third-party snippets `ignore`.

**INFRA-18: One giant `#[test]` for the whole conformance run, with weak skip accounting** · Low · M
- **Where:** `crates/datalogic-rs/tests/test_jsonlogic.rs:283-297, 425-497`.
- **Problem:** 2,020 cases × 7 modes run inside one test. Nothing asserts that no case was skipped under `--all-features`. A missing suite file only prints a warning.
- **Fix:** Generate one test per suite with `libtest-mimic`, assert zero skips, and panic on a missing file.

**INFRA-19: `tests/README.md` is stale about the harness** · Low · S
- **Where:** `crates/datalogic-rs/tests/README.md:75-78`.
- **Problem:** It doesn't mention the folding key or the 7 evaluation modes every case must agree across.
- **Fix:** Document both.

**INFRA-20: 46 separate integration-test binaries** · Low · M
- **Where:** `crates/datalogic-rs/tests/*.rs`.
- **Problem:** Each file links its own binary against the full crate, so link time dominates incremental test runs. File names are inconsistent.
- **Fix:** Consolidate into `tests/it/main.rs`.

**INFRA-21: `ARCHITECTURE.md` is out of date** · Low · S
- **Where:** `ARCHITECTURE.md:93-169`.
- **Problem:** No `all-operators` row; the bench features are wrong; the `projection` bench binary is missing; the budget overhead is given as 3.6%, but the proposal measures 2.8%.
- **Fix:** Correct them.

**INFRA-22: `bindings/BINDINGS.md` contradicts current conventions** · Low · S
- **Where:** `bindings/BINDINGS.md:41-47`.
- **Problem:** It says there is "no umbrella feature" and that bindings pin "5.0". It describes release jobs inline in `release.yml`, though they are now separate `release-build-*.yml` files, and mentions `jni` for a binding that uses FFM.
- **Fix:** Rewrite the convention table. The pin bump is BIND-20.

**INFRA-23: The `all-operators` feature is undocumented for users** · Low · S
- **Where:** `crates/datalogic-rs/README.md:460-475`, `docs/src/getting-started/installation.md`.
- **Fix:** Add a row to both feature tables.

**INFRA-24: `DEVELOPMENT.md`'s release section is inaccurate and carries an old ops log** · Low · S
- **Where:** `DEVELOPMENT.md:334-387`.
- **Problem:** It gets the order of the release flow wrong, and it carries dated registry status notes.
- **Fix:** Correct the flow and move the ops notes to a playbook or issue.

**INFRA-25: No `[workspace.dependencies]` or `[workspace.lints]`** · Low · S
- **Where:** root `Cargo.toml`; `serde_json` and `bumpalo` requirements differ across members; lints live only in `crates/datalogic-rs/src/lib.rs:1-3`.
- **Problem:** `datalogic-bind` and the bindings get none of the core lint policy, and only core declares a `rust-version`.
- **Fix:** Hoist the shared dependencies, lints and `rust-version` into the workspace.

**INFRA-26: Heavy dev-dependencies for a few tests** · Low · S
- **Where:** `crates/datalogic-rs/Cargo.toml` (`tokio` with `full`, `futures`).
- **Problem:** Two test files use them.
- **Fix:** Use tokio `rt-multi-thread` + `macros`, and `JoinSet` instead of `futures::join_all`.

**INFRA-27: The core crate is tested only on Linux x86_64** · Low · S
- **Where:** every non-release job in `ci.yml`.
- **Problem:** No macOS, Windows, 32-bit or big-endian runs, though `tensor` reinterprets byte buffers.
- **Fix:** Add weekly macOS and Windows legs, plus `cross` runs on i686 and s390x for the tensor suites.

**INFRA-28: Feature-matrix jobs don't deny warnings** · Low · S
- **Where:** the `feature-matrix` job in `ci.yml`.
- **Problem:** Each single-feature build is clean today, but nothing enforces it.
- **Fix:** Set `RUSTFLAGS: -D warnings` on the job.

**INFRA-29: Core examples are built but never run** · Low · S
- **Where:** the "Build examples" step in `ci.yml`.
- **Fix:** Run each of the 11 examples.

**INFRA-30: The benchmark labels its reports with the bench crate's version** · Low · S
- **Where:** `tools/benchmark/src/bin/self.rs:383`.
- **Problem:** Reports say `self-v0.1.0`, so they can't be tied to the engine release they measured.
- **Fix:** Use `datalogic_rs::VERSION`.

**INFRA-31: The Makefile has no repo-wide `test`, `doc`, `deny` or `stats` targets** · Low · S
- **Where:** `Makefile`.
- **Problem:** Testing the excluded binding workspaces takes manual `--manifest-path` runs, though CLAUDE.md says to use `make` for repo-wide work.
- **Fix:** Add the four fan-out targets.

**INFRA-32: CI cancels in-progress runs on `main`** · Low · S
- **Where:** `.github/workflows/ci.yml:19-21`.
- **Problem:** Back-to-back merges leave earlier main commits with no completed CI result.
- **Fix:** `cancel-in-progress: ${{ github.event_name == 'pull_request' }}`.

**INFRA-33: Two major versions of `syn` in the core build graph** · Low · S
- **Where:** `syn` 2 arrives through `half` → `zerocopy-derive`, and `syn` 3 through `serde_derive`.
- **Problem:** Extra build time for `tensor-half` users. It is build-time only.
- **Fix:** Track it with cargo-deny `bans` (INFRA-03) and update upstream.

## 1.7 Suggested 5.x sequencing

1. **5.8.1, safety patch:**
   - Crashes and UB: CORE-01, OPS-01, OPS-02, OPS-03, BIND-01, BIND-02, HOST-01, HOST-02.
   - JSON and pointer output: CORE-02, CORE-03, CORE-05.
   - UI: UI-01, UI-02.
   - Repo: INFRA-07, then INFRA-04.
2. **Supply chain and gates, before the next minor:** INFRA-01, -02, -03, -08, -06, -12, -13; UI-03; HOST-03.
3. **5.9, consistency:**
   - CORE-04, CORE-06, CORE-07, CORE-10, CORE-18.
   - OPS-04, OPS-05.
   - BIND-03 through BIND-09.
   - HOST-04 through HOST-10.
   - UI-04, UI-05, UI-08.
   - INFRA-09 through INFRA-11.
4. **Ongoing:**
   - Internal refactors that make v6 a smaller diff: CORE-08, -12, -14, OPS-07 through OPS-10, UI-06, UI-07.
   - Performance: CORE-09, -15, -16, -17, -21; OPS-11.
   - Everything else rated Low.

## 1.8 5.x preparatory steps for v6 items

These steps are non-breaking and smooth the v6 transition. Each points to its v6 item in Part 2.

| 5.x step | Enables |
|---|---|
| Derive `Copy`, `Eq`, `Hash` on `NanHandling` and `DivisionByZeroHandling` | V6-CORE-03 |
| Mark `OwnedInput::into_owned_input` deprecated | V6-CORE-06 |
| Document the `eval` vs `eval_str` argument asymmetry in the "Choosing an evaluate method" table | V6-CORE-07 |
| Add the new error kinds (CORE-07) | V6-CORE-08 |
| Document that DateTime and Duration values serialize differently per entry point | V6-CORE-02 |
| Opt-in config flags for each V6-OPS fix | V6-OPS-* |
| Add a `compile_checked` lint for a non-boolean `sort` direction | V6-OPS-13 |
| Error additive aliases (BIND-05), batch refactor (BIND-04), engine-mismatch warning (BIND-06), scenario pins (BIND-23) | V6-BIND-01 to -04 |
| Deprecation warnings on aliases (BIND-11) | V6-BIND-05 |
| .NET multi-target (HOST-09), JVM `Automatic-Module-Name` (HOST-12), PHP aliases (HOST-14) | V6-HOST-* |
| Export the missing UI type names, mark the node-model types `@deprecated`, and stop setting the unread `ui` hints | V6-UI-01, V6-UI-02 |
| Update proposal O8's cfg baseline: 369 sites, not 318 | Proposal accuracy |

---

# Part 2: Needs a breaking change (target v6)

These items are in addition to the restructuring in `proposal-v6.md`, which covers O1–O12: one compile path, strict by default, engine-bound rules, the extension trait, a public AST, the arena kept out of user code, split errors, instrumentation without cargo features, costs from the table, thin bindings, and the v2 wire schemas. Where an item below falls inside one of those objectives, it is listed so the v6 work can absorb it.

## 2.1 Core API and wire shapes

Paths are relative to `crates/datalogic-rs/src/`.

**V6-CORE-01: A rule refuses to evaluate on an engine other than the one that compiled it** · High · L
- **Where:** `node/logic.rs`.
- **Problem:** This absorbs CORE-04: folded config must not cross engines. It is proposal O3.

**V6-CORE-02: One serialization for host-supplied DateTime and Duration values** · Medium · S
- **Where:** `arena/value/conversion.rs:27-37`, `serde_bridge.rs:26-31` vs `result_output.rs:44-48`.
- **Problem:** `eval_str` returns a plain ISO string, while `eval_as::<serde_json::Value>` returns `{"datetime":…}`. Durations are wrapped under the key `"timestamp"`, which is wrong.
- **Fix:** Pick one representation and route both outputs through it.

**V6-CORE-03: Make the public types extensible** · Medium · S
- **Where:** `Metered<T>` (`engine/mod.rs:64`); `ExpressionNode`, `ExecutionStep` and `TracedRun` (`trace.rs:24,166,279`); `NanHandling`, `DivisionByZeroHandling` and `TruthyEvaluator` (`config.rs:172-201`).
- **Problem:** Each of these is an all-public struct or an exhaustive enum, so any later addition is breaking.
- **Fix:** Add `#[non_exhaustive]` to all of them. Proposal O12 needs this.

**V6-CORE-04: Structured errors in trace steps** · Low–Medium · S
- **Where:** `trace.rs:177` (`error: Option<String>`), `arena/context/mod.rs:319-321`.
- **Fix:** Use the shared error wire form from proposal §3.7. This unblocks the frame table in CORE-09.

**V6-CORE-05: The custom truthiness closure takes a borrowed value** · Medium · S
- **Where:** `config.rs:232`, `arena/value/strings.rs:140`.
- **Problem:** `TruthyEvaluator::Custom` deep-copies every value it tests, and it also turns off CSE and strength reduction.
- **Fix:** Change the closure signature to `Fn(&DataValue<'_>) -> bool`.

**V6-CORE-06: Seal the input and output traits** · Low–Medium · S
- **Where:** `eval_input.rs:66-90, 222-225`, `result_output.rs:27-32`, `truthy_input.rs:23-24`.
- **Problem:** `into_arena_value`, the `#[doc(hidden)]` `into_arena_for`, `into_owned_input`, `from_arena` and `truthy_with` are all public trait methods.
- **Fix:** Move them onto private supertraits. Part of proposal O6.

**V6-CORE-07: Consistent first argument for same-named methods; `TracedSession` naming** · Low · S
- **Where:** `trace.rs:303-423`, `session.rs:256` vs `engine/mod.rs:1127`.
- **Problem:** `eval_str` takes a compiled `Logic` on `Session` but rule source on `Engine`. `TracedSession` allocates a fresh arena per call, so it isn't really a session. There is no checked template compile.
- **Fix:** Part of proposal §3.3.

**V6-CORE-08: Switch depth errors to the new kinds; rename the recursion setting** · Medium · S
- **Where:** `node/compile_ctx.rs:143-149`, `engine/mod.rs:1247-1257`, `config.rs`.
- **Fix:** Report the existing depth failures as `DepthExceeded` (added in CORE-07), and rename `max_recursion_depth` to `max_reentry_depth`.

## 2.2 Operator semantics

Paths are relative to `crates/datalogic-rs/src/operators/`. Each item changes the result of an input that succeeds today, so the default flips in v6. Every one can ship earlier as a 5.x opt-in config flag. Pin the current behavior in the suites now (OPS-14).

**✅ V6-OPS-01: Any object with a `datetime` or `timestamp` key is treated as a temporal value** · High · S
- **Where:** `datetime/mod.rs:91-138`, `comparison/mod.rs:219-237`, `inspect.rs:52-61`.
- **Problem:** Two records `{"datetime":"…","user":"alice"}` and `{…,"user":"bob"}` compare equal under `===` and `==`. `in` matches them, `distinct` drops one, and `type` reports `{"timestamp":123,"id":1}` as `"duration"`.
- **Fix:** Treat only single-key objects whose value parses as temporal.
- **Recommendation:** Consider shipping this in 5.x as a correctness fix with a prominent changelog note.

**V6-OPS-02: The `datetime` cargo feature changes how plain strings compare** · High · M
- **Where:** `comparison/mod.rs:55-68, 216-238, 427-458`, `array/sort.rs:223-265`.
- **Problem:**
  - With the feature on, `{"===":["1d","24h"]}` is `true`, and so is `===` on two ISO strings that differ only in fractional digits.
  - `distinct` and `switch` collapse such values.
  - `sort` disagrees with `<` on datetimes with offsets.
  - Nested values skip the check.
  - Every binding enables the feature, so the baseline operators behave differently per build.
- **Fix:** Make temporal coercion an explicit runtime config, or limit it to the DateTime and Duration variants. At minimum, keep it out of `===` and make `sort` use the same comparator as `<`.

**V6-OPS-03: Integers above 2^53 lose precision** · Medium · S–M
- **Where:** `comparison/mod.rs:259,416`, `comparison/loose.rs:47-55`, `arithmetic/div_mod.rs:77-80`, `arithmetic/helpers.rs:237-266`, `array/sort.rs:241-252`.
- **Problem:** `===` says 2^53+1 equals 2^53 at the top level but not when nested. `<`, `/` and `%` are wrong for such values. Variadic `+` gives a different answer from binary `+`.
- **Fix:** Compare with `NumberValue`'s exact `PartialEq`/`PartialOrd`, use i64 `checked_div`/`checked_rem` for integer operands, and use the same coercion in the variadic fold.

**V6-OPS-04: `NanHandling` is honored inconsistently** · Medium · M
- **Where:** `arithmetic/helpers.rs:36`, `arithmetic/basic.rs:205-302`, `div_mod.rs`, `min_max.rs:86,110`, `extract.rs:131`, `comparison/loose.rs:60`.
- **Problem:**
  - `CoerceToZero` behaves like `IgnoreValue`: `{"*":[2,"x",3]}` gives 6. `tests/config_test.rs` pins this.
  - `-`, `/`, `%`, `min` and `max` ignore the config.
  - The strings `"NaN"` and `"inf"` parse and then serialize as `null`.
- **Fix:** Add a real zero substitution, apply the config uniformly, reject non-finite parses, and rewrite the test.

**V6-OPS-05: Division and modulo by zero depend on int vs float and on arity** · Medium · S
- **Where:** `arithmetic/div_mod.rs:80-257`, `datetime/arith.rs:128`.
- **Problem:** `7/0` errors, but `"7"/0` saturates. `0.0/0` errors because the literal collapses to an integer. `%` by zero gives `f64::MAX`.
- **Fix:** One policy based on the operand values, and a separate rule for modulo by zero.

**V6-OPS-06: Each iterator handles a scalar or object source differently** · Medium · M
- **Where:** `array/{filter,map,reduce,quantifiers,group_by,distinct,sort}.rs`, `arithmetic/min_max.rs:62-70`.
- **Problem:** With a scalar source, `filter` errors, `map` wraps it, `reduce` returns the initial value, `all` is false and `max` returns the scalar. With an object source, `reduce` drops the key frame that `filter` and `map` push.
- **Fix:** One source policy per family in the `each`/`iter` adapter, and keyed frames for `reduce`.

**V6-OPS-07: Loose `==` uses its own coercion table** · Medium · M
- **Where:** `comparison/loose.rs:28-162`.
- **Problem:** `0==""` raises NaN while `0<=""` is `true`. `null==0` is `true` even with `null_to_zero=false`. Equal objects compare unequal under `==` without an error, while unequal arrays error.
- **Fix:** Route coercion through the configured coercion, and give objects a defined rule.

**V6-OPS-08: Incomparable ordered pairs; `!=` arity** · Low–Medium · S
- **Where:** `comparison/mod.rs:476-484`, `table.rs:485-488`.
- **Problem:** `"a"<1` raises NaN, while `true<"a"` returns `false`. `!=` silently ignores extra arguments while `==` is variadic, so they aren't negations of each other.
- **Fix:** One rule for incomparable pairs. Make `!=` variadic, or reject extra arguments.

**V6-OPS-09: The `type` operator's heuristic disagrees with the parsers** · Medium · S
- **Where:** `inspect.rs:52-104`.
- **Problem:** `"password1"` is reported as `"duration"`. A valid offset datetime is reported as `"string"`.
- **Fix:** Classify with `DataDateTime::parse` and `DataDuration::parse`.

**V6-OPS-10: Unify numeric coercion semantics** · Low–Medium · M
- **Where:** `arithmetic/min_max.rs:85`, `extract.rs:117,131`, `datetime/arith.rs`.
- **Problem:** `{"max":["1",2]}` errors. `substr` ignores a string or fractional start. Datetime arithmetic bypasses `bool_to_number`.
- **Fix:** Build on the coercion module from OPS-09.

**V6-OPS-11: `try` catch object keyed on `ErrorCode`** · Medium · M
- **Where:** `error_handling.rs:222-257`.
- **Problem:** The catch arm receives the message text, which makes rewording a message a breaking change.
- **Fix:** Pass `{code, message}`. Extends proposal O7.

**V6-OPS-12: `ceil` and `floor` saturate; `abs("NaN")` returns null** · Low · S
- **Where:** `arithmetic/unary_math.rs:50-55`, `extract.rs:131`.
- **Fix:** Return a float when out of i64 range, and reject non-finite input.

**V6-OPS-13: `sort` direction** · Low · S
- **Where:** `array/sort.rs:81-95`.
- **Problem:** `"desc"` silently sorts ascending.
- **Fix:** Error on a non-boolean direction, or accept `"asc"` and `"desc"`.

**V6-OPS-14: `upper`/`lower` lose context-sensitive case rules** · Low · S
- **Where:** `string.rs:228-249`.
- **Problem:** Greek final sigma is lost: `"ΟΔΟΣ"` lowercases to `"οδοσ"` instead of `"οδος"`.
- **Fix:** Take an ASCII fast path, and otherwise use `str::to_lowercase`. This is arguably a 5.x bug fix.

**V6-OPS-15: The `z` token in `parse_date`/`format_date` works only as the whole format** · Low · S
- **Where:** `datetime/mod.rs:354-366`.
- **Fix:** Treat `z` as a normal token.

**V6-OPS-16: `merge` drops nulls inside its array arguments** · Low · S
- **Where:** `array/merge.rs:42-47`, pinned by `tests/suites/array/merge.json:57-74`.
- **Problem:** JSONLogic's concat semantics keep the null.
- **Fix:** Decide the v6 semantics and document the divergence.

## 2.3 Bindings (Rust tier and C ABI)

Paths are relative to `bindings/`.

**V6-BIND-01: One error wire shape in every binding** · Medium · M
- **Where:** the error-conversion code in each binding.
- **Problem:** The four bindings use different field names, casing and error class names (BIND-05).
- **Fix:** Use the same field names and casing everywhere, generated by `datalogic-bind`. This is proposal §3.7.

**V6-BIND-02: One batch item shape** · Medium · M
- **Where:** the batch code in each binding (BIND-04).
- **Fix:** Use `{ok, value} | {ok:false, error}` everywhere, Rust tier and hosts alike. This is proposal §3.9.

**V6-BIND-03: One spelling for invalid-argument errors** · Medium · S
- **Where:** see BIND-23.
- **Fix:** Use `InvalidArguments` everywhere (or a documented set of binding-level codes), and stop WASM reporting option errors as `ParseError`.

**V6-BIND-04: Refuse a rule from another engine in every binding** · Medium · S
- **Where:** the session code in Node, Python and WASM.
- **Fix:** Follows BIND-06 and proposal O3.

**V6-BIND-05: Remove deprecated names** · Low · S
- **Where:** `apply`, `evaluateNumber`, the legacy WASM `CompiledRule`, and the free `evaluate` / `evaluateWithTrace`.
- **Fix:** Remove them, as proposal §3.9 plans.

**V6-BIND-06: C ABI v3** · Medium · M
- **Where:** `c/src/error.rs`, `c/src/builder.rs`, `c/include/datalogic.h`.
- **Fix:**
  - Add statuses for `Compile`, `Config` and `Budget` (BIND-22).
  - Give every builder setter a status return (BIND-16).
  - Remove the non-mode `traced_session_evaluate` and other superseded entry points (HOST-15).
  - Make the `_ex` operator registration the baseline (BIND-08).

**V6-BIND-07: One object key-ordering policy** · Low · S
- **Where:** `node/src/engine.rs:684`, `node/src/session.rs:80`, `python/src/conv.rs:169-184`.
- **Problem:** Node's `evaluate()` and Python sort keys; `evaluateStr` and WASM keep the engine's order.

**V6-BIND-08: One result type per concept in the JS packages** · Medium · M
- **Where:** `node/src/engine.rs:74-86`, `wasm/src/lib.rs:220-229, 638-652, 779`.
- **Problem:** Metered results, `check`, `operators`, `facts` and `Rule.evaluate` return native objects in Node but JSON strings in WASM.

**V6-BIND-09: Separate methods for JSON text and host values** · Low · M
- **Where:** `node/src/engine.rs:403-683`, `python/src/engine.rs:371-601`.
- **Problem:** A host string is always parsed as JSON, so `rule.evaluate("abc")` raises `ParseError`.

## 2.4 Host bindings

Paths are relative to `bindings/`.

**V6-HOST-01: One exception hierarchy across the four hosts** · Medium · M
- **Where:** `jvm/.../DatalogicException.java:107`, `dotnet/.../DatalogicException.cs:93`, `php/src/Exception/DatalogicException.php:44`, `go/error.go:12`.
- **Problem:** The same failure maps to different exception types in each host (HOST-10).
- **Fix:** Add a configuration exception, and use consistent invalid-argument, internal and closed-handle types.

**V6-HOST-02: One batch item type across the four hosts** · Medium · M
- **Where:** the batch decoders (HOST-10).
- **Fix:** Matches V6-BIND-02.

**V6-HOST-03: PHP raw-handle constructors become private** · Low · S
- **Where:** `php/src/Rule.php`, `php/src/Session.php`, `php/src/TracedSession.php`, `php/src/Engine.php` (`fromHandle`, `handle()`).
- **Fix:** Make them private or factory-only (HOST-11).

**V6-HOST-04: JVM `module-info.java` that hides `internal`** · Low–Medium · S
- **Where:** `jvm/src/main/java/`.
- **Fix:** Export only `com.goplasmatic.datalogic` (HOST-12).

**V6-HOST-05: Remove Jackson 2 from the JVM public API** · Low · M
- **Where:** `jvm/.../TracedRun.java`, `EvalResult.java`, `TracedSession.java`.
- **Problem:** These return Jackson 2 `JsonNode`, which pins consumers to Jackson 2. Moving to Jackson 3 would itself be breaking.
- **Fix:** Return strings or a tree type owned by the binding.

**V6-HOST-06: Raise the JVM floor to Java 25 LTS** · Low · S
- **Where:** `jvm/pom.xml:38-41`.
- **Fix:** Breaking for JVM consumers on Java 22–24.

**V6-HOST-07: Drop .NET net8.0** · Low · S
- **Where:** `dotnet/src/Datalogic/Datalogic.csproj`.
- **Fix:** Breaking for .NET consumers still on net8.

**V6-HOST-08: Remove the naming outliers** · Low · S
- **Where:** see HOST-14.
- **Problem:** Go `SetConfigJSON` returns an error, PHP takes an `int` mode, and `Check` requires a mode in some hosts.
- **Fix:** Remove them once the 5.x aliases exist.

**V6-HOST-09: PHP trace returns objects with JSON fidelity** · Low · S
- **Where:** `php/src/TracedSession.php:68-76`.
- **Fix:** Follows HOST-21.

**V6-HOST-10: JVM native resource names use `<os>-<arch>`** · Low · S
- **Where:** `jvm/.../NativeLibrary.java:91-125`, `scripts/stage-jvm-natives.sh`.
- **Problem:** The JNA-style names linger from before the move to FFM.

## 2.5 React UI

Paths are relative to `ui/`.

**V6-UI-01: Narrow the public type surface** · Medium · M
- **Where:** `src/lib.ts:9-44`, `src/types/editor.ts:10-108`.
- **Problem:** The entry exports the internal render model (`LogicNode`, `LogicNodeData`, `CellData`, `VariableNodeData`, …, plus `jsonLogicToNodes` and `applyTreeLayout`), so any refactor of node data is breaking. `OPERATORS` is the live mutable registry.
- **Fix:** Remove or replace these exports, and export `OPERATORS` frozen and `Readonly`.

**V6-UI-02: Remove the unread registry UI hints** · Low · S
- **Where:** `src/config/operators.types.ts:133-138`, `src/config/operators/array-iteration.ts`.
- **Problem:** `iteratorContext`, `inlineEditable`, `collapsible`, `scopeJump`, `metadata` and `showArgLabels` are never read.
- **Fix:** Remove them, or derive `iteratorContext` from `scoped_arg`. Extends proposal §3.10.

**V6-UI-03: Possibly drop the `require` (CJS) export** · Low · S
- **Where:** `package.json` `exports["."].require`.
- **Fix:** Only needed if UI-01's fix proves too costly.

**V6-UI-04: Make `@goplasmatic/datalogic-wasm` a required peer dependency** · Medium · S
- **Where:** `package.json`.
- **Problem:** A host that already loads the WASM package carries a second engine inside the UI bundle (UI-04).
- **Fix:** Make it a required peer instead of bundling it.

**V6-UI-05: Render from AST v2 instead of expression strings** · Medium · L
- **Where:** `src/utils/trace/`.
- **Fix:** Proposal O12.

**V6-UI-06: Require hosts to import React Flow's own base CSS** · Low · S
- **Where:** `src/styles/reactflow-base.css`.
- **Fix:** Have hosts import `@xyflow/react/dist/base.css` instead of shipping a copy.

---

## Totals

| Area | 5.x items | v6 items |
|---|---|---|
| Core engine and compile | 25 | 8 |
| Operators | 14 | 16 |
| Rust bindings, C ABI, `datalogic-bind` | 23 | 9 |
| Host bindings (Go, JVM, .NET, PHP) | 22 | 10 |
| React UI | 21 | 6 |
| Tests, CI, build, docs | 33 | 0 |
| **Total** | **138** | **49** |

By severity, 5.x items:
- **High:** 17
- **Medium:** 52
- **Low–Medium:** 7
- **Low:** 62

Items with both a 5.x step and a v6 step appear in both columns.
