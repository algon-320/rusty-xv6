#===============================================================================
PROFILE := debug
CARGO_FLAG := $(if $(findstring release,$(PROFILE)),--release,)
OUT_DIR := ./target/i386/$(PROFILE)

BOOTLOADER_BIN := ./target/i386/release/bootloader
BOOTLOADER_DEPS := bootloader/Cargo.toml bootloader/bootloader.ld bootloader/src/* utils/src/*

KERNEL_BIN := $(OUT_DIR)/kernel
KERNEL_DEPS := kernel/Cargo.toml kernel/kernel.ld kernel/src/* utils/src/*

IMAGE := $(OUT_DIR)/xv6.img
#===============================================================================

.PHONY: qemu
qemu: $(IMAGE)
	qemu-system-i386\
    -drive file=$(IMAGE),index=0,media=disk,format=raw\
    -smp 2 -m 512 -serial mon:stdio

$(BOOTLOADER_BIN): $(BOOTLOADER_DEPS)
	cd bootloader; cargo build --release
$(KERNEL_BIN): $(KERNEL_DEPS)
	cd kernel; cargo build $(CARGO_FLAG)

$(IMAGE):  $(BOOTLOADER_BIN) $(KERNEL_BIN)
	objcopy -O binary -j .text -j .rodata -j .bootsig $(BOOTLOADER_BIN) $(OUT_DIR)/mbr
	dd if=/dev/zero of=$(IMAGE) count=10000
	dd if=$(OUT_DIR)/mbr of=$(IMAGE) conv=notrunc
	dd if=$(KERNEL_BIN) of=$(IMAGE) seek=1 conv=notrunc

.PHONY: clean
clean:
	-rm -r target
