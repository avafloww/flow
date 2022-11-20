###############################################################################
## Macros
###############################################################################
define color_header
    @tput setaf 6 2> /dev/null || true
    @printf '\n%s\n' $(1)
    @tput sgr0 2> /dev/null || true
endef

define color_progress_prefix
    @tput setaf 2 2> /dev/null || true
    @tput bold 2 2> /dev/null || true
    @printf '%12s ' $(1)
    @tput sgr0 2> /dev/null || true
endef

###############################################################################
## Build Targets
###############################################################################
# Default to qemu if no target is specified
BSP ?= qemu

ifeq ($(BSP),qemu)
	TARGET = aarch64-unknown-none-softfloat
	TARGET_SIMPLE=aarch64
	QEMU_BINARY = qemu-system-aarch64
	QEMU_MACHINE_TYPE = virt
	QEMU_ARGS = -cpu cortex-a72 -m 1024M -s -serial mon:stdio -device ramfb
	RUSTC_MISC_ARGS = -C target-cpu=cortex-a72
else
	$(call color_header, "Unknown or unspecified BSP: $(BSP)")
	exit 1
endif

QEMU_ARGS += -drive file=$(shell pwd)/deps/ovmf/ovmf-$(TARGET_SIMPLE)-padded.fd,if=pflash,format=raw,readonly=on
