//go:build darwin && amd64

package datalogic

// CoreFoundation is pulled in by `iana-time-zone` (transitive via
// `chrono`'s datetime feature); Security covers a small bit of
// `getrandom`/std on darwin. libpthread/libm/libdl come from libstd.

// #cgo LDFLAGS: ${SRCDIR}/lib/darwin_amd64/libdatalogic_c.a -framework CoreFoundation -framework Security -lm -ldl -lpthread
import "C"
