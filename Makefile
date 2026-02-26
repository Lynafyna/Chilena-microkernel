# Makefile Chilena

TARGET  := x86_64-chilena
KERNEL  := target/$(TARGET)/release/chilena
DISK    := disk.img
DISK_MB := 64

# CHN programs
CHN_DIR     := chn
CHN_TARGET  := x86_64-unknown-none
CHN_OUT     := $(CHN_DIR)/out

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

# Build semua program CHN
build-chn:
	@mkdir -p $(CHN_OUT)
	@echo "=== Building CHN programs ==="
	@cd $(CHN_DIR)/programs/hello && \
		cargo build --release --target x86_64-unknown-none \
			-Z build-std=core \
			-Z build-std-features=compiler-builtins-mem 2>&1
	@# Konversi ke flat binary pakai objcopy
	@rust-objcopy --strip-all -O binary \
		$(CHN_DIR)/programs/hello/target/x86_64-unknown-none/release/hello \
		$(CHN_OUT)/hello.raw 2>/dev/null || \
	llvm-objcopy --strip-all -O binary \
		$(CHN_DIR)/programs/hello/target/x86_64-unknown-none/release/hello \
		$(CHN_OUT)/hello.raw 2>/dev/null || \
	objcopy --strip-all -O binary \
		$(CHN_DIR)/programs/hello/target/x86_64-unknown-none/release/hello \
		$(CHN_OUT)/hello.raw
	@echo "Binary info:"
	@wc -c $(CHN_OUT)/hello.raw
	@python3 $(CHN_DIR)/tools/chn-pack.py \
		$(CHN_OUT)/hello.raw $(CHN_OUT)/hello.chn
	@echo "=== CHN programs built ==="
	@ls -la $(CHN_OUT)/

# Inject program CHN ke disk.img via loop mount
inject-chn: build-chn disk
	@echo "=== Injecting CHN programs ke disk ==="
	@echo "TODO: gunakan chfs-write dari dalam Chilena untuk upload"
	@echo "Untuk sekarang: jalankan make run-disk, lalu:"
	@echo "  (dari shell Chilena) chfs-write hello.chn <isi dari file>"
	@echo ""
	@echo "File .chn tersedia di: $(CHN_OUT)/"
	@ls -la $(CHN_OUT)/*.chn 2>/dev/null || echo "(tidak ada .chn)"

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
	rm -rf $(CHN_OUT)

.PHONY: build image disk run run-disk debug clean build-chn inject-chn
