//go:build windows && amd64

package datalogic

// Built against the `x86_64-pc-windows-gnu` Rust target so the produced
// libdatalogic_c.a is mingw-w64 ABI compatible (matches Go's default
// cgo toolchain on Windows).
//
// The system-lib list covers what Rust libstd + transitive deps resolve to:
//   ws2_32           WinSock (std::net)
//   userenv          user-profile / env lookup
//   advapi32         ACL / registry
//   bcrypt           getrandom + ring-style crypto
//   ntdll            low-level NT API used indirectly by std
//   synchronization  std::sync primitives (Rust 1.65+)
//   ole32, oleaut32  COM init pulled in by iana-time-zone's WinRT path
//   dbghelp          panic/backtrace symbolication
//   userenv covers IO + thread; kernel32 is auto-linked by mingw.

// #cgo LDFLAGS: ${SRCDIR}/lib/windows_amd64/libdatalogic_c.a -lws2_32 -luserenv -ladvapi32 -lbcrypt -lntdll -lsynchronization -loleaut32 -lole32 -ldbghelp
import "C"
