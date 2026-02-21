//! help — show available commands

pub fn run() {
    println!("Available commands:");
    println!("  help           — show this message");
    println!("  echo [text]    — print text");
    println!("  cd [path]      — change directory");
    println!("  info           — system information");
    println!("  ls [path]      — list files");
    println!("  cat [file]     — show file contents");
    println!("  write [f] [t]  — write text to file");
    println!("  mkdir [path]   — create directory");
    println!("  install        — setup initial filesystem");
    println!("  send <pid> <m> — send IPC message");
    println!("  recv           — receive IPC message");
    println!("  reboot         — restart the system");
    println!("  exit           — exit the shell");
}
