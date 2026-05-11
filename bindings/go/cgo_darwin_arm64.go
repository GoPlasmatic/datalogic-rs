//go:build darwin && arm64

package datalogic

// #cgo LDFLAGS: ${SRCDIR}/lib/darwin_arm64/libdatalogic_c.a -framework CoreFoundation -framework Security -lm -ldl -lpthread
import "C"
