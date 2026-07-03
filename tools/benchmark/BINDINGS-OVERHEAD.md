# Binding overhead analysis

Status: **implemented and re-measured**. This document began as the
exploratory analysis quantifying the per-call overhead each language
binding adds on top of the Rust core; that analysis (sections 1-7,
kept as the record) drove the C ABI v2 arc that shipped in 5.0.1. The
"Outcome" section below is the scoreboard; section 2 and the appendix
carry the current (post-v2) capture, with the pre-v2 numbers preserved
as the historical baseline at the end of the appendix.

> **Decision (2026-07-03):** a direct C ABI v2 build for the v5.0.1
> release was approved and implemented the same day, superseding the
> staged sequencing in section 6 (the additive B-tier variants were
> skipped; the C ABI is in-tree only, so v2 replaced v1 outright). The
> shipped contract lives in
> [`bindings/c/README.md`](../../bindings/c/README.md) and the generated
> `include/datalogic.h`; the v1 → v2 migration table is in
> [`MIGRATION.md`](../../MIGRATION.md). Every number in this file is
> reproducible in-tree: [`boundary/README.md`](./boundary/README.md).

> **Captured:** 2026-07-03 • Apple M2 Pro (arm64), macOS 26.5 • Rust 1.96
> release builds • Node v24 • CPython 3.13 (abi3 wheel) • Go 1.25 toolchain
> • OpenJDK 26 + JNA 5.17 • .NET SDK 9 (net9.0 console over net8.0 binding)
> • PHP 8.4 with FFI • median of 5 samples, each sized to ~250 ms, after
> warmup. Same three workloads in every runtime; every runtime produced
> byte-identical results before timing started.
>
> **Re-captured (ABI v2 + mirror wave):** 2026-07-03, same host and
> discipline, after the v2 rollthrough and the Node/Python/WASM mirror
> wave — JVM now OpenJDK 26 via FFM (JNA deleted), PHP now 8.5.7, and
> all nine runtimes expose the data-handle/typed/batch tiers. Current
> tables in section 2 and the appendix come from one
> `tools/benchmark/boundary/run.sh all` capture.

## Outcome: what shipped in 5.0.1

Everything from section 1 down reads as the original analysis; this
section is the scoreboard. **Shipped:** A1 (Python LTO), A3 (Node
string fast-path), A4 (PHP preload header), A6 (docs honesty pass), and
the C ABI v2 — C1 collapsing B1 data handles, B2 batch, B3 typed scalar
results, B4 `(ptr,len)`, B5 status-code errors, B7 arena pooling, and
B8 buffer-reuse results into one surface — with the JVM binding
rewritten FFM-only (B10 + C2 in one step: JDK 22 floor, JNA deleted).
A2 was superseded by the FFM rewrite.

The same release then closed the deferred list. The **Node/Python/WASM
mirrors** expose data handles, typed results, and batch natively in
each crate (they bind the core directly, so no C ABI detour). **B9**
shipped as Node's `evaluateStrAsync` on the libuv pool. **A5** was
measured and shipped as the opt-in `WASM_PROFILE=speed` build
(1.13-1.85x faster across tiers, +8.1% raw size but 1.4% *smaller*
gzipped; the published default stays size-optimized). **B6** ended with
a split verdict, exactly as section 3's analysis anticipated: accepted
for Python (a direct dict↔arena walk, 2.5-3.4x at every size, with
pythonize kept as the exotic-shape fallback and pre-change semantics
pinned by a 549-case equivalence corpus) and attempted-then-reverted
for Node (built to full parity, measured 23-31% faster than the serde
bridge, but still behind V8's `JSON.stringify` + one string crossing on
all three workloads — the string path remains Node's fast lane, and the
equivalence test stays in-tree as the gate for future attempts). C3
stays rejected.

Re-measured on the `simple` workload, ns per evaluation:

| Binding | v1 string | v2 string | v2 data handle | v2 batch (100 rules × 1 payload, per eval) |
|---------|----------:|----------:|---------------:|-------------------------------------------:|
| C ABI   |     186.0 |     122.8 |           39.1 |                                        39.3 |
| .NET    |     216.7 |     162.3 |           49.7 |                                        62.0 |
| Go      |     358.5 |     205.5 |          117.6 |                                        61.1 |
| JVM     |   3,268.7 |     270.0 |          133.3 |                                        70.9 |
| PHP     |     559.3 |     689.8 |          599.3 |                                       165.5 |

Reading it:

- **Every gate from the plan was met**: C session ≤140 (122.8); C
  data-handle ≤110 / ≤450 / ≤2,200 across the three workloads (39.1 /
  158.9 / 1,000.5 — a 12.2x reduction over the string path at 8 KB);
  .NET ≤180 (162.3); Go ≤220 (205.5, with the per-call
  `LockOSThread` pair and `C.CString` copies gone); JVM ≤600 (270.0).
- **The JVM outlier is gone**: 12x faster on the string path, and the
  plan's "100 × `simple` ≈ 327 µs → ~16 µs" batch estimate landed at
  ~7 µs (~46x).
- **The C session path now sits below the "string-contract floor"**
  (122.8 vs 132.8) because the floor includes a result-`String`
  allocation that v2's borrowed-result contract skips entirely.
- **`ParsedData` passthrough is free**: the core `parseddata-eval` tier
  equals `eval-preparsed` within noise (29.4 vs 29.5 ns).
- **PHP is the one honest regression on the string path** (559 → 690
  ns): PHP FFI dispatch costs per *argument*, and v2 adds a length
  argument plus two out-params to the hot call (this capture also runs
  PHP 8.5 vs the baseline's 8.4). PHP's wins land exactly where the
  structural APIs do — data handle 8.2x at 8 KB, batch 4.2x at
  `simple` (165.5 ns/eval). If the PHP string path matters later, an
  additive NUL-terminated convenience entry point would restore parity;
  deferred until demand appears.

## Workloads

The existing matrix in [BENCHMARK.md](./BENCHMARK.md) measures engine cost
(pre-parsed inputs, no API-shape cost). This analysis measures the opposite:
the full boundary cost a real caller pays per evaluation. Three workloads
spanning the payload range:

| Name          | Rule                                                     | Rule size | Data size |
|---------------|----------------------------------------------------------|-----------|-----------|
| `simple`      | `and` of two comparisons over two `var` lookups          | 74 B      | 68 B      |
| `eligibility` | nested `if`/`and`/`or`/`missing`/`in`/`cat` over an applicant object | 458 B | 955 B |
| `array100`    | `map` over `filter` of a 100-row object array            | 89 B      | 8,279 B   |

All numbers below are **ns per evaluation** (lower is better). "Hot path"
means the best documented pattern per binding: compile once, reuse a
session/arena where the API offers one, JSON text in and out.

## 1. The shared floor: what the current contract costs before any FFI

Measured on the core crate directly (compile once, then per call):

| Core tier                                            | simple | eligibility | array100 |
|------------------------------------------------------|-------:|------------:|---------:|
| evaluate only (data pre-parsed, arena reused)        |     31 |         139 |      727 |
| + parse data JSON per call                           |    107 |         886 |   10,466 |
| + serialize result to a JSON `String`                |    133 |         993 |   11,087 |
| same, but fresh `Bump` arena per call                |    163 |       1,052 |   11,467 |
| `serde_json::Value` in, `Value` out (object bridge)  |     78 |         570 |    6,010 |

Reading it:

- **Parsing the data JSON dominates the string contract.** It is 71% of the
  string-path total on `simple`, 75% on `eligibility`, 88% on `array100`.
  The parser is already excellent (single-pass SWAR, zero-copy strings,
  `datavalue` `parser.rs:88`); the cost is architectural: every binding
  re-parses the same payload on every call because nothing in the C ABI or
  the wrapper APIs can hold parsed data across calls
  (`bindings/c/src/session.rs:97`, `crates/datalogic-rs/src/eval_input.rs:67-73`).
- **Result serialization is comparatively cheap** here (+26 to +620 ns)
  because these results are small. It scales with result size, not input
  size.
- **Fresh arena per call costs 30 to 380 ns** versus arena reuse. Real, but
  an order of magnitude smaller than the parse. This is why Session tiers
  barely move the needle below.
- **The `serde_json::Value` bridge is cheaper than text parsing** on the
  Rust side. The object-path pain measured in Node and Python (section 3)
  comes from the host-side walk, not from `Value -> DataValue`.

The "parse-eval-serialize" row (133 / 993 / 11,087) is the **string-contract
floor**: no binding can beat it without changing the contract, and every
binding's hot path should be judged by its distance from it.

## 2. Hot path per binding

**(Re-captured post-v2; the pre-v2 table is in the appendix's
historical baseline.)** Compile-once + session, JSON string in/out:

| Binding                     | simple | eligibility | array100 | Fixed overhead vs floor (simple) |
|-----------------------------|-------:|------------:|---------:|---------------------------------:|
| string-contract floor       |  140.3 |     1,013.4 | 12,016.6 | 0 |
| C ABI, called from C        |  122.5 |       949.9 | 12,327.1 | -18 |
| .NET (`Session.Evaluate`)   |  157.3 |     1,064.5 | 12,739.2 | +17 |
| Go (cgo)                    |  204.9 |     1,059.6 | 12,723.5 | +65 |
| JVM (FFM)                   |  261.4 |     1,182.5 | 12,973.8 | +121 |
| Python (`evaluate_str`)     |  279.5 |     1,200.7 | 12,367.7 | +139 |
| Node (`evaluateStr`)        |  341.4 |     1,366.0 | 13,289.9 | +201 |
| WASM (session)              |  594.5 |     3,535.3 | 33,358.6 | +454 |
| PHP (FFI)                   |  666.5 |     1,535.4 | 12,746.7 | +526 |

And all nine bindings on the structural tier — parse the payload once
(`datalogic_data_parse` / `DataHandle`), then evaluate:

| Binding (data handle) | simple | eligibility | array100 | 100 rules × 1 payload (per eval, simple) |
|-----------------------|-------:|------------:|---------:|------------------------------------------:|
| C ABI                 |   39.3 |       158.6 |  1,002.2 | 39.3 |
| .NET                  |   50.4 |       173.9 |  1,027.5 | 62.6 |
| Go                    |  118.8 |       243.3 |  1,075.4 | 60.7 |
| JVM                   |  132.0 |       295.8 |  1,141.4 | 71.4 |
| Python                |  170.7 |       378.2 |  1,359.9 | 90.3 |
| Node                  |  212.8 |       434.1 |  1,525.7 | 618.4 |
| WASM                  |  349.2 |       732.9 |  4,041.1 | 687.3 |
| PHP                   |  575.0 |       706.7 |  1,530.5 | 164.8 |

(Node's and WASM's batch column exceeds their single-handle column
because each batch item materialises an allSettled-style result object
across the JS boundary; the other bindings return leaner per-item
shapes.)

(Cross-process run-to-run variance is roughly ±5%; treat single-digit
percent differences between adjacent rows as noise. On the string tier
at 8 KB the shared parse still dominates and the native bindings
converge to ~12-13 µs — which is exactly the cost the data-handle tier
removes: at 8 KB it runs ~12x faster across C, .NET, Go, and JVM.)

Takeaways, per binding (post-v2):

- **C, .NET, and Go now sit within ~75 ns of the floor** — and the raw
  C session path is *below* it, because v2's borrowed-result contract
  skips the result-`String` allocation the floor row includes. What
  remains is UTF-8 marshalling and call dispatch.
- **The JVM outlier is gone**: from 10-15x the .NET boundary cost under
  JNA interface mapping to ~1.7x under eager FFM downcall handles.
  Argument strings are now explicitly UTF-8, so the v1 default-charset
  correctness hazard is structurally impossible. JDK 22+ required; add
  `--enable-native-access=ALL-UNNAMED` on JDK 24+.
- **Python and Node string paths are unchanged by design** (they bind
  the Rust core directly, not the C ABI) and reproduce the baseline
  within noise. Node's object-typed entry points now take the same
  string fast-path as `evaluateStr` when handed JSON text, and
  Python's object path converts via the direct walk (section 3).
- **WASM's string path is unchanged, but the mirror changes its
  story**: a resident `DataHandle` removes the per-call JS→WASM
  payload copy + in-module parse, collapsing the 8 KB session path
  8.3x (33,359 → 4,041 ns). The remaining gap to native handles is
  eval speed inside the size-optimized module — which is what the
  opt-in `WASM_PROFILE=speed` build buys back (up to 1.85x on handle
  tiers).
- **PHP traded ~130 ns on the string path for the structural wins**:
  PHP FFI dispatch costs per argument, and v2's hot call gained a
  length argument and two out-params (the capture also moved from PHP
  8.4 to 8.5). Its hot lane is now the handle + batch tier — 164.8
  ns/eval batched vs 666.5 single-call, and 8.3x at 8 KB.

One-shot convenience tiers (compile per call: `apply`, `engine.eval`,
free-function `evaluate`) still cost 5 to 15x the hot path at small
payloads (e.g. Node 4,819 ns vs 331 ns on `simple`). The docs already
steer users to compile-once; no action needed beyond keeping that
guidance loud.

## 3. The object paths: the largest self-inflicted cost

*(Updated post-B6. Python's dict path now converts via a direct
Python↔arena walk — accepted on its bar: 337 / 2,007 / 25,119 ns
versus 822 / 6,754 / 81,608 before (2.5-3.4x), faster than the
dumps/loads round-trip at every size. Node's converter was built to
full behavioral parity, measured 23-31% faster than the serde bridge —
and reverted, because it still lost to the stringify round-trip on all
three workloads; the table below therefore remains Node's current
reality, and the stringify row is its fast lane. The pure-JS row and
the strategic context around it are unchanged.)*

Node and Python also accept native objects. Measured against routing the
same object through the string path:

| Path                                                  | simple | eligibility | array100 |
|-------------------------------------------------------|-------:|------------:|---------:|
| Node `rule.evaluate(obj)`                              |  1,302 |      12,823 |  157,859 |
| Node `JSON.stringify` + `evaluateStr` + `JSON.parse`   |    526 |       2,888 |   26,288 |
| Python `rule.evaluate(dict)`                           |    856 |       6,998 |   88,572 |
| Python `json.dumps` + `evaluate_str` + `json.loads`    |  1,917 |       7,240 |   70,314 |
| pure-JS `json-logic-engine` (compiled, object in, **no boundary**) | 3.6 | 111 | 536 |

- **The Node object path is strictly worse than a JSON text round-trip, at
  every size, by 2.4x to 6x.** The napi serde bridge walks the JS object
  with ~7 N-API calls and a UTF-16 -> UTF-8 -> UTF-16 key round-trip per
  property (napi-rs `serde.rs:35-110`, `object.rs:736-819`), builds a
  `serde_json::Value` tree, deep-copies it into the arena, and does the
  reverse dance for the result. Four tree materializations per call. V8's
  own `JSON.stringify` plus the SWAR parser is far cheaper than that walk.
- **Python is subtler**: `pythonize` beats a `json.dumps` round-trip at
  small sizes (Python's json module has ~1.5 µs of fixed interpreter cost
  per call) but loses above ~1 KB, where per-node C-API cost compounds. The
  README's blanket "3-10x faster than a JSON-string round-trip" claim
  (`bindings/python/README.md:84-86`) is true only below roughly 1 KB and
  reverses at 8 KB.
- **The pure-JS engine line is the strategic context for the JS bindings**:
  when the data is already a JS object and rules are small, a JIT-compiled
  JS engine with zero boundary beats every native option by an order of
  magnitude (this microbenchmark flatters it with perfectly warm inline
  caches, but the repo's own 44-suite matrix agrees directionally:
  47 ns geomean vs 856 ns for WASM). The native bindings win on: string
  payloads (no JS-side materialization), large/complex rules, spec
  conformance, deterministic performance, memory bounds, and worker/thread
  parallelism. The docs should say this plainly rather than leaving
  "native = fast" implied for all shapes.

## 4. Overhead taxonomy

Every cost identified, ranked by measured impact, with who pays it:

| # | Cause | Who pays | Evidence |
|---|-------|----------|----------|
| T1 | Data JSON re-parsed every call (no resident-data handle) | all 8 bindings | `eval_input.rs:67-73`; floor table above |
| T2 | Host-object tree walks + double materialization (`serde_json::Value` intermediates both directions) | Node/Python object paths | napi `serde.rs:35-110`; `conv.rs:22-30` (pythonize); 4 trees per call |
| T3 | JNA interface-mapping reflective dispatch | JVM | +3,136 ns fixed vs +84 ns for .NET |
| T4 | Size-profile WASM build (`opt-level="z"` twice) + JS<->wasm string copies | WASM | 2.8x floor at 8 KB; `bindings/wasm/Cargo.toml:60-65`, `build.sh:62` |
| T5 | Result serialized to a fresh malloc'd string + a second FFI crossing to free it | C, Go, JVM, .NET, PHP | `session.rs:101`, `lib.rs:78-85` |
| T6 | NUL-terminated strings: strlen + separate UTF-8 validation pass in, NUL scan out; forces `C.CString` copy in Go | C-family | `lib.rs:89-97`, `lib.rs:105-117` |
| T7 | Thread-local last-error: TLS clear per call and Go's per-call `LockOSThread` | C-family, worst in Go | `error.rs:64-68`; `datalogic.go:195-201` |
| T8 | Fresh `Bump` arena per call on non-session tiers | all except session users | +30 to +380 ns; `rule.rs:74` |
| T9 | No batch API anywhere: N evaluations = N full crossings | all 8 | every binding, grep "batch" = 0 hits |
| T10 | Python wheel ships without LTO (`[profile.release]` missing entirely) | Python | `bindings/python/Cargo.toml`; the comment at `bindings/node/Cargo.toml:57-59` claiming inheritance is wrong |
| T11 | PHP `FFI::cdef` per process instead of a preloadable `FFI::load` header; incompatible with `ffi.enable=preload` deployments | PHP | `Native.php:87-92` |
| T12 | One-shot tiers recompile the rule (and `apply` builds an engine) per call | users who hold them wrong | measured 5-25x hot path |
| T13 | Custom operator bridge: JSON round-trip + 2 boundary crossings per operator invocation | all bindings with custom ops | e.g. `builder.rs:277-327` |
| T14 | JVM argument strings encoded with default charset, not forced UTF-8 | JVM | `DatalogicNative.java:30` vs `Engine.java:124` |
| T15 | Sync-only Node API blocks the event loop for the full call | Node | no async/AsyncTask variants in `bindings/node/src/` |

## 5. Options to reduce overhead

*(Historical record — the outcome of every option is in the "Outcome"
section at the top: A1/A3/A4/A6 shipped as-is, B1-B5 + B7 + B8 shipped
collapsed into the C ABI v2 (C1), B10/C2 shipped as the FFM-only JVM
rewrite, A2 was superseded, A5/B6/B9 deferred, C3 rejected.)*

Graded: **A** = drop-in, no API change, no break. **B** = additive API
(non-breaking; old paths stay). **C** = structural/breaking (next major).
Expected gains are derived from the measurements above; treat them as
directional until re-measured after implementation.

### A. Drop-in fixes

| ID | Change | Expected effect | Notes |
|----|--------|-----------------|-------|
| A1 | Add `[profile.release] lto = "fat", codegen-units = 1, strip = true` to `bindings/python/Cargo.toml` (T10); fix the stale comment in `bindings/node/Cargo.toml:57-59` | Single-digit % on Python eval-heavy work; free | The wheel currently loses cross-crate inlining that node/c/wasm all have. Zero risk. |
| A2 | JVM: switch JNA interface mapping to **direct mapping** (`Native.register`) and set `Library.OPTION_STRING_ENCODING`/`jna.encoding` to UTF-8 (T3, T14) | Fixed cost from ~3.1 µs toward the several-hundred-ns range (JNA's documented "substantially faster, near custom JNI"); also fixes the non-ASCII encode hazard | Same public API, internal-only. The single highest-leverage per-binding fix in the repo. |
| A3 | Node: route string-typed `Rule.evaluate`/`Session.evaluate` input through `DataValue::from_str` instead of `unify_input`'s `serde_json::Value` detour (`conv.rs:27-33`) | Removes one full tree build for string inputs on the object-typed entry points | Behavior-identical; `evaluate_str` already does this (`engine.rs:392-397`). |
| A4 | PHP: ship an `FFI::load`-compatible header + document `opcache.preload` (T11); keep `FFI::cdef` fallback | Removes per-process header parse + dlopen; enables `ffi.enable=preload` production configs; modest per-call gain | Additive file + docs; no break. |
| A5 | WASM: build and measure a speed-profile variant (`opt-level = 3` or `"s"`, `wasm-opt -O3`) before deciding; potentially publish as the default if size delta is acceptable, or as a second artifact | Parse+eval inside wasm is the scaling cost; a speed build should close part of the 2.8x-at-8KB gap | Needs a size-vs-speed decision (browser users care about bytes). Measure first, it may be a 20-40% eval win for tolerable size. |
| A6 | Docs honesty pass: qualify the Python "3-10x" object-path claim by payload size; add "when a pure-JS engine is the right choice" guidance to the Node/WASM READMEs; keep steering to compile-once + session | Prevents users from picking the slowest path believing it is the fastest | Follows directly from section 3 measurements. |

### B. Additive API (non-breaking, highest structural leverage)

The C ABI changes here lift Go, JVM, .NET, and PHP simultaneously; the
Node/Python items are their own crates.

| ID | Change | Expected effect | Notes |
|----|--------|-----------------|-------|
| B1 | **Parsed-data handle in the C ABI + all bindings** (T1): `datalogic_data_parse(json, len) -> datalogic_data*`, plus `datalogic_rule_evaluate_data(rule, data_handle)`; internally an owned self-contained tree (`OwnedDataValue` or `self_cell` arena), fed to the engine through the existing zero-cost `&DataValue` passthrough (`eval_input.rs:51-56`) | Amortizes the dominant cost. Rule-set example (10 rules over one `eligibility` payload): today 10 × 993 ns floor ≈ 9.9 µs of core work; with one parse + 10 evals ≈ 0.75 + 10 × 0.25 ≈ 3.2 µs, a ~3x floor reduction, more at larger payloads | The single biggest contract fix. Immutable handles; document lifetime. Mirrors the trick `bin/self.rs` already uses natively. |
| B2 | **Batch evaluate** (T9): one rule × N payloads (array of strings or one length-prefixed buffer), and N rules × one payload, returning packed results | Amortizes crossing + TLS + (with B1) parse over N. JVM `simple` today: 100 calls ≈ 327 µs; batched ≈ 1 crossing + 100 × floor ≈ 16 µs, ~20x. Even .NET gains ~1.4x at small payloads | Biggest for JVM/Go; natural fit for the rule-set/feature-flag use case. |
| B3 | **Typed scalar results** (T5): `datalogic_rule_evaluate_bool/_i64/_f64(rule, data, out*) -> status` | Skips result `String` + `CString` + the free crossing + host-side decode. Predicate rules are the common case; saves ~50-80 ns of the C-family fixed cost and halves crossings | Falls back with a status code when the result is not scalar. |
| B4 | **(ptr,len) input/output variants** (T6): accept non-NUL-terminated buffers with explicit length; return length alongside the pointer | Deletes strlen + separate UTF-8 pass in (one `from_utf8` pass instead of two scans), NUL scan out; lets Go pass pinned string bytes without the `C.CString` malloc+copy (4 crossings -> 2), lets .NET use spans | Pure addition beside the existing functions. |
| B5 | **Error out-param variants** (T7): `..._e(args, dl_error* out)` returning status | Removes the per-call TLS clear and, in Go, the `LockOSThread`/`Unlock` pair entirely | Also makes JNA direct mapping cleaner (no TLS read ordering concern). |
| B6 | **Direct host-object converters** (T2): napi-rs JS-object -> arena `DataValue` walk (skip `serde_json::Value` both directions), same for pyo3 (`PyAny` -> arena directly, result -> Python objects directly) | Removes 2 of 4 tree materializations per object call. Node object path should land well under the stringify round-trip it currently loses to (target: ≤ 26 µs at 8 KB from 158 µs; Python dict path from 88 µs toward ~50-60 µs) | Medium effort, contained in each binding crate. The napi property walk itself remains the floor; if it still loses to `JSON.stringify`+parse, document the string path as the fast lane for large objects. |
| B7 | **Session/arena pooling behind `rule_evaluate`** (T8): thread-local or lock-free pool of `Bump` arenas inside the C ABI (and node/python fresh-arena paths) | Recovers the +30-380 ns fresh-arena tax without requiring users to discover Session; removes the "Session is not thread-safe" trap for Go servers | Invisible to APIs. Bound pool size; reset-with-capacity discipline as in the bench harness. |
| B8 | **Buffer-reuse output** plumbed through core: expose `write_json_into(&mut Vec<u8>)` (already in `datavalue` `emit.rs:374`) via `Engine`/`Session`, use caller-owned buffers in the C ABI (`evaluate_into(buf, cap) -> needed_len`) | Removes per-call `String` alloc + growth reallocs + the free crossing on the result side | Pairs with B4. |
| B9 | **Node async tier** (T15): `AsyncTask`-based `evaluateAsync` for large payloads | Not faster per op, but stops multi-µs evaluations from blocking the event loop; `Logic` is already `Send + Sync` | Additive method. |
| B10 | **JVM FFM backend** (JDK 22+ `java.lang.foreign`) selected at runtime when available, JNA fallback otherwise | Brings JVM fixed cost from µs-range to .NET-like levels; also resolves the JDK 24+ restricted-access warnings | Additive if auto-selected; heavier build/test matrix (multi-release JAR). A2 first, this second. |

### C. Structural / breaking (candidates for a future major)

| ID | Change | Why it might be worth a break |
|----|--------|-------------------------------|
| C1 | **C ABI v2**: (ptr,len) everywhere, caller-owned output buffers, status-code errors (no TLS), data handles and batch as first-class, no NUL-terminated contract | Collapses B1/B2/B3/B4/B5/B8 into one coherent surface instead of "variant" sprawl. Old symbols could ship alongside for one major. In-tree consumers migrate in lockstep; only external direct-C users break. |
| C2 | JVM drops JNA for FFM as the only backend (JDK 22+ floor) | Deletes the slowest FFI mechanism in the project and a dependency; blocked on ecosystem JDK floor tolerance. |
| C3 | Binary interchange format (CBOR/MessagePack) instead of JSON text | **Not recommended now**: the SWAR JSON parser is fast, host JSON serializers (V8, PHP) are heavily optimized, and B1/B2 remove more parse work than a format swap would, without a new dependency in 8 ecosystems. Revisit only if profiling after B1/B2 shows serialization still dominant. |
| C4 | WASM structural changes (resident data handles inside the module, shared-buffer views) | The JS<->wasm copy is unavoidable for strings; a data-handle API (B1 mirrored into the WASM classes) is non-breaking and captures most of it. Full redesign only if evidence demands. |

## 6. Suggested sequencing

*(Historical — executed 2026-07-03 in a compressed form: step 0 shipped
as `tools/benchmark/boundary/`; the A-tier quick wins landed; and steps
3-5's additive staging was replaced by the direct C ABI v2 build per
the decision at the top of this document.)*

1. **Step 0, before any optimization: make these numbers reproducible in-tree.**
   Port the per-binding boundary runners into `tools/benchmark/runners/`
   (one file per runtime, same three workloads, same discipline) and give
   `compare.rs` a `--boundary` mode or a small driver script. Everything
   below should be judged by this harness, and the repo currently has no
   per-binding benchmark at all (the canonical README block quotes
   core-only numbers in every binding).
2. **A-tier quick wins**: A1 (Python LTO), A2 (JNA direct mapping + UTF-8),
   A3 (Node string fast-path), A4 (PHP preload header), A6 (docs honesty).
   A5 (WASM speed profile) as a measured experiment.
3. **B1 + B2 + B3 as one C ABI extension arc** (data handles, batch, typed
   results), rolled through Go/JVM/.NET/PHP, then mirrored as native APIs
   in Node/Python/WASM. This is where the structural multiples live
   (3x to 20x for rule-set and batch workloads).
4. **B6 object converters** for Node/Python once the string-side contract
   is settled, with the section 3 tables as the acceptance bar.
5. Revisit **C1** (ABI v2) only after B-tier ships and external C-consumer
   feedback exists; C3 likely never.

## 7. Resolved: the `dlrs:wasm:compiled` ERR cells in BENCHMARK.md

The 2026-05-10 matrix shows ERR for `dlrs:wasm:compiled` on 12 suites that
`dlrs:engine` passes. Investigated 2026-07-03; two distinct root causes.

**Mechanism.** The node runner wraps each case's `new CompiledRule(...)`
in a bare `try/catch`; a failed case becomes an always-throwing stub
(`tools/benchmark/runners/node-runner.js:98-101`), and a cell renders ERR
when more than 50% of its cases error (`ERR_THRESHOLD`,
`src/bin/compare.rs:55`). Suites dedicated to one operator therefore flip
to ERR wholesale when that operator is missing.

**Cause 1, stale feature set (11 of 12 suites, self-healed).** On capture
day the wasm crate compiled the core with `features = ["wasm"]`, and the
core's `wasm` meta-feature was only `["datetime", "trace", "templating"]`
(commit `e7c28e8`, the pre-flatten `packages/wasm` layout). No
`error-handling`, no `ext-string/array/control/math`, so `try`, `length`,
`sort`, `slice`, `coalesce`, `exists`, `abs`, `ceil`, `floor`, and the
string suite were unknown operators. The very next day the flatten commit
(`ae33586`) switched the wasm crate to the full explicit feature list, but
the matrix was never re-captured. Re-run on 2026-07-03 with the current
build: all 11 suites pass 100% of cases. Fresh cells (ns/op, same
discipline): coalesce 754, ceil 736, floor 787, abs 854, exists 844,
string/string 1,109, slice 1,196, try.extra 1,933, sort 1,930, length
1,631, try 2,577.

**Cause 2, a real bug, since fixed (`datetime/now.json`).** The core
declares `chrono` with `default-features = false, features = ["std",
"clock"]` and **no `wasmbind`** (`crates/datalogic-rs/Cargo.toml:78`). On
`wasm32-unknown-unknown`, `Utc::now()` falls through to
`SystemTime::now()`, which panics with "time not implemented on this
platform"; under `panic = "abort"` the panic becomes a wasm trap that
surfaces in JS as `RuntimeError: unreachable`. All 10 positive cases of
`datetime/now.json` fail this way today; the other datetime suites pass
because they never read the clock. Two operational notes: the trap is
empirically contained (an unrelated compiled rule still evaluates
correctly afterwards), but each trapped call leaks its per-call `Bump`
arena since the drop never runs, and wasm linear memory only grows. The
stale-cells note in BENCHMARK.md wrongly listed `now.json` among the
cells a re-capture would fix; the note and the matrix were both
corrected on 2026-07-03.

**Fix shipped (2026-07-03).** Cause 2 is fixed by a new opt-in core
feature, `wasm-clock = ["chrono?/wasmbind"]`, which the wasm-bindgen
binding enables; `now` measures 2,108.6 ns in the re-captured matrix.
It is deliberately NOT unconditional: enabling `wasmbind` for all wasm32
builds links `__wbindgen_placeholder__` imports that non-JS wasm
runtimes (wasmtime, wazero, Chicory) cannot satisfy. That was issue #47,
fixed in v4.0.19 as the opt-in `wasm` feature; the v5 rewrite dropped
both the unconditional dep and the opt-in, which is how `now` silently
broke for JS hosts. Guards against either direction resurfacing: a
`wasm-bindgen` test asserts `now` returns a parseable timestamp
(`bindings/wasm/tests/web.rs`), and a CI step asserts the default wasm32
dependency graph of the core stays free of `wasm-bindgen`/`js-sys`
(`.github/workflows/ci.yml`, wasm job). The re-capture also exposed and
fixed the same bug class in the harness itself: `datalogic-bench` was
missing the `flagd` feature, so `flagd/*.json` reported ERR for the
native column. The injectable-clock idea (deterministic `now` via
`EvaluationConfig`) remains open as a future enhancement.

## Appendix: full result tables

Current capture (ABI v2 + mirror wave, 2026-07-03), ns/op, median of
5 — reproduce with `tools/benchmark/boundary/run.sh all` (the
`dumps-str-loads-roundtrip`/`array100` cell was re-measured once after
a transient outlier in the batch run; every other cell is the single
run):

```
runtime    mode                                 simple  eligibility    array100
rust-core  eval-preparsed                         29.2        132.8       648.7
rust-core  parseddata-eval                        29.2        132.2       648.1
rust-core  parse-eval                            111.3        900.8    11,419.7
rust-core  parse-eval-serialize                  140.3      1,013.4    12,016.6
rust-core  parse-eval-serialize-fresharena       161.8      1,073.1    12,467.4
rust-core  serde-value-in-out                     76.9        560.4     6,110.1
c-abi      session-evaluate                      122.5        949.9    12,327.1
c-abi      session-evaluate-data                  39.3        158.6     1,002.2
c-abi      session-evaluate-many-100              39.3        161.3     1,008.0
c-abi      rule-evaluate                         141.4      1,054.7    12,528.6
c-abi      engine-apply-oneshot                1,510.9      8,064.1    13,984.8
dotnet     session-evaluate                      157.3      1,064.5    12,739.2
dotnet     session-evaluate-data                  50.4        173.9     1,027.5
dotnet     session-evaluate-many-100              62.6        187.4     1,039.6
dotnet     rule-evaluate                         190.2      1,170.0    12,967.7
dotnet     engine-apply-oneshot                1,557.3      8,104.0    14,420.6
python     session-evaluate-str                  279.5      1,200.7    12,367.7
python     session-evaluate-data                 170.7        378.2     1,359.9
python     session-evaluate-many-100              90.3        291.3     1,295.7
python     rule-evaluate-str                     276.8      1,226.7    12,631.9
python     rule-evaluate-dict                    337.4      2,006.7    25,119.2
python     dumps-str-loads-roundtrip           1,902.0      6,782.4    69,855.5
python     engine-eval-oneshot                 3,429.7     16,103.9    28,807.4
node       session-evaluateStr-str               341.4      1,366.0    13,289.9
node       session-evaluate-data                 212.8        434.1     1,525.7
node       session-evaluate-many-100             618.4        834.6     1,936.6
node       rule-evaluateStr-str                  332.5      1,395.2    13,650.3
node       rule-evaluate-obj                   1,342.1     12,936.4   163,065.8
node       stringify-str-parse-roundtrip         538.6      2,682.0    26,385.2
node       engine-eval-oneshot                 4,912.1     29,982.1   166,562.6
go         session-evaluate                      204.9      1,059.6    12,723.5
go         session-evaluate-data                 118.8        243.3     1,075.4
go         session-evaluate-many-100              60.7        185.8     1,022.0
go         rule-evaluate                         266.5      1,212.2    12,987.2
go         engine-apply-oneshot                1,685.7      8,376.9    14,530.4
jvm        session-evaluate                      261.4      1,182.5    12,973.8
jvm        session-evaluate-data                 132.0        295.8     1,141.4
jvm        session-evaluate-many-100              71.4        195.6     1,047.3
jvm        rule-evaluate                         289.2      1,251.5    13,260.3
jvm        engine-apply-oneshot                1,754.8      8,335.0    14,809.5
php        session-evaluate                      666.5      1,535.4    12,746.7
php        session-evaluate-data                 575.0        706.7     1,530.5
php        session-evaluate-many-100             164.8        290.2     1,131.9
php        rule-evaluate                         642.7      1,578.8    12,889.2
php        encode-eval-decode-roundtrip          795.2      2,856.7    26,755.6
php        engine-apply-oneshot                2,053.8      8,600.8    14,353.9
wasm       session-evaluate-str                  594.5      3,535.3    33,358.6
wasm       session-evaluate-data                 349.2        732.9     4,041.1
wasm       session-evaluate-many-100             687.3      1,113.0     4,500.2
wasm       compiledrule-evaluate-str             606.1      3,312.9    31,578.7
wasm       oneshot-evaluate                    3,378.5     18,865.8    76,235.1
```

The Python object-path rows reflect the accepted B6 direct converter
(`rule-evaluate-dict` was 822 / 6,754 / 81,608 in the pre-mirror v2
capture; `engine-eval-oneshot` improves for the same reason). Node's
object rows are unchanged — its converter was reverted (section 3).

Historical baseline (pre-v2, captured earlier the same day — the
numbers sections 1-6 were written against; JVM rows are the JNA
binding, PHP rows are PHP 8.4):

```
runtime      mode                              simple   eligibility   array100
rust-core    eval-preparsed                      30.8         138.9      726.9
rust-core    parse-eval                         106.7         885.6   10,465.7
rust-core    parse-eval-serialize               133.1         992.5   11,086.9
rust-core    parse-eval-serialize-fresharena    163.3       1,052.1   11,467.3
rust-core    serde-value-in-out                  77.9         569.6    6,010.0
c-abi        session-evaluate                   186.0       1,134.9   12,799.4
c-abi        rule-evaluate                      217.3       1,218.6   13,192.4
c-abi        engine-apply-oneshot             1,570.7       8,225.3   14,679.0
dotnet       session-evaluate                   216.7       1,262.7   13,341.3
dotnet       rule-evaluate                      239.8       1,326.4   13,765.3
dotnet       engine-apply-oneshot             1,612.0       8,366.5   15,286.0
python       session-evaluate-str               270.0       1,199.6   12,403.6
python       rule-evaluate-str                  264.2       1,223.1   12,671.3
python       rule-evaluate-dict                 856.0       6,998.3   88,572.4
python       dumps-str-loads-roundtrip        1,916.8       7,240.4   70,314.3
python       engine-eval-oneshot              4,154.0      22,573.7   92,499.3
node         session-evaluateStr-str            333.4       1,321.2   12,009.7
node         rule-evaluateStr-str               322.5       1,358.7   12,385.8
node         rule-evaluate-obj                1,301.9      12,822.7  157,858.7
node         stringify-str-parse-roundtrip      526.2       2,887.9   26,288.0
node         engine-eval-oneshot              4,882.5      29,886.4  161,732.7
go           session-evaluate                   358.5       1,352.9   13,220.9
go           rule-evaluate                      389.6       1,433.6   13,719.7
go           engine-apply-oneshot             1,896.9       8,691.5   15,384.4
jvm          rule-evaluate                    2,553.9       4,069.0   19,623.6
jvm          session-evaluate                 3,268.7       3,919.0   18,950.5
jvm          engine-apply-oneshot             5,242.8      11,927.4   19,983.5
php          session-evaluate                   559.3       1,450.4   11,980.7
php          rule-evaluate                      544.3       1,481.1   12,365.9
php          encode-eval-decode-roundtrip       716.5       3,174.6   27,061.5
php          engine-apply-oneshot             1,907.6       8,483.6   13,868.1
wasm         session-evaluate-str               552.5       3,103.1   30,724.2
wasm         compiledrule-evaluate-str          586.8       3,198.6   30,994.0
wasm         stringify-str-parse-roundtrip      789.9       4,806.5   45,510.1
wasm         oneshot-evaluate                 3,284.0      18,837.7   72,589.7
js-jle       jle-compiled-obj (no boundary)       3.6         110.6      536.3
```

Methodology notes:

- Timing loop: warmup (2,000 iterations, 5,000 on JIT runtimes), pilot pass
  to size N for ~250 ms per sample, median of 5 samples. Results consumed
  (`black_box`/sink) to prevent elision. Native calls are opaque to JITs,
  so dead-code elimination is not a concern on binding paths; it is for the
  pure-JS row, which additionally benefits from perfectly warm inline
  caches (single hot object identity). Treat that row as a best case.
- The Python wheel measured here was built by `maturin build --release`
  with the crate's current (LTO-less) profile, i.e. what PyPI users get
  (see A1). Other Rust artifacts build with their in-tree profiles
  (fat LTO for node/c; size-first for wasm).
- The JVM runner used OpenJDK 26 (Homebrew) because the system `java` stub
  has no runtime; JNA emits restricted-native-access warnings there.
- The v1 capture's runners lived outside the repo; they now ship
  in-tree as [`tools/benchmark/boundary/`](./boundary/README.md) with
  the workloads checked in byte-stable (the v2 capture above came from
  them). Exact workload definitions:

```jsonc
// simple: rule 74 B, data 68 B
{"and": [{">": [{"var": "age"}, 18]}, {"==": [{"var": "country"}, "US"]}]}
{"age": 21, "country": "US", "name": "Ada Lovelace", "tier": "gold"}

// eligibility: rule 458 B, data 955 B
{"if": [
  {"and": [
    {">=": [{"var": "applicant.age"}, 21]},
    {"<":  [{"var": "applicant.age"}, 65]},
    {"or": [
      {">=": [{"var": "applicant.income"}, 45000]},
      {"and": [{">=": [{"var": "applicant.credit_score"}, 700]},
                {"<=": [{"var": "applicant.debt_ratio"}, 0.3]}]}
    ]},
    {"!": {"missing": ["applicant.ssn", "applicant.address.zip"]}},
    {"in": [{"var": "applicant.state"}, ["CA", "NY", "TX", "WA", "MA"]]}
  ]},
  {"cat": ["approved:", {"var": "applicant.id"}]},
  "rejected"
]}
// data: applicant{id,age:34,income:52000,credit_score:715,debt_ratio:0.22,
//   ssn,state:"CA",address{street,city,zip:"95014",country},
//   employment{...}, accounts[3], flags{...}}, meta{...}, padding[8 x 32-char]

// array100: rule 89 B, data 8,279 B
{"map": [{"filter": [{"var": "items"}, {">": [{"var": "price"}, 250]}]},
          {"var": "qty"}]}
// data: {"items": [{"id": i, "price": (i*37) % 500, "qty": i % 7,
//   "name": "item-%04d", "tags": ["retail","q3"]} for i in 0..100]}
```
