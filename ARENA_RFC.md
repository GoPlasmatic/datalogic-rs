# RFC: Internal Arena Allocation with Stable Public API

**Status**: Draft for review
**Author**: Performance investigation, 2026-04
**Target**: datalogic-rs v4.x → v5.0 (breaking only at internal SemVer level)

## 1. Motivation

v3 hit ~7 ns/op on the compatibility suite using arena allocation throughout. v4 trades that performance for API simplicity (no lifetime parameters in user-facing APIs) and now sits at ~36 ns/op on the same suite — a 5× regression. This RFC proposes re-introducing arena allocation **internally only**, preserving v4's clean public API contract.

### Measured baseline (v4.0.21, pre-RFC)

From CPU profiling on a representative mixed workload (filter/sort heavy):

| Symbol | % of cycles |
|---|---|
| `drop_in_place<serde_json::Value>` | 33% |
| `Vec::Clone::clone` | 25% |
| `BTreeMap::Drop::drop` + `IntoIter::dying_next` | ~36% |
| `_xzm_*` (allocator) + `_platform_memset` | ~14% |

**Roughly 45–50% of cycles on this mix are heap traffic — alloc, free, or Drop.** The flamegraph and allocation counts both confirm this. See `experiment/cow-var-lookup` for the precursor investigation.

### Why the simpler "Cow borrow" attempt only partially solved it

A prototype on `experiment/cow-var-lookup` made `filter` borrow its input from the data `Arc` instead of deep-cloning. Result on the worst case: **638 ns → 393 ns (−39%), 21 → 12 allocs (−43%)**. Across the full 53-suite benchmark: 49 ns/op → 48 ns/op — within noise.

The Cow trick has a structural ceiling: it only helps when **output cardinality < input cardinality**. Sort, map, reduce produce as many (or more) values than they consume, so input clones get replaced 1:1 by output clones. Only an arena makes both sides cheap.

### Performance projections for arena (with stable API)

Based on the measured allocation breakdown:

| Workload | Current | Est. with arena | Speedup |
|---|---|---|---|
| `filter` on object array | 638 ns | ~250 ns | 2.5× |
| `sort` on object array | 575 ns | ~280 ns | 2.0× |
| `compatible.json` (mostly cheap ops) | 36 ns | 26–30 ns | 1.2–1.4× |
| All-suite avg | 49 ns | 32–38 ns | **1.3–1.5×** |

**This does not reach v3's 7 ns/op.** Closing the remaining gap requires (a) a custom packed value type (not `serde_json::Value` underneath), (b) eliminating the `Mode` generic on the hot path, and (c) reference-returning operators end-to-end. Those are out of scope for this RFC; arena alone is the first 1.5× of a 5× target.

## 2. Goals

1. **Public API unchanged.** `engine.evaluate(&compiled, data) -> Result<Value>` and the custom operator trait stay byte-for-byte identical. No new lifetime parameters visible to library users.
2. **Thread safety preserved.** `DataLogic: Send + Sync` and `CompiledLogic: Send + Sync` continue to hold. Concurrent `evaluate` calls from different threads remain safe with no new synchronization primitives in user code.
3. **Measured 1.3–1.5× speedup on the full suite, 2× on heavy ops.**
4. **No WASM/async regressions.** The current build targets continue to work without special configuration.

## 3. Non-Goals

- Reaching v3's 7 ns/op number (would require packed value type — separate RFC).
- Changing the JSON Logic semantics or adding new operators.
- Exposing arena lifetimes in the public API.
- Optimizing compile-time performance (this RFC is about evaluation hot path).

## 4. Public API Contract (preserved)

These signatures **must not change**:

```rust
// engine.rs
impl DataLogic {
    pub fn evaluate(&self, compiled: &CompiledLogic, data: Arc<Value>) -> Result<Value>;
    pub fn evaluate_owned(&self, compiled: &CompiledLogic, data: Value) -> Result<Value>;
    pub fn evaluate_json(&self, logic: &str, data: &str) -> Result<Value>;
    pub fn evaluate_structured(&self, compiled: &CompiledLogic, data: Arc<Value>)
        -> std::result::Result<Value, StructuredError>;
}

// Custom operator trait — NO LIFETIME PARAMETER
pub trait Operator: Send + Sync {
    fn evaluate(&self, args: &[Value], context: &mut ContextStack, evaluator: &dyn Evaluator)
        -> Result<Value>;
}
```

## 5. Internal Architecture

### 5.1 The `ArenaValue` type

A lifetime-parameterized mirror of `serde_json::Value`, designed for arena allocation. **This type is `pub(crate)` — it never appears in public APIs.**

```rust
// src/arena/value.rs
pub(crate) enum ArenaValue<'a> {
    Null,
    Bool(bool),
    Number(serde_json::Number),                 // 24 B inline, no heap
    String(&'a str),                            // arena-allocated string slice
    Array(&'a [ArenaValue<'a>]),                // arena-allocated slice
    Object(&'a [(&'a str, ArenaValue<'a>)]),    // sorted key-value pairs (binary search)

    /// Borrow into the input `Arc<Value>` tree without cloning. Used by var
    /// lookups so the data stays where it is. Lifetime `'a` is constrained
    /// by the arena (which borrows the Arc clone for its lifetime).
    InputRef(&'a serde_json::Value),
}
```

Key design choices and rationale:

- **Slice instead of `Vec<ArenaValue>`**: `Vec` has heap headers and capacity. A `&[T]` is two words. The flamegraph showed `Vec::Clone` taking 25% of cycles — using slices entirely eliminates the `Vec` machinery from the internal hot path.
- **Sorted key-value slice instead of `BTreeMap`**: `BTreeMap` operations dominated the flamegraph (~36% of cycles when objects are involved). For typical JSON objects (small N), a sorted array with binary search is faster *and* uses no heap headers. Build cost: O(n log n) sort during construction.
- **`InputRef` variant**: When `var` resolves into the input data, we don't copy — we hold a reference. This is the Cow-borrow win, generalized.
- **Lifetime `'a`**: tied to the arena, scoped to a single `evaluate()` call.

### 5.2 Lifetime model

```text
evaluate() lifetime 'a
├── Bump arena (owns all arena-allocated bytes)
├── Arc<Value> input data clone (held for the call's duration)
└── ArenaValue<'a> tree (borrows from both above)
        └── returned to caller as owned Value via arena_to_value() walk
```

Every internal operator function changes shape:

```rust
// Before
fn evaluate_filter(args: &[CompiledNode], ctx: &mut ContextStack,
                   engine: &DataLogic) -> Result<Value>;

// After
fn evaluate_filter<'a>(args: &[CompiledNode], ctx: &mut ArenaContextStack<'a>,
                       engine: &DataLogic, arena: &'a Bump)
    -> Result<&'a ArenaValue<'a>>;
```

The `'a` lifetime is invasive *internally* but **never escapes**:

```rust
// Public API stays the same. Arena lives only inside this function.
pub fn evaluate(&self, compiled: &CompiledLogic, data: Arc<Value>) -> Result<Value> {
    let arena = self.acquire_arena(compiled);            // see §5.5 for sizing
    let mut ctx = ArenaContextStack::new(&arena, &data);
    let arena_result = self.evaluate_arena(&compiled.root, &mut ctx, &arena)?;
    let owned = arena_to_value(arena_result);             // single tree walk
    drop(ctx);
    self.release_arena(arena);                            // see §6 for pooling
    Ok(owned)
}
```

### 5.3 `ArenaContextStack`

Mirror of `ContextStack` but frames hold `&'a ArenaValue<'a>` instead of owned `Value`:

```rust
pub(crate) struct ArenaContextStack<'a> {
    root: &'a serde_json::Value,         // borrowed from caller's Arc
    frames: Vec<ArenaContextFrame<'a>>,  // small Vec of frames (existing pattern)
    error_path: Vec<u32>,                // unchanged
    arena: &'a Bump,                     // for any frame-local allocations
}

enum ArenaContextFrame<'a> {
    Indexed { data: &'a ArenaValue<'a>, index: usize },
    Keyed   { data: &'a ArenaValue<'a>, index: usize, key: &'a str },
    Reduce  { current: &'a ArenaValue<'a>, accumulator: &'a ArenaValue<'a> },
    Data(&'a ArenaValue<'a>),
}
```

The `Arc<Value>` from the public API is held alive (one refcount bump) for the duration of the call. `root` is a `&'a Value` borrowed from that Arc. Nothing escapes.

### 5.4 The boundary conversion

Converting `&'a ArenaValue<'a>` → owned `Value` happens once, on return:

```rust
fn arena_to_value(v: &ArenaValue<'_>) -> Value {
    match v {
        ArenaValue::Null => Value::Null,
        ArenaValue::Bool(b) => Value::Bool(*b),
        ArenaValue::Number(n) => Value::Number(n.clone()),
        ArenaValue::String(s) => Value::String((*s).to_string()),
        ArenaValue::Array(items) => Value::Array(items.iter().map(arena_to_value).collect()),
        ArenaValue::Object(pairs) => {
            let mut map = serde_json::Map::new();
            for (k, v) in *pairs {
                map.insert((*k).to_string(), arena_to_value(v));
            }
            Value::Object(map)
        }
        ArenaValue::InputRef(v) => (*v).clone(),  // deep clone at boundary only
    }
}
```

Conversion cost is **the same as today's clone cost for the result tree** — we're not adding allocations, just moving where they happen. For typical results (booleans, numbers, short strings), this is sub-100 ns.

### 5.5 Compile-time arena sizing

The user observation that "we already know the maximum size of the allocation needed" applies to the **static** part of the rule:

```rust
// During compile()
struct ArenaSizeHint {
    static_bytes: usize,   // from literals, structured-object skeletons, throw payloads
    expected_dynamic_factor: f32,  // multiplier on input data size
}

// Stored on CompiledLogic
pub struct CompiledLogic {
    root: CompiledNode,
    arena_hint: ArenaSizeHint,  // NEW
}
```

At eval time:

```rust
fn acquire_arena(&self, compiled: &CompiledLogic) -> Bump {
    let hint = compiled.arena_hint.static_bytes
        .saturating_mul(2)                 // headroom
        .max(4096);                        // first-chunk default
    Bump::with_capacity(hint)              // single allocation up front
}
```

Bump grows by doubling on overflow (it's still amortized O(1)), so the hint is just an optimization to avoid the first few chunk allocations. Data-dependent allocations (filter results, etc.) cannot be predicted statically.

### 5.6 Library choice: `bumpalo`

A survey of 31 Rust arena crates ([reference catalog](https://donsz.nl/blog/arenas/)) was conducted. After eliminating single-type arenas (we need ≥4 types: `ArenaValue`, `str`, `[ArenaValue]`, `[(&str, ArenaValue)]`), GC-based crates (overhead we don't use), and Drop-tracking crates (`ArenaValue` has no `Drop` impls), the realistic candidates are:

| Crate | MSRV | Verdict | Rationale |
|---|---|---|---|
| **`bumpalo` 3.20.x** | 1.71.1 | **Adopt** | Mature (used in wasmtime, cranelift, regex, Firefox), WASM-tested in production, low MSRV, exactly the API we need (`alloc_slice_copy`, `alloc_str`, `with_capacity`) |
| `bump-scope` 2.3.x | 1.85.1 | Skip | Nested scopes are an attractive feature but JSON Logic rules iterate over dozens not millions of items, so peak-memory wins don't materialize. MSRV jump is a real downstream cost. Less production surface than bumpalo |
| `blink-alloc` 0.4.x | unknown | Skip | Built for concurrent allocation + Drop tracking — both pure overhead for our use case |
| `bumpalo-herd` 0.1.x | inherits | **Skip** for pool — see §6.2 | Designed for rayon/scoped threads with stable thread-task pinning; **unsafe with Tokio's work-stealing scheduler** |

**Add `bumpalo = "3"` to `[dependencies]`. No other arena crate.**

A future "custom packed value type" RFC should revisit `compact_arena`'s u8/u16/u32 index design — at that point we'd have a single `ArenaValue` type and could swap 8-byte pointers for 4-byte indices, halving cache pressure on the value tree. Out of scope here.

## 6. Thread Safety Strategy

This is the highest-priority concern. Three options analyzed:

### 6.1 Per-call `Bump::new()` — **RECOMMENDED**

```rust
fn acquire_arena(&self, compiled: &CompiledLogic) -> Bump { Bump::new() /* + sizing */ }
fn release_arena(&self, _arena: Bump) { /* drop */ }
```

| Property | Status |
|---|---|
| `DataLogic: Send + Sync` | ✅ Preserved |
| Concurrent `evaluate` from N threads | ✅ Each call owns its arena |
| Async-safe (Tokio task migration) | ✅ Arena is `Send`; lives within one `evaluate` call |
| WASM-safe | ✅ No threading assumptions |
| Cost | One `malloc` per call for first arena chunk (~30 ns) |

**Trade-off**: every call pays for arena setup. Mitigated by `acquire_arena` using `Bump::with_capacity(hint)` so the first chunk is sized correctly and subsequent allocations are bump-pointer arithmetic (5 ns).

### 6.2 Thread-local arena pool — optional optimization (in-tree, not bumpalo-herd)

Implemented in-tree with ~20 lines, **not via the `bumpalo-herd` crate**:

```rust
thread_local! {
    static ARENA_POOL: RefCell<Vec<Bump>> = const { RefCell::new(Vec::new()) };
}

fn acquire_arena(&self, compiled: &CompiledLogic) -> Bump {
    ARENA_POOL.with(|pool| pool.borrow_mut().pop())
        .unwrap_or_else(|| Bump::with_capacity(compiled.arena_hint.static_bytes))
}

fn release_arena(&self, mut arena: Bump) {
    arena.reset();  // O(1), zero allocations freed
    ARENA_POOL.with(|pool| {
        if pool.borrow().len() < 4 { pool.borrow_mut().push(arena); }
    });
}
```

| Property | Status |
|---|---|
| `DataLogic: Send + Sync` | ✅ TLS doesn't affect engine type |
| Concurrent threads | ✅ Each thread has its own pool |
| Async (Tokio) — task migration | ✅ acquire/release fully within sync `evaluate()`; no `.await` between them |
| WASM | ✅ TLS works in wasm32 |
| Memory growth | TLS pools persist for thread lifetime — bounded at 4 |

**Why not `bumpalo-herd`?** The `bumpalo-herd` crate is purpose-built for `rayon` and `std::thread::scope` — execution models where each task is pinned to one thread. Tokio's work-stealing scheduler can migrate a task between calls to `Herd::get()`, breaking the per-thread allocator guarantee silently. Even though our `evaluate()` is sync and never `.await`s internally, users routinely wrap it in async tasks. The in-tree pool above is safer because the acquire→release window is fully synchronous and confined to one `evaluate()` call — there is no point at which Tokio could move us mid-arena.

**Async safety argument (for the in-tree pool)**: A Tokio task may be scheduled on thread A for one `evaluate()` and thread B for the next, but each call's TLS access happens entirely within that call's synchronous body. The arena is acquired, used, and released without yielding. Migration between calls is harmless — the next call just hits whichever thread's pool it lands on.

### 6.3 Engine-owned arena pool with `Mutex`

Rejected. Adds locking overhead; defeats the per-call performance win.

### 6.4 Decision

**Ship 6.1 (per-call `Bump`) as the default.** Add 6.2 (TLS pool) behind a `pool-arena` feature flag for high-throughput single-threaded scenarios. Document the async caveat for 6.2.

## 7. Custom Operator Bridge

The custom operator trait stays unchanged:

```rust
pub trait Operator: Send + Sync {
    fn evaluate(&self, args: &[Value], context: &mut ContextStack, evaluator: &dyn Evaluator)
        -> Result<Value>;
}
```

Internally, when a `CompiledNode::CustomOperator` is dispatched, we bridge:

```rust
fn dispatch_custom<'a>(node: &CustomOperatorData, ctx: &mut ArenaContextStack<'a>,
                       arena: &'a Bump) -> Result<&'a ArenaValue<'a>> {
    // 1. Convert each arena arg → owned Value (same as the boundary walk, scoped to args)
    let owned_args: Vec<Value> = node.args.iter()
        .map(|n| evaluate_arena_then_convert(n, ctx, arena))
        .collect::<Result<_>>()?;

    // 2. Build a borrowed `ContextStack` view over the arena context
    let mut bridge_ctx = ContextStack::from_arena(ctx);  // lifetime-erasing view

    // 3. Call the custom operator with owned values
    let result = operator.evaluate(&owned_args, &mut bridge_ctx, evaluator)?;

    // 4. Promote the returned Value into the arena
    Ok(value_to_arena(result, arena))
}
```

**Cost**: one round-trip conversion per custom operator invocation. Acceptable because:
- Custom ops are a power-user feature, not the cheap-op hot path.
- Conversion cost is bounded by argument size, which is typically small.
- Power users who care about perf can opt into a future `OperatorArena` trait (deferred to a follow-up RFC).

## 8. Implementation Plan

### Phase 1: Skeleton (3 days, no perf change)

- Add `bumpalo` dependency.
- Define `ArenaValue<'a>` and `ArenaContextStack<'a>`.
- Implement `arena_to_value` and `value_to_arena` conversions.
- Add `ArenaSizeHint` field to `CompiledLogic`, computed during compile.
- Unit tests for conversions (round-trip safety).

### Phase 2: Pilot operator (2 days)

- Wire one operator end-to-end: `filter` (highest measured win).
- Internal `evaluate_filter_arena` returns `&'a ArenaValue<'a>`.
- Bridge at the dispatch boundary so other operators still take/return owned `Value`.
- Measure on the alloc-profile harness; confirm 2–2.5× on filter-on-objects.

### Phase 3: Group migrations (5–7 days)

Migrate operators in dependency order, each as a separate PR for reviewability:

1. **Arithmetic** (`+`, `-`, `*`, `/`, `%`, `min`, `max`) — leaf operators, mostly numeric.
2. **Comparison** (`==`, `!=`, `===`, etc.) — leaf, return Bool.
3. **Logical** (`&&`, `||`, `!`).
4. **Control** (`if`, `?:`).
5. **Variable** (`var`, `val`, `exists`) — mostly returns `InputRef`; high impact.
6. **Array** (`map`, `filter`, `reduce`, `sort`, `all`, `some`, `none`).
7. **String** (`cat`, `substr`, `split`, etc.).
8. **Datetime, type, missing, throw, try, preserve**.

After each group: run full test suite (must remain 1350/1350) + benchmark suite + alloc profile.

### Phase 4: Custom operator bridge (1 day)

- Implement bridge per §7.
- Verify `examples/custom_operator.rs` still works.

### Phase 5: Tracing and structured errors (2 days)

- Trace mode currently captures intermediate `Value` snapshots — these need conversion at trace recording points.
- Structured errors carry no `Value` payloads, only metadata; should be unchanged.

### Phase 6: TLS pool feature (1 day)

- Implement §6.2 behind `pool-arena` feature flag.
- Document async caveat.

### Phase 7: Documentation and release (1 day)

- Update `docs/src/performance.md` with new numbers.
- CHANGELOG entry.
- Bump to v5.0.0 (no public breakage but internal architecture is significant; SemVer-major signals the audit-worthy change).

**Total: ~15–17 dev days.**

## 9. Risk Analysis

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Lifetime gymnastics break borrow checker | High | Medium | Phase 2 pilot proves the pattern before full migration |
| Custom operator regression | Medium | Medium | Bridge has measured overhead < 50 ns; acceptable |
| Trace mode complexity | Medium | Low | Trace is feature-gated; convert at recording sites |
| Bump arena memory bloat under load | Low | Medium | TLS pool bounded to 4 entries; per-call Bump dropped immediately |
| Async/Tokio breakage with TLS | Low | High | Default to per-call Bump; TLS opt-in only |
| Achieved speedup < projected | Medium | Medium | Phase 2 measurement gates Phase 3 commitment |
| Reviewer fatigue from large PR | High | Low | Phased PRs, each independently mergeable |

### Failure mode plan

If Phase 2 measurement shows < 1.5× on filter-on-objects, **stop**. The projection was wrong; the architectural cost isn't justified. Revert Phase 1 + 2 to a tag and document findings.

## 10. Alternatives Considered

### A. Keep status quo + Cow/Arc-borrow only

Done as `experiment/cow-var-lookup`. Results: 39% on filter-on-objects, ~0% on full suite. **Insufficient for the stated goal of approaching v3 perf.**

### B. Custom packed `DataValue` without arena

Would still need `Vec` and `Box<str>` somewhere — heap traffic doesn't go away. Rejected as insufficient on its own.

### C. Re-introduce v3's design (lifetime-parameterized public API)

Rejected: this RFC's primary constraint is preserving the v4 public API.

### D. Switch to a JIT (cranelift / LLVM)

Out of scope. Order of magnitude more effort, not motivated by measured profile.

### E. SmallVec everywhere

Already partially done (existing optimization). Marginal further gains. Doesn't address `BTreeMap`/`String` allocations which dominate.

## 11. Open Questions

1. **Should `ArenaValue::Object` use a sorted slice or a `&'a [(u32, ArenaValue<'a>)]` with interned keys?** Key interning could cut object construction further but adds compile-time complexity. Defer to Phase 3.6 measurement.

2. **Do we need a `Cow<'a>` variant in `ArenaValue` for borrowed `BTreeMap` access?** Probably not — `InputRef` covers this.

3. **Should the boundary `arena_to_value` conversion stream into a pre-sized result?** For large array results, pre-sizing the outer `Vec` saves one realloc. Trivial optimization, defer to Phase 7.

4. **Tracing mode: snapshot at `&ArenaValue` pointer or at converted `Value`?** Snapshotting the pointer requires the trace to outlive the arena — won't work. Must convert at trace points; this adds cost only when tracing is on.

5. **Should `Bump::with_capacity` be informed by the input data size as well?** A hash of `data.len()` could give a better hint than the static-only number. Worth A/B-testing in Phase 2.

6. **Future packed value type: revisit `compact_arena` indices.** When we eventually do the follow-up RFC for a custom packed `DataValue` (the second multiplier toward v3 perf), the value tree becomes a single homogeneous type. At that point `compact_arena`'s u8/u16/u32 branded indices become interesting — replacing 8-byte pointers with 4-byte indices halves cache pressure on the value tree. Out of scope for this RFC; flag for the successor.

## 12. Decision Required

To proceed, please confirm:

- [ ] The performance target (1.3–1.5× full suite, 2× heavy ops) is worth ~15 dev days of focused work.
- [ ] The thread-safety design (per-call Bump default, TLS pool opt-in) meets your requirements.
- [ ] The phased plan with a Phase 2 stop-gate is acceptable.
- [ ] v5.0.0 bump (signaling internal-arch change) is the right SemVer signal.

If any answer is "no," let's discuss before Phase 1 begins.

---

## Appendix A: Measured allocation breakdown (baseline)

From `examples/alloc_profile.rs` on `experiment/cow-var-lookup` baseline (v4.0.21):

| Case | ns/op | allocs/op | bytes/op |
|---|---|---|---|
| `const true` | 19.5 | 0 | 0 |
| `var: a` | 31.4 | 0 | 0 |
| `var: a.b.c` | 46.4 | 0 | 0 |
| `===` | 50.0 | 0 | 0 |
| `+ (2 ints)` | 64.5 | 0 | 0 |
| `+ (4 ints)` | 123.4 | 0 | 0 |
| `if/===` (true str branch) | 68.6 | 1 | 3 |
| `reduce sum, 10` | 73.7 | 1 | 320 |
| `map +1, 10` | 124.0 | 2 | 640 |
| `string concat 3` | 113.4 | 3 | 58 |
| `filter == on field, 10` | 638.6 | 21 | 6650 |
| `sort by field, 10` | 575.4 | 21 | 6650 |

## Appendix B: Flamegraph top frames

| Frame | % of cycles |
|---|---|
| `evaluate_node_with_mode` (recursive) | ~85% |
| `evaluate_compiled_var` | 29% |
| `evaluate_filter` | 23% |
| `drop_in_place<serde_json::Value>` | 33% |
| `Vec::Clone::clone` | 25% |
| `BTreeMap::Drop::drop` | 19% |
| `BTreeMap::IntoIter::dying_next` | 17% |
| `BTreeMap::Clone::clone_subtree` | 11% |
| Allocator (`_xzm_*`, `_platform_memset`) | ~14% |
