## What

<!-- One or two sentences. Treat this as the commit message you'd write
     to your future self. -->

## Why

<!-- The problem this solves or the motivation. Link issues with
     `Closes #123` if applicable. -->

## How

<!-- Anything non-obvious about the approach: trade-offs, alternatives
     considered, why a smaller change wouldn't work. Skip if the diff
     speaks for itself. -->

## Breaking changes

- [ ] Yes (describe below)
- [ ] No

<!-- If yes, what breaks and what the migration path is. -->

## Test plan

<!-- How did you verify this works? -->

- [ ] `cargo fmt --all` passes
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes
- [ ] `cargo test --workspace --all-features` passes
- [ ] Added/updated JSON suite under `crates/datalogic-rs/tests/suites/` (for
      operator changes)
- [ ] Rebuilt WASM (`cd bindings/wasm && ./build.sh`) and verified UI /
      JS still work (only if you touched the WASM crate or the public
      Rust API)
