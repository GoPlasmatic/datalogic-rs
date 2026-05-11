---
name: Bug report
about: Report incorrect evaluation, a panic, or other unexpected behaviour
title: "[bug] "
labels: bug
---

## Which package?

<!-- Tick the package(s) where the bug shows up. -->

- [ ] `datalogic-rs` (Rust crate)
- [ ] `@goplasmatic/datalogic-wasm` (WASM npm package)
- [ ] `@goplasmatic/datalogic-node` (Node native npm package)
- [ ] `@goplasmatic/datalogic-ui` (React component)
- [ ] `datalogic-py` (Python)
- [ ] `datalogic-go` (Go)
- [ ] `datalogic-bench` (benchmark harness)

## Versions

- Package + version:
- Rust toolchain (`rustc --version`) / Node version, if applicable:
- OS + architecture:

## Repro

A minimal JSONLogic rule + data + the actual / expected result. The
smaller, the better — paste both as JSON so we can drop them into a test
suite.

```json
{
  "rule": { "...": "..." },
  "data": { "...": "..." }
}
```

**Expected:** <!-- what you thought the engine would return -->

**Actual:** <!-- what it actually returned, or the panic / error message -->

## Anything else

<!-- Workarounds tried, related issues, links to your code, etc. -->
