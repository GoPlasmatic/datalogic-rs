//! Build script: regenerates `include/datalogic.h` via cbindgen.
//!
//! Runs on every `cargo build`. The generated header is committed to the
//! repo so downstream consumers (Go, PHP, JVM, plain-C) don't need cbindgen
//! on their machine; the build script keeps it in sync during development.

use std::env;
use std::path::PathBuf;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let out_header = PathBuf::from(&crate_dir)
        .join("include")
        .join("datalogic.h");

    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rerun-if-changed=cbindgen.toml");
    println!("cargo:rerun-if-changed=build.rs");

    // Opt-out for downstream language packages that vendor this crate and
    // don't want a cbindgen toolchain pulled in. They consume the pre-generated
    // header from `include/` directly.
    if env::var("DATALOGIC_C_SKIP_CBINDGEN").is_ok() {
        return;
    }

    let config = cbindgen::Config::from_file(format!("{crate_dir}/cbindgen.toml"))
        .expect("cbindgen.toml must be present and valid");

    let bindings = cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(config)
        .generate()
        .expect("cbindgen failed to generate bindings");

    bindings.write_to_file(&out_header);
}
