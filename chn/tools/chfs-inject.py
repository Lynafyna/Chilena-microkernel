#!/usr/bin/env python3
"""
chfs-inject.py — Inject file ke disk.img yang sudah diformat ChilenaFS

Usage: python3 chfs-inject.py <disk.img> <file> [--name nama_di_disk]

ChilenaFS layout:
  Sektor 0      : Superblock
  Sektor 1-8    : Inode Table (64 inode, 64 bytes per inode, 8 per sektor)
  Sektor 9+     : Data blocks
"""

import sys
import struct
import argparse
import os

SECTOR_SIZE       = 512
MAGIC             = 0x43484653  # 'CHFS'
VERSION           = 1
INODE_SIZE        = 64
INODES_PER_SECTOR = 8
INODE_SECTORS     = 8
MAX_INODES        = 64
SUPERBLOCK_SECTOR = 0
INODE_TABLE_START = 1
DATA_START        = 9

INODE_FREE = 0
INODE_FILE = 1
INODE_DIR  = 2

def read_sector(disk, sector):
    disk.seek(sector * SECTOR_SIZE)
    return disk.read(SECTOR_SIZE)

def write_sector(disk, sector, data):
    assert len(data) == SECTOR_SIZE, f"Sector data must be {SECTOR_SIZE} bytes, got {len(data)}"
    disk.seek(sector * SECTOR_SIZE)
    disk.write(data)

def read_superblock(disk):
    data = read_sector(disk, SUPERBLOCK_SECTOR)
    magic, version, inode_count, data_start = struct.unpack_from('<IIII', data, 0)
    return {'magic': magic, 'version': version,
            'inode_count': inode_count, 'data_start': data_start}

def write_superblock(disk, inode_count):
    data = bytearray(SECTOR_SIZE)
    struct.pack_into('<IIII', data, 0,
        MAGIC, VERSION, inode_count, DATA_START)
    write_sector(disk, SUPERBLOCK_SECTOR, bytes(data))

def read_inode(disk, idx):
    sector = INODE_TABLE_START + (idx // INODES_PER_SECTOR)
    offset = (idx % INODES_PER_SECTOR) * INODE_SIZE
    data   = read_sector(disk, sector)
    raw    = data[offset:offset + INODE_SIZE]

    flags        = raw[0]
    name_bytes   = raw[1:49]
    name         = name_bytes.split(b'\x00')[0].decode('utf-8', errors='replace')
    size         = struct.unpack_from('<I', raw, 49)[0]
    start_sector = struct.unpack_from('<I', raw, 53)[0]
    block_count  = struct.unpack_from('<H', raw, 57)[0]

    return {'flags': flags, 'name': name, 'size': size,
            'start_sector': start_sector, 'block_count': block_count}

def write_inode(disk, idx, flags, name, size, start_sector, block_count):
    sector = INODE_TABLE_START + (idx // INODES_PER_SECTOR)
    offset = (idx % INODES_PER_SECTOR) * INODE_SIZE

    # Baca sektor yang ada dulu
    sector_data = bytearray(read_sector(disk, sector))

    # Build inode bytes
    inode = bytearray(INODE_SIZE)
    inode[0] = flags

    name_bytes = name.encode('utf-8')[:47]
    inode[1:1+len(name_bytes)] = name_bytes

    struct.pack_into('<I', inode, 49, size)
    struct.pack_into('<I', inode, 53, start_sector)
    struct.pack_into('<H', inode, 57, block_count)

    # Tulis inode ke sector data
    sector_data[offset:offset + INODE_SIZE] = inode
    write_sector(disk, sector, bytes(sector_data))

def find_free_inode(disk):
    for i in range(MAX_INODES):
        inode = read_inode(disk, i)
        if inode['flags'] == INODE_FREE:
            return i
    return None

def find_next_free_sector(disk):
    """Cari sektor data pertama yang bebas"""
    next_sec = DATA_START
    for i in range(MAX_INODES):
        inode = read_inode(disk, i)
        if inode['flags'] != INODE_FREE:
            end = inode['start_sector'] + inode['block_count']
            if end > next_sec:
                next_sec = end
    return next_sec

def inject_file(disk_path, file_path, disk_name):
    # Baca file yang mau diinject
    with open(file_path, 'rb') as f:
        file_data = f.read()

    file_size = len(file_data)
    block_count = (file_size + SECTOR_SIZE - 1) // SECTOR_SIZE

    print(f"Injecting: {file_path} -> disk:{disk_name}")
    print(f"  Size       : {file_size} bytes ({block_count} sektor)")

    with open(disk_path, 'r+b') as disk:
        # Validasi superblock
        sb = read_superblock(disk)
        if sb['magic'] != MAGIC:
            print(f"ERROR: disk belum diformat ChilenaFS (magic={sb['magic']:#X})")
            print("  Jalankan 'chfs-format' di Chilena dulu!")
            sys.exit(1)

        print(f"  Disk OK    : {sb['inode_count']} file sudah ada")

        # Cek apakah file sudah ada (overwrite)
        existing_id = None
        for i in range(MAX_INODES):
            inode = read_inode(disk, i)
            if inode['flags'] != INODE_FREE and inode['name'] == disk_name:
                existing_id = i
                print(f"  Overwrite  : inode #{i}")
                break

        if existing_id is not None:
            # Hapus inode lama
            write_inode(disk, existing_id, INODE_FREE, '', 0, 0, 0)
            new_count = max(0, sb['inode_count'] - 1)
            write_superblock(disk, new_count)
            sb['inode_count'] = new_count

        # Cari inode kosong
        inode_id = find_free_inode(disk)
        if inode_id is None:
            print("ERROR: tidak ada inode kosong (max 64 file)")
            sys.exit(1)

        # Cari sektor kosong
        start_sector = find_next_free_sector(disk)
        print(f"  Inode      : #{inode_id}")
        print(f"  Sektor     : {start_sector} - {start_sector + block_count - 1}")

        # Tulis data ke disk
        for i in range(block_count):
            sector_data = bytearray(SECTOR_SIZE)
            src_start = i * SECTOR_SIZE
            src_end   = min(src_start + SECTOR_SIZE, file_size)
            chunk     = file_data[src_start:src_end]
            sector_data[:len(chunk)] = chunk
            write_sector(disk, start_sector + i, bytes(sector_data))

        # Tulis inode
        write_inode(disk, inode_id, INODE_FILE, disk_name,
                    file_size, start_sector, block_count)

        # Update superblock
        write_superblock(disk, sb['inode_count'] + 1)

    print(f"  SELESAI    : '{disk_name}' berhasil diinject!")
    print(f"  Di Chilena : run {disk_name}")

def list_files(disk_path):
    with open(disk_path, 'rb') as disk:
        sb = read_superblock(disk)
        if sb['magic'] != MAGIC:
            print(f"ERROR: bukan ChilenaFS (magic={sb['magic']:#X})")
            sys.exit(1)

        print(f"ChilenaFS — {sb['inode_count']} file:")
        print(f"  {'ID':<4}  {'SIZE':<8}  {'SEKTOR':<8}  NAME")
        print(f"  {'-'*40}")
        for i in range(MAX_INODES):
            inode = read_inode(disk, i)
            if inode['flags'] != INODE_FREE:
                kind = 'FILE' if inode['flags'] == INODE_FILE else 'DIR'
                print(f"  {i:<4}  {inode['size']:<8}  {inode['start_sector']:<8}  {inode['name']} [{kind}]")

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description='ChilenaFS disk injector')
    sub = parser.add_subparsers(dest='cmd')

    # inject command
    p_inject = sub.add_parser('inject', help='Inject file ke disk')
    p_inject.add_argument('disk',  help='Path ke disk.img')
    p_inject.add_argument('file',  help='File yang diinject')
    p_inject.add_argument('--name', help='Nama di disk (default: basename file)')

    # list command
    p_list = sub.add_parser('list', help='List file di disk')
    p_list.add_argument('disk', help='Path ke disk.img')

    args = parser.parse_args()

    if args.cmd == 'inject':
        name = args.name or os.path.basename(args.file)
        inject_file(args.disk, args.file, name)
    elif args.cmd == 'list':
        list_files(args.disk)
    else:
        parser.print_help()
