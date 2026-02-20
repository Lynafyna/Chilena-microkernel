# Chilena Kernel

Kernel x86_64 minimalis yang ditulis dalam **Rust** (`no_std`).

Terinspirasi dari filosofi desain MOROS, namun ditulis ulang dari awal
dengan arsitektur dan pendekatan yang berbeda.

---

## Arsitektur

```
src/
├── main.rs          ← entry point (boot → init → shell)
├── lib.rs           ← root library, macro global
├── sys/             ← KERNEL LAYER
│   ├── gdt.rs       ← Global Descriptor Table
│   ├── idt.rs       ← Interrupt Descriptor Table + syscall gate
│   ├── pic.rs       ← 8259 PIC
│   ├── mem/         ← Memory management
│   │   ├── bitmap.rs   ← frame allocator fisik
│   │   ├── paging.rs   ← page table x86_64
│   │   └── heap.rs     ← kernel heap (linked_list_allocator)
│   ├── process.rs   ← process table, context switch, ELF loader
│   ├── syscall/     ← syscall dispatcher + nomor + service
│   ├── fs/          ← in-memory VFS
│   ├── clk/         ← PIT timer + RTC
│   ├── console.rs   ← stdin buffer + output
│   ├── keyboard.rs  ← PS/2 driver
│   ├── serial.rs    ← UART 16550
│   ├── vga/         ← VGA text mode 80×25
│   ├── cpu.rs       ← CPUID info
│   └── acpi.rs      ← power management
├── api/             ← API LAYER (bridge kernel ↔ userspace)
│   ├── process.rs   ← ExitCode, exit()
│   ├── syscall.rs   ← syscall wrappers ergonomis
│   ├── console.rs   ← Style (warna ANSI)
│   └── io.rs        ← read/write helpers
└── usr/             ← USERSPACE LAYER
    ├── shell.rs     ← shell interaktif
    ├── help.rs      ← perintah help
    └── info.rs      ← info sistem
```

---

## Syscall

Chilena memiliki **16 syscall** yang bersih dan minimalis:

| No   | Nama    | Fungsi                        |
|------|---------|-------------------------------|
| 0x01 | EXIT    | Keluar dari proses            |
| 0x02 | SPAWN   | Buat proses baru dari ELF     |
| 0x03 | READ    | Baca dari handle              |
| 0x04 | WRITE   | Tulis ke handle               |
| 0x05 | OPEN    | Buka file/device              |
| 0x06 | CLOSE   | Tutup handle                  |
| 0x07 | STAT    | Metadata file                 |
| 0x08 | DUP     | Duplikasi handle              |
| 0x09 | REMOVE  | Hapus file                    |
| 0x0A | HALT    | Halt/reboot sistem            |
| 0x0B | SLEEP   | Tunda N detik                 |
| 0x0C | POLL    | Cek kesiapan I/O              |
| 0x0D | ALLOC   | Alokasi memori userspace      |
| 0x0E | FREE    | Bebaskan memori               |
| 0x0F | KIND    | Tipe handle                   |

Dipanggil via `int 0x80` dengan konvensi System V ABI.

---

## Build & Jalankan

### Prasyarat

- Rust nightly
- `cargo-bootimage`
- QEMU

### Install tools

```bash
rustup override set nightly
rustup component add rust-src llvm-tools-preview
cargo install bootimage
```

### Build dan run

```bash
make run
```

---

## Perintah Shell

| Perintah        | Fungsi                        |
|-----------------|-------------------------------|
| `help`          | Tampilkan daftar perintah     |
| `info`          | Info sistem (RAM, uptime, dll)|
| `echo [teks]`   | Cetak teks                    |
| `clear`         | Bersihkan layar               |
| `cd [path]`     | Ganti direktori               |
| `ls`            | Daftar file                   |
| `cat [file]`    | Tampilkan isi file            |
| `write [f] [t]` | Tulis teks ke file            |
| `reboot`        | Restart sistem                |
| `halt`          | Matikan sistem                |
| `exit`          | Keluar shell                  |

---

## Lisensi

MIT
