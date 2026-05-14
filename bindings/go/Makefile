# Top-level Makefile for the datalogic Go binding.
#
# Two modes of consumption:
#
#  1. Tagged release (`go get github.com/.../bindings/go@vX.Y.Z`)
#     The release tag already has prebuilt static libraries for every
#     supported (os, arch) under bindings/go/lib/, so `go build` /
#     `go test` work without a Rust toolchain. CI produces these via
#     the matrix in .github/workflows/release.yml.
#
#  2. In-tree development
#     The static libraries are gitignored on main. `make build` runs
#     the local C ABI crate and stages the host platform's outputs
#     into bindings/go/lib/<host_os>_<host_arch>/ and the header into
#     bindings/go/include/, where the cgo build tags pick them up.
#
# Run `make build` once after cloning, and again whenever the C ABI
# surface changes. Cross-platform builds are CI's job, not the
# developer's.

# Detect host (os, arch) and map to Go's GOOS/GOARCH naming, which is
# also what we use for lib/<os>_<arch>/ subdirectories.
UNAME_S := $(shell uname -s)
UNAME_M := $(shell uname -m)

ifeq ($(UNAME_S),Darwin)
    GOOS := darwin
else ifeq ($(UNAME_S),Linux)
    GOOS := linux
else
    $(error Unsupported host OS '$(UNAME_S)' — supported: Darwin, Linux. \
            Windows contributors should build via CI or a Linux/WSL host.)
endif

ifeq ($(UNAME_M),x86_64)
    GOARCH := amd64
else ifeq ($(UNAME_M),arm64)
    GOARCH := arm64
else ifeq ($(UNAME_M),aarch64)
    GOARCH := arm64
else
    $(error Unsupported host arch '$(UNAME_M)' — supported: x86_64, arm64, aarch64)
endif

PLATFORM    := $(GOOS)_$(GOARCH)
C_DIR       := ../c
C_BUILD_DIR := $(C_DIR)/target/release
LIB_DIR     := lib/$(PLATFORM)
INC_DIR     := include
LIB_NAME    := libdatalogic_c.a
HEADER      := datalogic.h

.PHONY: build test bench clean print-platform

build: $(LIB_DIR)/$(LIB_NAME) $(INC_DIR)/$(HEADER)

# Always invoke cargo — it short-circuits if the inputs haven't changed.
# Skipping the recipe based on file timestamps would miss Rust-source-only
# changes that don't update the artifact mtime via cargo's incremental.
$(C_BUILD_DIR)/$(LIB_NAME): FORCE
	cargo build --release --manifest-path $(C_DIR)/Cargo.toml

$(LIB_DIR)/$(LIB_NAME): $(C_BUILD_DIR)/$(LIB_NAME)
	@mkdir -p $(LIB_DIR)
	cp $(C_BUILD_DIR)/$(LIB_NAME) $(LIB_DIR)/

# cargo build re-runs build.rs and regenerates the header on every
# invocation, so depending on the generated file picks up cbindgen changes.
$(INC_DIR)/$(HEADER): $(C_DIR)/include/$(HEADER)
	@mkdir -p $(INC_DIR)
	cp $(C_DIR)/include/$(HEADER) $(INC_DIR)/

FORCE:

test: build
	go test -v ./...

bench: build
	go test -bench=. -benchmem ./...

clean:
	rm -rf lib include

# Diagnostic helper — confirms which platform subdirectory `make build`
# will stage into.
print-platform:
	@echo $(PLATFORM)
