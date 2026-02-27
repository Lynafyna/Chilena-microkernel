//! help — show available commands

pub fn run() {
    println!("Available commands:");
    println!("  help           — tampilkan pesan ini");
    println!("  echo [text]    — print teks");
    println!("  cd [path]      — pindah direktori");
    println!("  info           — informasi sistem");
    println!("");
    println!("  [ChilenaFS — persistent disk filesystem]");
    println!("  chfs-format    — format disk (HAPUS SEMUA DATA!)");
    println!("  chfs-ls        — list semua file di disk");
    println!("  chfs-write <f> <isi> — tulis file ke disk");
    println!("  chfs-cat <f>   — baca file dari disk");
    println!("  chfs-rm <f>    — hapus file dari disk");
    println!("");
    println!("  [Disk — debug]");
    println!("  disk-ping      — cek status VirtIO disk");
    println!("  disk-read <n>  — baca sektor N dari disk");
    println!("");
    println!("  [IPC]");
    println!("  send <pid> <m> — kirim pesan IPC");
    println!("  recv           — terima pesan IPC");
    println!("");
    println!("  [System]");
    println!("  run <prog.chn> — jalankan program CHN");
    println!("  reboot         — restart sistem");
    println!("  exit           — keluar shell");
}
