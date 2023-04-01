include ../common.mk

RUSTFLAGS = $(RUSTC_MISC_ARGS) \
	-C link-arg=--script=src/linker.ld

.PHONY: clean build all

all: build

build:
	$(call color_header, "Building init")
	@RUSTFLAGS="$(RUSTFLAGS)" cargo build --target $(TARGET_INIT)

clean:
	rm -rf target/
