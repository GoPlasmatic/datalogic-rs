//go:build linux && amd64

package datalogic

// Static-link libdatalogic_c.a plus the libstd system deps it pulls in
// (libm for math, libdl for dlopen used by some std crates, libpthread
// for thread primitives). The path is resolved at cgo time relative to
// the package's own directory via ${SRCDIR}.

// #cgo LDFLAGS: ${SRCDIR}/lib/linux_amd64/libdatalogic_c.a -lm -ldl -lpthread
import "C"
