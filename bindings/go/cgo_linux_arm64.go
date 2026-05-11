//go:build linux && arm64

package datalogic

// #cgo LDFLAGS: ${SRCDIR}/lib/linux_arm64/libdatalogic_c.a -lm -ldl -lpthread
import "C"
