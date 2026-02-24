//! help — show available commands

pub fn run() {
    println!("Available commands:");
    println!("  help           — show this message");
    println!("  echo [text]    — print text");
    println!("  cd [path]      — change directory");
    println!("  info           — system information");
    println!("");
    println!("  [ChilenaFS — persistent disk filesystem]");
    println!("  chfs-format    — format disk (HAPUS SEMUA DATA!)");
    println!("  chfs-ls        — list semua file di disk");
    println!("  chfs-write <f> <isi> — tulis file ke disk");
    println!("  chfs-cat <f>   — baca file dari disk");
    println!("  chfs-rm <f>    — hapus file dari disk");
    println!("");
    println!("  [MemFS — RAM filesystem]");
    println!("  ls [path]      — list files");
    println!("  cat [file]     — show file contents");
    println!("  write [f] [t]  — write text to file");
    println!("  mkdir [path]   — create directory");
    println!("");
    println!("  [Disk raw access]");
    println!("  disk-ping      — cek status VirtIO disk");
    println!("  disk-read <n>  — baca sektor N dari disk");
    println!("  disk-write <n> <text> — tulis ke sektor N");
    println!("");
    println!("  [Lainnya]");
    println!("  install        — setup filesystem");
    println!("  send <pid> <m> — send IPC message");
    println!("  recv           — receive IPC message");
    println!("  reboot         — restart the system");
    println!("  exit           — exit the shell");
}
