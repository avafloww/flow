##    ______
##   / __/ /___ _      __
##  / /_/ / __ \ | /| / /
## / __/ / /_/ / |/ |/ /
##/_/ /_/\____/|__/|__/
##
## Flow OS Makefile
##
## Copyright (c) 2022 Ava Chaney <hello@ava.dev> and contributors

include ./common.mk

.PHONY: clean kernel init iso qemu qemu_wait qemu_dump_dtb gdb all

all: clean kernel init iso

kernel:
	@$(MAKE) -C kernel -f kernel.mk

init:
	@$(MAKE) -C init -f init.mk

target/limine:
	git clone "https://github.com/limine-bootloader/limine.git" --branch v4.x-branch-binary --depth=1 target/limine
	cd target/limine && make

iso: target/limine kernel #init
	$(call color_header, "Building ISO")
	@rm -f $(shell pwd)/target/flow.iso
	@rm -rf $(shell pwd)/target/isoroot
	@mkdir -p $(shell pwd)/target/isoroot
	cp kernel/target/$(TARGET)/debug/flow-kernel limine.cfg target/limine/limine{.sys,-cd.bin,-cd-efi.bin} target/isoroot
	xorriso -as mkisofs -b limine-cd.bin \
		-no-emul-boot -boot-load-size 4 -boot-info-table --efi-boot limine-cd-efi.bin \
		-efi-boot-part --efi-boot-image --protective-msdos-label \
		$(shell pwd)/target/isoroot -o $(shell pwd)/target/flow.iso

clean:
	$(call color_header, "Cleaning build files")
	rm -rf target/
	@$(MAKE) -C kernel -f kernel.mk clean

deps/ovmf/ovmf-aarch64-padded.fd:
	# Workaround for QEMU annoyances, without bloating the repo
	cp deps/ovmf/ovmf-aarch64.fd deps/ovmf/ovmf-aarch64-padded.fd
	truncate -s 64M deps/ovmf/ovmf-aarch64-padded.fd

ifeq ($(QEMU_MACHINE_TYPE),)

qemu:
	@$(call color_header, "QEMU is not supported for this board type")
	exit 1

qemu_wait:
	@$(call color_header, "QEMU is not supported for this board type")
	exit 1

qemu_dump_dtb:
	@$(call color_header, "QEMU is not supported for this board type")
	exit 1
else

qemu: iso deps/ovmf/ovmf-aarch64-padded.fd
	@$(call color_header, "Starting QEMU and proceeding normally with boot")
	$(QEMU_BINARY) -M $(QEMU_MACHINE_TYPE) $(QEMU_ARGS) -cdrom $(shell pwd)/target/flow.iso

qemu_wait: iso deps/ovmf/ovmf-aarch64-padded.fd
	@$(call color_header, "Starting QEMU and waiting for GDB connection before boot")
	$(QEMU_BINARY) -M $(QEMU_MACHINE_TYPE) $(QEMU_ARGS) -S -cdrom $(shell pwd)/target/flow.iso

qemu_dump_dtb: deps/ovmf/ovmf-aarch64-padded.fd
	$(QEMU_BINARY) -M $(QEMU_MACHINE_TYPE),dumpdtb=dump.dtb $(QEMU_ARGS)
	dtc -I dtb -O dts dump.dtb -o dump.dts
	@$(call color_header, "QEMU device tree dumped to dump.dts")
endif

gdb: iso
	@$(call color_header, "Starting GDB")
	gdb -iex "file kernel/target/$(TARGET)/debug/flow-kernel" -iex 'target remote localhost:1234'
