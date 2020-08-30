#===============================================================================
PROFILE := release
CARGO_FLAG := $(if $(findstring release,$(PROFILE)),--release,)
OUT_DIR := ./target/i386/$(PROFILE)

BOOTLOADER_BIN := ./target/i386/release/bootloader
BOOTLOADER_DEPS := bootloader/Cargo.toml bootloader/bootloader.ld bootloader/src/* utils/src/*

KERNEL_BIN := $(OUT_DIR)/kernel
KERNEL_DEPS := kernel/Cargo.toml kernel/kernel.ld kernel/src/* utils/src/*

INITCODE := $(OUT_DIR)/init.bin
INITCODE_DEPS := user/init/Cargo.toml user/init/init.ld user/init/src/*

IMAGE := $(OUT_DIR)/xv6.img

GDB_PORT := $(shell expr `id -u` % 5000 + 25000)
GDB_EXTERN_TERM := gnome-terminal --
#===============================================================================

.PHONY: qemu
qemu: build-image ./fs.img
	qemu-system-i386\
    -drive file=$(IMAGE),index=0,media=disk,format=raw\
    -drive file=fs.img,index=1,media=disk,format=raw\
    -smp 2 -m 512 -serial mon:stdio

.PHONY: gdb
gdb: build-image ./fs.img
	qemu-system-i386\
    -drive file=$(IMAGE),index=0,media=disk,format=raw\
    -drive file=fs.img,index=1,media=disk,format=raw\
    -smp 1 -m 512 -S -gdb tcp::$(GDB_PORT) -serial mon:stdio
.PHONY: gdb-attach
gdb-attach:
	$(GDB_EXTERN_TERM) gdb $(KERNEL_BIN) -ex "target remote localhost:$(GDB_PORT)"

$(BOOTLOADER_BIN): $(BOOTLOADER_DEPS)
	cd bootloader; cargo build --release

$(KERNEL_BIN): $(KERNEL_DEPS) $(INITCODE)
	cd kernel; cargo build $(CARGO_FLAG)

$(INITCODE): $(INITCODE_DEPS)
	cd user/init; cargo build $(CARGO_FLAG)
	objcopy -O binary -j .text -j .rodata $(OUT_DIR)/init $(INITCODE)

.PHONY: build-image
build-image: $(BOOTLOADER_BIN) $(KERNEL_BIN)
	objcopy -O binary -j .text -j .rodata -j .bootsig $(BOOTLOADER_BIN) $(OUT_DIR)/mbr
	dd if=/dev/zero of=$(IMAGE) count=10000 status=none
	dd if=$(OUT_DIR)/mbr of=$(IMAGE) conv=notrunc status=none
	dd if=$(KERNEL_BIN) of=$(IMAGE) seek=1 conv=notrunc status=none

.PHONY: test
test: $(INITCODE)
	cd kernel; cargo test

.PHONY: clean
clean:
	-rm -r target
