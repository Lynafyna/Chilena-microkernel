# Makefile Chilena

TARGET  := x86_64-chilena
KERNEL  := target/$(TARGET)/release/chilena
DISK    := disk.img
DISK_MB := 64

# Build kernel
build:
	cargo build --release --target $(TARGET).json \
		-Z build-std=core,alloc \
		-Z build-std-features=compiler-builtins-mem

# Buat disk image bootable
image: build
	cargo bootimage --release --target $(TARGET).json \
		-Z build-std=core,alloc \
		-Z build-std-features=compiler-builtins-mem

# Buat disk image kosong untuk VirtIO (64 MB) — hanya kalau belum ada
disk:
	@if [ ! -f $(DISK) ]; then \
		dd if=/dev/zero of=$(DISK) bs=1M count=$(DISK_MB) status=progress; \
		echo "Disk image created: $(DISK) ($(DISK_MB) MB)"; \
	else \
		echo "Disk image already exists: $(DISK)"; \
	fi

# Jalankan di QEMU tanpa VirtIO disk
run: image
	qemu-system-x86_64 \
		-drive format=raw,file=target/$(TARGET)/release/bootimage-chilena.bin \
		-serial mon:stdio \
		-m 256M \
		--no-reboot \
		-nographic

# Jalankan di QEMU dengan VirtIO disk
run-disk: image disk
	qemu-system-x86_64 \
		-drive format=raw,file=target/$(TARGET)/release/bootimage-chilena.bin \
		-drive file=$(DISK),if=virtio,format=raw \
		-serial mon:stdio \
		-m 256M \
		--no-reboot \
		-nographic

# Debug dengan monitor QEMU + VirtIO disk
debug: image
	qemu-system-x86_64 \
		-drive format=raw,file=target/$(TARGET)/release/bootimage-chilena.bin \
		-drive file=$(DISK),if=virtio,format=raw \
		-serial stdio \
		-monitor telnet:localhost:1234,server,nowait \
		-m 256M \
		--no-reboot

# Bersihkan hasil build
clean:
	cargo clean
	rm -f $(DISK)

.PHONY: build image disk run run-disk debug clean
