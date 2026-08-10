# Root Makefile — cargo commands that reach *every* Rust manifest in the tree.
#
# The root workspace has only two members (crates/datalogic-rs,
# tools/benchmark). Every other Rust crate — the four bindings and the fuzz
# crate — is `exclude`d from it and declares its own `[workspace]` table, so
# `cargo fmt --all`, `cargo clippy --workspace` and `cargo clean` run from
# the root silently skip them.
#
# The exclusions are deliberate, for different reasons per crate — the
# `exclude` comment in ./Cargo.toml is the authoritative list. In short:
# bindings/wasm carries a sizing profile (`opt-level = "z"`, `lto`,
# `panic = "abort"`) that cargo only honors at a workspace root and cannot
# express as a per-package override; python/c/node keep pyo3, cbindgen and
# napi codegen out of core-only builds; fuzz needs nightly + libfuzzer.
#
# So instead of one workspace, this Makefile fans each command out over all
# the manifests.

CARGO        ?= cargo
CLIPPY_FLAGS := --all-features --all-targets -- -D warnings

# Every Cargo manifest in the tree: the root workspace (which covers
# crates/datalogic-rs + tools/benchmark) plus the standalone workspace
# roots, reachable only via --manifest-path. A new Rust binding is picked up
# by the wildcard automatically — everything below derives from this list.
MANIFESTS := Cargo.toml \
             $(wildcard bindings/*/Cargo.toml) \
             crates/datalogic-rs/fuzz/Cargo.toml

# Binding lints that are a plain `cargo clippy`. wasm needs a --target and
# fuzz needs nightly, so those two keep hand-written recipes below.
CLIPPY_PLAIN   := $(filter-out clippy-wasm,\
                  $(patsubst bindings/%/Cargo.toml,clippy-%,$(wildcard bindings/*/Cargo.toml)))
CLIPPY_TARGETS := clippy-root $(CLIPPY_PLAIN) clippy-wasm clippy-fuzz

.DEFAULT_GOAL := help
.PHONY: help fmt fmt-check lint clippy $(CLIPPY_TARGETS) clean clean-all

help: ## Show this help
	@echo "datalogic-rs — repo-wide cargo targets"
	@echo ""
	@grep -hE '^[a-zA-Z_ -]+:.*?## .*$$' $(MAKEFILE_LIST) \
	  | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'

# --- formatting -------------------------------------------------------------
#
# rustfmt is purely syntactic: it never builds, never links, and needs no
# feature resolution, so every manifest takes one identical invocation and
# fmt/fmt-check share a recipe (fmt-check adds `-- --check` via FMT_ARGS).

fmt-check: FMT_ARGS := -- --check
fmt fmt-check: ## fmt: format every crate / fmt-check: check only (CI gate)
	@fail=0; for m in $(MANIFESTS); do \
	  echo "==> $@ $$m"; \
	  $(CARGO) fmt --manifest-path $$m --all $(FMT_ARGS) || fail=1; \
	done; \
	if [ "$$fail" -ne 0 ] && [ "$@" = "fmt-check" ]; then \
	  echo ""; \
	  echo "Formatting issues found. Run 'make fmt' to fix them."; \
	fi; \
	exit $$fail

# --- linting ----------------------------------------------------------------

lint: fmt-check clippy ## fmt-check + clippy over the whole tree

# The sub-make runs with `-k`, so one failing crate never hides the others —
# a lint sweep always reports every failure in one pass, no flag needed from
# the caller.
clippy: ## Clippy every Rust crate in the tree
	@$(MAKE) -k $(CLIPPY_TARGETS)

clippy-root: ## Clippy the root workspace (core + benchmark)
	@echo "==> clippy <root workspace>"
	$(CARGO) clippy --workspace $(CLIPPY_FLAGS)

# One rule serves every binding without special needs. (pyo3's
# `extension-module` leaves Python symbols undefined at link time, which
# breaks `cargo build` but not clippy — clippy stops before linking, so
# bindings/python needs nothing extra here.)
$(CLIPPY_PLAIN): clippy-%:
	@echo "==> clippy bindings/$*"
	$(CARGO) clippy --manifest-path bindings/$*/Cargo.toml $(CLIPPY_FLAGS)

# bindings/c's build.rs regenerates the *committed* include/datalogic.h via
# cbindgen on every build, and clippy runs build scripts — so a plain lint
# run dirties a tracked file that CI guards with `git diff --exit-code`. The
# crate's own opt-out keeps linting from ever mutating the working tree.
clippy-c: export DATALOGIC_C_SKIP_CBINDGEN = 1

# bindings/wasm/tests/web.rs opens with #![cfg(target_arch = "wasm32")], so
# a host-target lint compiles it to an empty file and reports success having
# checked nothing; linting for real needs the wasm32 target. A missing
# target degrades to a host lint with a warning rather than blocking someone
# who only touched core — except in CI (GitHub Actions sets `CI`), where
# silently lost coverage must fail the build instead.
clippy-wasm: ## Clippy bindings/wasm (needs wasm32-unknown-unknown for full coverage)
	@target=; \
	if rustup target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown; then \
	  target="--target wasm32-unknown-unknown"; \
	elif [ -n "$$CI" ]; then \
	  echo "ERROR: wasm32-unknown-unknown is not installed, so tests/web.rs"; \
	  echo "       cannot be linted. Install it with:"; \
	  echo "       rustup target add wasm32-unknown-unknown"; \
	  exit 1; \
	else \
	  echo "WARNING: wasm32-unknown-unknown is not installed, so tests/web.rs"; \
	  echo "         is cfg'd out and will NOT be linted."; \
	  echo "         Install it with: rustup target add wasm32-unknown-unknown"; \
	fi; \
	echo "==> clippy bindings/wasm $${target:-(host target)}"; \
	$(CARGO) clippy --manifest-path bindings/wasm/Cargo.toml $$target $(CLIPPY_FLAGS)

# The fuzz crate is #![no_main] + libfuzzer_sys::fuzz_target!, neither of
# which builds on stable — skipped (not failed) when nightly is missing.
# That includes CI: no workflow installs nightly for lint, so the SKIP there
# is deliberate. `make fmt` still covers the crate, since rustfmt doesn't
# build. Probed via `rustup toolchain list` rather than `cargo +nightly`,
# which rustup <1.28 treats as permission to auto-install a whole toolchain.
clippy-fuzz: ## Clippy the fuzz crate (requires a nightly toolchain)
	@if rustup toolchain list 2>/dev/null | grep -q '^nightly'; then \
	  echo "==> clippy crates/datalogic-rs/fuzz (+nightly)"; \
	  $(CARGO) +nightly clippy \
	    --manifest-path crates/datalogic-rs/fuzz/Cargo.toml $(CLIPPY_FLAGS); \
	else \
	  echo "==> SKIP crates/datalogic-rs/fuzz — needs nightly clippy"; \
	  echo "    Install it with: rustup toolchain install nightly"; \
	fi

# --- cleaning ---------------------------------------------------------------

clean: ## cargo clean every Rust manifest (~3 GB in a warm tree)
	@for m in $(MANIFESTS); do \
	  echo "==> clean $$m"; \
	  $(CARGO) clean --manifest-path $$m; \
	done

# Everything `clean` does, plus the generated artifacts of the non-Rust
# bindings and the UI. Kept separate because this deletes node_modules, the
# Python venv and the PHP vendor tree — recovering from it means re-running
# npm install / maturin develop / composer install, which is slow enough
# that it should never be a surprise. Every path below is build output; the
# list mirrors .gitignore's generated-output entries — update the two
# together. Go stages its own artifacts, so its Makefile owns that clean.
clean-all: clean ## clean + every non-Rust build artifact (node_modules, venv, vendor, ...)
	@echo "==> clean non-Rust artifacts"
	rm -rf bindings/wasm/pkg bindings/wasm/pkg-web \
	       bindings/wasm/pkg-bundler bindings/wasm/pkg-nodejs
	rm -rf bindings/node/node_modules bindings/node/npm \
	       bindings/node/index.js bindings/node/index.d.ts
	rm -f  bindings/node/*.node
	rm -rf bindings/python/.venv bindings/python/dist \
	       bindings/python/build bindings/python/wheels
	$(MAKE) -C bindings/go clean
	rm -rf bindings/jvm/target
	rm -rf bindings/php/vendor
	rm -rf ui/node_modules ui/dist ui/dist-embed ui/vendor
	rm -rf docs/book docs/src/wasm
	rm -f  docs/src/assets/*.js docs/src/assets/*.js.map docs/src/assets/*.css
	@find bindings/dotnet -type d \( -name bin -o -name obj \) -prune -exec rm -rf {} +
