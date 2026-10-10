<!-- Anthropic's OSS Scanner (https://github.com/anthropics/oss-scanner) reads this file
     before it audits the repository. Human reporters follow SECURITY.md; this file
     restates that scope for the scanner and adds severity guidance. -->
# Threat model

## What this project does and where untrusted input enters

datalogic-rs is a JSONLogic rule engine: a Rust core (`crates/datalogic-rs`) and the
language bindings that wrap it. It evaluates **untrusted rules over trusted data**. End
users write the rules; the host application stores them and controls the data they run
against. Hosts use it for pricing, eligibility, access control, payment routing and
feature flags (flagd), so a rule's result can decide what a user may do.

An attacker reaches the engine through three inputs:

- **Rule JSON**, the main attack surface. The engine parses it (`Engine::compile`,
  `Engine::eval_str`), checks it (`Engine::check`), compiles and optimises it
  (`src/compile/`, `src/node/`), then evaluates it (`src/engine/`, `src/operators/`).
- **Data JSON**. The same parser reads it, and `var`, `val`, `exists` and `missing`
  look values up in it. Treat its content as attacker-controlled; bounding its size is
  the host's job (see "Anything to leave alone").
- **The C ABI** (`bindings/c/src/`): `(pointer, length)` byte slices, opaque handles
  and custom-operator callbacks. The Go, JVM, .NET and PHP bindings call it, and so can
  plain C.

`docs/src/advanced/security.md` defines the sandbox. A rule can read the data the host
passes in, compute with the built-in operators and any custom operators the host
registers, and return a value. It cannot run code, perform I/O (`now` reads the clock
and is the one exception), read outside its input data and the active iteration scope,
or mutate the input, the engine or shared state.

## Components that matter most / least

Most:

- `bindings/c/src/` holds the only hand-written `unsafe` in shipped code; the core crate
  is `#![forbid(unsafe_code)]`. Look hardest at handle lifetimes, borrowed versus owned
  result buffers, the v2 custom-operator callback contract, the re-entrancy rules, and
  panic containment (`ffi_guard` / `guard_status` in `lib.rs`).
  `bindings/c/include/datalogic.h` and ARCHITECTURE.md ("The C ABI Boundary") define
  the contract.
- The compile pipeline and optimiser (`src/compile/`, `src/node/`). Constant folding,
  CSE, dead-code elimination, scope binding and the fast paths must not change what a
  rule means.
- The evaluator, operators and arena values (`src/engine/`, `src/operators/`,
  `src/arena/`), and the documented resource bounds: JSON parse depth (256), compile
  nesting depth (256), `max_recursion_depth` (256), `reduce` accumulator nesting
  (1,024), the tensor constructor cap (2^28 elements) and `ops_budget` (the `budget`
  feature).
- Templating mode (the `templating` feature, `with_template_key_escape`).

Also in scope, lower priority:

- The other binding shims: `bindings/node` (napi-rs), `bindings/python` (pyo3) and
  `bindings/wasm` (wasm-bindgen). Each puts an `unsafe impl Send`/`Sync` on its
  custom-operator or shared-data wrapper.
- Handle management in the Go, JVM, .NET and PHP wrappers (`bindings/go`,
  `bindings/jvm`, `bindings/dotnet`, `bindings/php`). A use-after-free or double free
  that a wrapper's own code causes is in scope. This image has no toolchains for these
  languages, so review them by reading the source.
- `crates/datalogic-bind`, the wire formats the bindings share.

Out of scope:

- `tools/` (benchmark), `scripts/`, `docs/`, `.github/`, examples, and test code
  (`crates/datalogic-rs/tests/`, `bindings/*/tests/`). The `unsafe` counting allocator
  in `crates/datalogic-rs/tests/owned_input_test.rs` belongs to the tests.
- `ui/`, the React debugger, a developer tool. XSS from a rule someone pastes into it is
  out of scope unless it escapes the sandbox.
- Dependencies (`datavalue-rs`, `bumpalo`, `serde_json`, `chrono`, ...), unless this
  repository's code makes a flaw in one reachable from a rule.

## How to exercise it

The Dockerfile warms the build caches, and the commands below run offline from `/src`.

- Core tests: `cargo test --workspace --all-features`. Pass `--all-features`; without
  it, cargo skips most integration tests and still reports success.
- One JSONLogic suite (path relative to `crates/datalogic-rs/`):
  `JSONLOGIC_TEST_FILE=tests/suites/arithmetic/plus.json cargo test -p datalogic-rs --all-features --test test_jsonlogic -- --nocapture`.
  `crates/datalogic-rs/tests/README.md` documents the suite format.
- Differential testing: `crates/datalogic-rs/tests/oracle/` is an unoptimised
  reference interpreter, and `tests/oracle_test.rs` compares the engine with it under
  proptest. A rule where the engine and the oracle disagree points to an optimiser bug.
- C ABI: `cargo test --manifest-path bindings/c/Cargo.toml`. The Dockerfile built the
  three C programs in `bindings/c/examples/` against
  `bindings/c/target/debug/libdatalogic_c.so`; rebuild them with
  `make -C bindings/c/examples LIBDIR=../target/debug`.
- Fuzzing: `cd crates/datalogic-rs && cargo +nightly fuzz run eval_str -- -max_total_time=300 -rss_limit_mb=4096 -timeout=30`.
  The target feeds (templating flag, rule, data) into `Engine::eval_str` with all
  operator families compiled in, and the Dockerfile seeded its corpus from the
  conformance suites. Count a panic, abort or stack overflow as a finding, and ignore
  `Err` results.
- Reproduce a case with `datalogic_rs::Engine::new().eval_str(rule, data)`, or with
  `Engine::builder().with_templating(true).build()` for templating mode, in a test under
  `crates/datalogic-rs/tests/`.

## How you rate severity

Assume the attacker controls the rule and the content of the data, and that the host
follows the untrusted-rule checklist in `docs/src/advanced/security.md`: it compiles
with `compile_checked`, sets `ops_budget`, and bounds the size of rules and data.

- **Critical**: memory corruption (out-of-bounds write, use-after-free, double free)
  that a caller following the documented contract can reach through the C ABI or a
  binding. A rule that runs code, performs I/O, or reads or writes outside its input
  data.
- **High**: a memory-safety bug that needs an unusual but documented call sequence. A
  rule that, with small data and despite the bounds above, crashes the host process
  (stack overflow, abort, out-of-memory) in a way no binding can catch. A rule that does
  unbounded work or allocation while the host has set `ops_budget`, or exceeds another
  documented bound. An optimiser bug that changes a rule's result in a way that could
  flip an allow/deny decision.
- **Medium**: a panic that escapes the Rust API from a rule or data. The bindings catch
  it as `InternalError`, so the process survives. Compile, check or evaluation cost
  superlinear in rule size. One rule and data pair giving different results in two
  bindings.
- **Low**: results that differ from the JSONLogic spec with no security consequence.

## Anything to leave alone

- Resource use that scales with the size of the input data, and work a rule does while
  `ops_budget` is unset. `docs/src/advanced/security.md` makes both the host's
  responsibility.
- The missing wall-clock timeout and cancellation. The same document lists both as
  unbounded.
- Bugs in a host's own `CustomOperator`. A custom operator runs host code with host
  privileges by design.
- A caller that breaks the C header's contract: freeing a handle twice, reading a
  borrowed session result after the next call on that session, or passing a wrong
  length. This exclusion does not cover the Go, JVM, .NET and PHP wrappers in this
  repository; they are in scope.
- `now` returning different values between calls.

## Reports and patches

In each report, give the minimal rule and data, the engine options, cargo features or
binding involved, and the impact you observed. Add a regression test: a JSON case under
`crates/datalogic-rs/tests/suites/`, a Rust test, or for the C ABI a test in
`bindings/c/tests/`. Keep patches minimal, base them on `main`, and check that
`cargo test --workspace --all-features` passes.
