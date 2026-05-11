//go:build windows && arm64

package datalogic

// Built against the `aarch64-pc-windows-gnullvm` Rust target (llvm-mingw)
// so the produced libdatalogic_c.a is mingw-style ABI compatible with
// Go's cgo toolchain on Windows ARM64. There is no stable traditional
// mingw-w64 ARM64 port, so the gnullvm flavour is the only mingw option
// here — release-build-go.yml installs llvm-mingw on the windows-11-arm
// runner before building.
//
// System-lib list mirrors the amd64 file (cgo_windows_amd64.go); the set
// of Rust libstd + transitive deps is identical across Windows
// architectures.

// #cgo LDFLAGS: ${SRCDIR}/lib/windows_arm64/libdatalogic_c.a -lws2_32 -luserenv -ladvapi32 -lbcrypt -lntdll -lsynchronization -loleaut32 -lole32 -ldbghelp
import "C"
