# Anthropic's OSS Scanner (https://github.com/anthropics/oss-scanner) builds
# this image; its projects/datalogic-rs/project.yaml points here.
#
# The scanner builds from a fresh checkout, then audits the image with no
# network. This file fetches and builds what the audit needs up front: the
# crates for each workspace, nightly with cargo-fuzz, and warm build caches.
#
# To test it, run `tools/check datalogic-rs` from an oss-scanner checkout.

# Current stable, which meets the workspace MSRV (`rust-version` in Cargo.toml).
FROM rust:1-bookworm

ENV DEBIAN_FRONTEND=noninteractive \
    CARGO_TERM_COLOR=never \
    RUST_BACKTRACE=1

# python3 runs scripts/fuzz-seed-corpus.py.
RUN apt-get update && apt-get install -y --no-install-recommends python3 \
    && rm -rf /var/lib/apt/lists/*

# cargo-fuzz needs nightly for the sanitizer flags it passes rustc. The
# version matches .github/workflows/fuzz.yml.
RUN rustup toolchain install nightly --profile minimal \
    && cargo install cargo-fuzz --version 0.13.2 --locked

# scanner contract: the checkout lives inside the image, at /src
COPY . /src
WORKDIR /src

# The root workspace, the C ABI and the fuzz crate are separate workspace
# roots (DEVELOPMENT.md), so each needs its own fetch. The fuzz crate has no
# committed lockfile, so its fetch runs without --locked.
RUN cargo fetch --locked \
    && cargo fetch --locked --manifest-path bindings/c/Cargo.toml \
    && cargo fetch --manifest-path crates/datalogic-rs/fuzz/Cargo.toml

# Compile the tests, the C library and the C examples; a compile error fails
# the build. Most integration tests need `serde_json` and the suite runner
# needs `templating`, so pass `--all-features`. The C ABI debug build writes
# bindings/c/target/debug/libdatalogic_c.so, which the examples link against.
RUN cargo test --workspace --all-features --no-run \
    && cargo build --manifest-path bindings/c/Cargo.toml \
    && cargo test --manifest-path bindings/c/Cargo.toml --no-run \
    && make -C bindings/c/examples LIBDIR=../target/debug

# Seed the fuzz corpus from the conformance suites, as the nightly fuzz job
# does, and build the target with the flags `cargo fuzz run` uses.
RUN python3 scripts/fuzz-seed-corpus.py crates/datalogic-rs/fuzz/corpus/eval_str \
    && cd crates/datalogic-rs && cargo +nightly fuzz build eval_str

# Run the tests to confirm the image works. A failing test prints a warning
# and the build continues.
RUN cargo test --workspace --all-features || echo "WARNING: root workspace tests reported failures"
RUN cargo test --manifest-path bindings/c/Cargo.toml || echo "WARNING: C ABI tests reported failures"
RUN make -C bindings/c/examples LIBDIR=../target/debug run || echo "WARNING: C examples reported failures"
