# Build and run the datalogic C ABI examples.
#
# Build the cdylib + header once from the repo root:
#   cargo build --release --manifest-path bindings/c/Cargo.toml
# then, from this directory:
#   make run        # build + run all three examples
#   make            # build only
#   make clean
#
# The binaries embed an rpath to ../target/release so they find
# libdatalogic_c.{so,dylib} at runtime without LD_LIBRARY_PATH. If your
# linker ignores it, run with LD_LIBRARY_PATH=../target/release (Linux) or
# DYLD_LIBRARY_PATH=../target/release (macOS).

CC       ?= cc
CFLAGS   ?= -O2 -std=c11 -Wall -Wextra
INCDIR   := ../include
LIBDIR   := ../target/release
LDFLAGS  := -L$(LIBDIR) -ldatalogic_c -Wl,-rpath,$(abspath $(LIBDIR))

EXAMPLES := getting-started custom-operator compile-once-evaluate-many

.PHONY: all run clean
all: $(EXAMPLES)

$(EXAMPLES): %: %.c
	$(CC) $(CFLAGS) -I$(INCDIR) $< $(LDFLAGS) -o $@

run: all
	@for e in $(EXAMPLES); do \
		echo "== $$e =="; \
		./$$e || exit 1; \
	done

clean:
	rm -f $(EXAMPLES)
