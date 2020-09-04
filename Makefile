PROFILE := release
CARGO_FLAGS := $(if $(findstring release,$(PROFILE)),--release,)

IMAGE := out/xv6.img
FS_IMAGE := out/fs.img

QEMU_ARGS :=\
    -drive file=$(IMAGE),index=0,media=disk,format=raw\
    -drive file=$(FS_IMAGE),index=1,media=disk,format=raw\
    -smp 2 -m 512 -serial mon:stdio
GDB_PORT := $(shell expr `id -u` % 5000 + 25000)

#===============================================================================
BOOTLOADER_BIN := out/bootloader
BOOTLOADER_DEPS := bootloader/Cargo.toml bootloader/bootloader.ld bootloader/src/* utils/src/*

KERNEL_BIN := out/kernel
KERNEL_DEPS := kernel/Cargo.toml kernel/kernel.ld kernel/src/* utils/src/*

INITCODE := out/init.bin
INITCODE_DEPS := user/init/Cargo.toml user/init/init.ld user/init/src/*

MKFS := out/mkfs
MKFS_DEPS := mkfs/Cargo.toml mkfs/src/*

BUILD_DIR := ./target/i386/$(PROFILE)

.PHONY: qemu
qemu: build-image build-fs
	qemu-system-i386 $(QEMU_ARGS)

.PHONY: gdb
gdb: build-image build-fs
	qemu-system-i386 $(QEMU_ARGS) -S -gdb tcp::$(GDB_PORT)

.PHONY: gdb-attach
gdb-attach:
	gdb $(KERNEL_BIN) -ex "target remote localhost:$(GDB_PORT)"

.PHONY: build-image
build-image: ./out $(BOOTLOADER_BIN) $(KERNEL_BIN)
	objcopy -O binary -j .text -j .rodata -j .bootsig $(BOOTLOADER_BIN) out/mbr
	dd if=/dev/zero of=$(IMAGE) count=10000 status=none
	dd if=out/mbr of=$(IMAGE) conv=notrunc status=none
	dd if=$(KERNEL_BIN) of=$(IMAGE) seek=1 conv=notrunc status=none

.PHONY: build-fs
build-fs: ./out $(MKFS)
	$(MKFS) $(FS_IMAGE)

.PHONY: test
test: ./out $(INITCODE)
	cd kernel; cargo test

.PHONY: clean
clean:
	-rm -r target
	-rm -r out

out:
	mkdir out

$(BOOTLOADER_BIN): $(BOOTLOADER_DEPS)
	cd bootloader; cargo build --release
	cp ./target/i386/release/bootloader $(BOOTLOADER_BIN)

$(KERNEL_BIN): $(KERNEL_DEPS) $(INITCODE)
	cd kernel; cargo build $(CARGO_FLAGS)
	cp $(BUILD_DIR)/kernel $(KERNEL_BIN)

$(INITCODE): $(INITCODE_DEPS)
	cd user/init; cargo build $(CARGO_FLAGS)
	objcopy -O binary -j .text -j .rodata $(BUILD_DIR)/init $(INITCODE)

$(MKFS): $(MKFS_DEPS)
	cd mkfs; cargo build --release
	cp ./target/release/mkfs $(MKFS)
