#!/usr/bin/env python3
"""
chn-pack — Tool untuk membuat file .chn dari raw binary

Usage: python3 chn-pack.py <input.bin> <output.chn> [--stack-size N]

Format CHN header (32 bytes):
  [0..4]   magic        = 0x7F 'C' 'H' 'N'
  [4..6]   version      = 1
  [6..8]   flags        = 0x0001 (executable)
  [8..12]  entry_offset = 0 (entry di awal code)
  [12..16] code_size    = ukuran binary
  [16..20] data_size    = 0 (sudah embedded di code)
  [20..24] stack_size   = 65536 (64KB default)
  [24..28] min_memory   = code_size + stack_size
  [28..30] target_arch  = 0x0001 (x86_64)
  [30]     os_version   = 1
  [31]     checksum     = XOR semua 31 bytes sebelumnya
"""

import sys
import struct
import argparse

CHN_MAGIC    = b'\x7fCHN'
CHN_VERSION  = 1
CHN_FLAGS    = 0x0001  # executable
CHN_ARCH     = 0x0001  # x86_64

def pack_chn(input_path: str, output_path: str, stack_size: int = 65536):
    with open(input_path, 'rb') as f:
        code = f.read()

    code_size  = len(code)
    data_size  = 0
    min_memory = code_size + stack_size

    # Build header (31 bytes dulu, lalu hitung checksum)
    header_no_checksum = struct.pack(
        '<4sHHIIIIIHB',
        CHN_MAGIC,       # 4 bytes magic
        CHN_VERSION,     # 2 bytes version
        CHN_FLAGS,       # 2 bytes flags
        0,               # 4 bytes entry_offset (0 = awal code)
        code_size,       # 4 bytes code_size
        data_size,       # 4 bytes data_size
        stack_size,      # 4 bytes stack_size
        min_memory,      # 4 bytes min_memory
        CHN_ARCH,        # 2 bytes target_arch
        1,               # 1 byte os_version
    )
    # Total sekarang = 31 bytes
    assert len(header_no_checksum) == 31, f"Header size salah: {len(header_no_checksum)}"

    # Hitung checksum = XOR semua 31 bytes
    checksum = 0
    for b in header_no_checksum:
        checksum ^= b

    # Full header = 31 bytes + 1 byte checksum = 32 bytes
    header = header_no_checksum + bytes([checksum])
    assert len(header) == 32, f"Full header size salah: {len(header)}"

    # Tulis output
    with open(output_path, 'wb') as f:
        f.write(header)
        f.write(code)

    total_size = len(header) + len(code)
    print(f"CHN: {input_path} -> {output_path}")
    print(f"  magic      : 0x7F CHN")
    print(f"  version    : {CHN_VERSION}")
    print(f"  code_size  : {code_size} bytes")
    print(f"  stack_size : {stack_size} bytes")
    print(f"  min_memory : {min_memory} bytes")
    print(f"  checksum   : 0x{checksum:02X}")
    print(f"  total size : {total_size} bytes")

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description='CHN binary packer')
    parser.add_argument('input',  help='Input raw binary')
    parser.add_argument('output', help='Output .chn file')
    parser.add_argument('--stack-size', type=int, default=65536,
                        help='Stack size in bytes (default: 65536)')
    args = parser.parse_args()

    pack_chn(args.input, args.output, args.stack_size)
