##    ______
##   / __/ /___ _      __
##  / /_/ / __ \ | /| / /
## / __/ / /_/ / |/ |/ /
##/_/ /_/\____/|__/|__/
##
## Flow OS Makefile
##
## Copyright (c) 2022 Ava Chaney <hello@ava.dev> and contributors

include ../common.mk

RUSTFLAGS = $(RUSTC_MISC_ARGS) \
	-C panic=abort \
	-C link-arg=-Lsrc/arch/$(TARGET_SIMPLE)/ \
	-C link-arg=--script=src/arch/$(TARGET_SIMPLE)/kernel.ld

.PHONY: clean build all

all: build

build:
	$(call color_header, "Building kernel")
	@RUSTFLAGS="$(RUSTFLAGS)" cargo build --target $(TARGET) --features bsp_$(BSP)

clean:
	rm -rf target/
