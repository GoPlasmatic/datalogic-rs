# Binding boundary costs

What each language binding costs per evaluation on top of the Rust
core, per API tier, and why. This is the canonical **boundary-cost
reference** — the complement to [BENCHMARK.md](./BENCHMARK.md), which
measures engine cost with pre-parsed inputs and no API-shape cost.
Link here from other docs rather than re-quoting numbers inline.

Every number is reproducible in-tree with the boundary harness:
[`boundary/README.md`](./boundary/README.md).

> **Captured:** 2026-07-03 • Apple M2 Pro (arm64), macOS 26.5 • Rust
> 1.96 release builds • Node v22 • CPython 3.13 (abi3 wheel) • Go 1.25
> • OpenJDK 26 via FFM • .NET SDK 9 (net9.0 console over net8.0
> binding) • PHP 8.5 with FFI • median of 5 samples, each sized to
> ~250 ms, after warmup; results consumed so work can't be elided.
> Same three workloads in every runtime; every runtime produced
> byte-identical results before timing started. One
> `tools/benchmark/boundary/run.sh all` run produced every current
> table. A pre-5.0.1 baseline is preserved at the end of the appendix.

## Workloads

Three workloads spanning the payload range:

| Name          | Rule                                                     | Rule size | Data size |
|---------------|----------------------------------------------------------|-----------|-----------|
| `simple`      | `and` of two comparisons over two `var` lookups          | 74 B      | 68 B      |
| `eligibility` | nested `if`/`and`/`or`/`missing`/`in`/`cat` over an applicant object | 458 B | 955 B |
| `array100`    | `map` over `filter` of a 100-row object array            | 89 B      | 8,279 B   |

All numbers are **ns per evaluation** (lower is better). "Hot path"
means the best documented pattern per binding: compile once, reuse a
session where the API offers one.

## 1. The shared floor: what each contract costs before any FFI

Measured on the core crate directly (compile once, then per call):

| Core tier                                            | simple | eligibility | array100 |
|------------------------------------------------------|-------:|------------:|---------:|
| evaluate only (data pre-parsed, arena reused)        |   29.2 |       132.8 |    648.7 |
| evaluate via a `ParsedData` handle                   |   29.2 |       132.2 |    648.1 |
| + parse data JSON per call                           |  111.3 |       900.8 | 11,419.7 |
| + serialize result to a JSON `String`                |  140.3 |     1,013.4 | 12,016.6 |
| same, but fresh `Bump` arena per call                |  161.8 |     1,073.1 | 12,467.4 |
| `serde_json::Value` in, `Value` out (object bridge)  |   76.9 |       560.4 |  6,110.1 |

Reading it:

- **Parsing the data JSON dominates the string contract**: the parse
  step alone is 58% / 76% / 90% of the parse-eval-serialize total
  across the three workloads. The parser itself is excellent
  (single-pass SWAR, zero-copy strings); the cost is architectural —
  any tier that receives JSON text per call must pay it per call.
  This is exactly what the **data-handle tier removes**: parse once
  into a `ParsedData`, and per-call cost drops to the eval-only row
  (the handle passthrough is free — first two rows are equal within
  noise).
- **Result serialization is comparatively cheap** here because these
  results are small. It scales with result size, not input size.
- **A fresh arena per call costs 20-450 ns** versus arena reuse —
  real, but an order of magnitude smaller than the parse, which is why
  session tiers alone barely move the needle.

The "parse-eval-serialize" row (140 / 1,013 / 12,017) is the
**string-contract floor**: the reference every binding's string tier
is judged against below.

## 2. Hot path per binding

Compile-once + session, JSON string in/out:

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
at 8 KB the shared parse dominates and the native bindings converge to
~12-13 µs — which is exactly the cost the data-handle tier removes: at
8 KB it runs ~12x faster across C, .NET, Go, and JVM, and 8-9x for
Python, Node, WASM, and PHP.)

Takeaways, per binding:

- **C, .NET, and Go sit within ~65 ns of the floor** — and the raw C
  session path is *below* it, because the borrowed-result contract
  skips the result-`String` allocation the floor row includes. What
  remains is UTF-8 marshalling and call dispatch.
- **JVM** runs on eager `java.lang.foreign` downcall handles at ~1.7x
  the .NET boundary cost, with argument strings explicitly UTF-8.
  JDK 22+ required; add `--enable-native-access=ALL-UNNAMED` on
  JDK 24+.
- **Python and Node** bind the Rust core directly (napi-rs / pyo3),
  not the C ABI. Their string tiers pay one host-string extraction in
  and one result copy out. Object inputs: see section 3.
- **WASM's string tier scales with payload** — the JS→WASM copy plus
  the in-module parse make it 2.8x the floor at 8 KB. A resident
  `DataHandle` removes both per-call costs (33,359 → 4,041 ns at
  8 KB). The remaining gap to native handles is eval speed inside the
  size-optimized module, which the opt-in `WASM_PROFILE=speed` build
  narrows (1.13-1.85x faster across tiers, +8.1% raw size, 1.4%
  smaller gzipped; the published default stays size-optimized).
- **PHP FFI dispatch costs per argument**, which keeps its fixed
  overhead the highest of the native bindings and makes single-call
  savings hard to reach. Its hot lane is the handle + batch tier:
  164.8 ns/eval batched vs 666.5 single-call, and 8.3x at 8 KB. (If a
  leaner single-call string path ever matters for PHP, an additive
  NUL-terminated convenience entry point would help; deferred until
  demand appears.)

One-shot convenience tiers (compile per call: `apply`, `engine.eval`,
free-function `evaluate`) cost 5 to 15x the hot path at small payloads
(e.g. Node 4,912 ns vs 341 ns on `simple`). That is per-call rule
compilation, by design — the docs steer users to compile-once.

## 3. The object paths (Node and Python)

Node and Python also accept native objects. Measured against routing
the same object through the string path:

| Path                                                  | simple | eligibility | array100 |
|-------------------------------------------------------|-------:|------------:|---------:|
| Node `rule.evaluate(obj)`                              |  1,342 |      12,936 |  163,066 |
| Node `JSON.stringify` + `evaluateStr` + `JSON.parse`   |    539 |       2,682 |   26,385 |
| Python `rule.evaluate(dict)`                           |    337 |       2,007 |   25,119 |
| Python `json.dumps` + `evaluate_str` + `json.loads`    |  1,902 |       6,782 |   69,856 |
| pure-JS `json-logic-engine` (compiled, object in, **no boundary**) | ~4 | ~111 | ~536 |

- **Python's dict path converts via a direct Python↔arena walk** (no
  intermediate tree; `pythonize` retained only as the exotic-shape
  fallback, with the semantics pinned by a 549-case equivalence
  corpus). It beats the `json.dumps` round-trip at every payload size,
  so dicts are simply the natural input shape in Python.
- **Node's object path remains the napi serde bridge**, and the JSON
  text round-trip beats it at every size (2.5-6x): V8's own
  `JSON.stringify` plus one string crossing plus the SWAR parser is
  structurally cheaper than a per-property N-API walk. A direct
  converter was built to full behavioral parity and measured 23-31%
  faster than the bridge — still not enough, so it was not shipped;
  the equivalence corpus (`__test__/object-bridge.test.mjs`) remains
  in-tree as the gate for future attempts. **If your Node data is
  already a JS object and the call is hot, stringify it yourself and
  call `evaluateStr`, or better, parse it once into a `DataHandle`.**
- **The pure-JS engine row is the strategic context for the JS
  bindings** (from the pre-5.0.1 capture; unchanged code path): when
  the data is already a JS object and rules are small, a JIT-compiled
  JS engine with zero boundary beats every native option by an order
  of magnitude — that microbenchmark flatters it with perfectly warm
  inline caches, but the repo's cross-library matrix agrees
  directionally. The native bindings win on: string payloads,
  parse-once/batch shapes, large or complex rules, spec conformance,
  deterministic latency, bounded memory, and worker/thread
  parallelism. The Node and WASM READMEs say this plainly.

## 4. Where the nanoseconds go

The measured cost sources across the boundary, and their current
status:

| Cost | Who pays it | Status |
|------|-------------|--------|
| Per-call data JSON parse (58-90% of the string tier) | every string-tier call | **Removed by the data-handle tier** in all nine bindings; string tier remains for one-shot shapes |
| Result malloc + a second crossing to free it | C family (pre-5.0.1) | **Eliminated**: sessions return borrowed bytes from a reusable buffer; one-shots return owned bufs |
| NUL-terminated inputs (strlen + second scan; Go `C.CString` copy) | C family (pre-5.0.1) | **Eliminated**: `(ptr, len)` UTF-8 contract; Go passes string bytes zero-copy |
| Thread-local error state (+ Go `LockOSThread` per call) | C family (pre-5.0.1) | **Eliminated**: status codes + optional error-handle out-params |
| JNA reflective dispatch (~3 µs fixed per call) | JVM (pre-5.0.1) | **Eliminated**: FFM downcall handles (JNA dependency deleted) |
| Fresh arena on session-less tiers | one-shot / rule tiers | **Mitigated**: pooled thread-local arenas give session-grade allocation without a session |
| JS↔WASM payload copy + in-module parse | WASM string tier | **Avoided by resident `DataHandle`**; residual in-module eval speed addressable via the `WASM_PROFILE=speed` opt-in |
| Per-argument FFI dispatch | PHP, every call | Inherent to PHP FFI; **amortized by batch** (one call per set) |
| Host-object tree walk | Node object inputs | Inherent to N-API per-property traffic; the string lane and data handles are the documented fast paths (Python's direct walk shipped; Node's measured attempt did not clear the bar) |
| Per-call rule compile | one-shot tiers | By design; docs steer to compile-once |
| Custom-operator bridge (JSON args in, JSON result out) | bindings with custom operators | Reduced in 5.0.1 (no cross-boundary allocator handoff); the JSON contract itself remains |

## History: the 5.0.1 boundary overhaul

The pre-5.0.1 contract was NUL-terminated strings in, malloc'd strings
out, thread-local error state, no way to hold parsed data across
calls, and a JNA-based JVM binding. Measuring it (the historical
baseline in the appendix) drove a wholesale replacement in 5.0.1: the
C ABI v2 (`(ptr,len)` inputs, status-code errors, borrowed session
results, data handles, batch, typed scalar results, pooled arenas,
allocator-free operator callbacks) rolled through Go/JVM/.NET/PHP in
lockstep, the JVM binding rewritten on FFM, and the same tiers
mirrored natively into Node, Python, and WASM. Headline effect on
`simple` (string tier, v1 → v2): C 186 → 123, .NET 217 → 157, Go
359 → 205, **JVM 3,269 → 261**, PHP 559 → 667 (the one string-path
regression — per-argument FFI dispatch meets the added length/out
params — traded for its 8.3x handle tier and 4x batch tier). Wrapper
public APIs stayed source-compatible; the migration table for direct
C-ABI consumers is in [`MIGRATION.md`](../../MIGRATION.md), and the
change-by-change record is in the repo
[`CHANGELOG.md`](../../CHANGELOG.md).

## Appendix: full result tables

Current capture (2026-07-03), ns/op, median of 5 — reproduce with
`tools/benchmark/boundary/run.sh all` (the
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

Historical baseline (pre-5.0.1, captured 2026-07-03 before the
overhaul — JVM rows are the JNA binding, PHP rows are PHP 8.4, Python
object rows are the pythonize bridge, and no data-handle/batch/typed
tiers existed):

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
- The Python wheel in the historical baseline was built without any
  release profile (no LTO), which is what wheels shipped before 5.0.1;
  the current capture's wheel builds with the fat-LTO profile. Other
  Rust artifacts build with their in-tree profiles (fat LTO for
  node/c; size-first for wasm).
- The JVM runner uses OpenJDK 26 (Homebrew) because the macOS system
  `java` stub has no runtime.
- Runners, workloads, and the driver live in
  [`tools/benchmark/boundary/`](./boundary/README.md); workloads are
  checked in byte-stable. Exact workload definitions:

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
