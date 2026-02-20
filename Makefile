# Makefile Chilena

TARGET  := x86_64-chilena
KERNEL  := target/$(TARGET)/release/chilena
IMAGE   := chilena.img

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

# Jalankan di QEMU
run: image
	qemu-system-x86_64 \
		-drive format=raw,file=target/$(TARGET)/release/bootimage-chilena.bin \
		-serial stdio \
		-m 256M \
		--no-reboot

# Jalankan dengan monitor QEMU
debug: image
	qemu-system-x86_64 \
		-drive format=raw,file=target/$(TARGET)/release/bootimage-chilena.bin \
		-serial stdio \
		-monitor telnet:localhost:1234,server,nowait \
		-m 256M \
		--no-reboot

# Bersihkan hasil build
clean:
	cargo clean

.PHONY: build image run debug clean
