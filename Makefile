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
CHN_PROGRAMS := hello counter fibonacci sysinfo

build-chn:
	@mkdir -p $(CHN_OUT)
	@echo "=== Building CHN programs ==="
	@for prog in $(CHN_PROGRAMS); do \
		echo "--- $$prog ---"; \
		(cd $(CHN_DIR)/programs/$$prog && \
			cargo build --release --target x86_64-unknown-none \
				-Z build-std=core \
				-Z build-std-features=compiler-builtins-mem 2>&1); \
		OBJCOPY=$$(which rust-objcopy 2>/dev/null || which llvm-objcopy 2>/dev/null || which objcopy); \
		$$OBJCOPY --strip-all -O binary \
			$(CHN_DIR)/programs/$$prog/target/x86_64-unknown-none/release/$$prog \
			$(CHN_OUT)/$$prog.raw; \
		python3 $(CHN_DIR)/tools/chn-pack.py \
			$(CHN_OUT)/$$prog.raw $(CHN_OUT)/$$prog.chn; \
		echo "  OK: $(CHN_OUT)/$$prog.chn"; \
	done
	@echo "=== Selesai ==="
	@ls -la $(CHN_OUT)/*.chn

# Inject semua program CHN ke disk.img
inject-chn: build-chn disk
	@echo "=== Injecting ke disk ==="
	@for prog in $(CHN_PROGRAMS); do \
		echo "  Injecting $$prog.chn..."; \
		python3 $(CHN_DIR)/tools/chfs-inject.py inject $(DISK) $(CHN_OUT)/$$prog.chn; \
	done
	@echo "=== Inject selesai ==="

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
