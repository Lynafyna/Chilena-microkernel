//! Chilena Shell — interactive command interpreter

use crate::sys;
use crate::api::process::ExitCode;
use crate::usr::cl;
use alloc::string::ToString;

const PROMPT: &str = "\x1b[36mchilena\x1b[0m:\x1b[33m{cwd}\x1b[0m$ ";
const BANNER: &str = r"
  ____  _     _ _                
 / ___|| |__ (_) | ___ _ __  __ _
| |    | '_ \| | |/ _ \ '_ \/ _` |
| |___ | | | | | |  __/ | | | (_| |
 \____||_| |_|_|_|\___|_| |_|\__,_|
                                    
";

/// Run the interactive shell
pub fn run_interactive() -> Result<(), ExitCode> {
    println!("{}", BANNER);
    println!("Chilena v{} — type 'help' for commands.\n", crate::VERSION);

    loop {
        let prompt = build_prompt();
        print!("{}", prompt);

        let line = sys::console::read_line();
        let line = line.trim().to_string();

        if line.is_empty() { continue; }

        if let Err(ExitCode::Success) = exec_line(&line) {
            break;
        }
    }
    Ok(())
}

/// Run a shell script from a file
pub fn run_script(path: &str) -> Result<(), ExitCode> {
    use crate::sys::fs::FileIO;
    if let Some(mut f) = sys::fs::open_file(path) {
        let mut buf = alloc::vec![0u8; f.size()];
        if let Ok(n) = f.read(&mut buf) {
            let content = alloc::string::String::from_utf8_lossy(&buf[..n]);
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') { continue; }
                exec_line(line).ok();
            }
            Ok(())
        } else {
            Err(ExitCode::IoError)
        }
    } else {
        Err(ExitCode::NotFound)
    }
}

fn build_prompt() -> alloc::string::String {
    let cwd = sys::process::cwd();
    PROMPT.replace("{cwd}", &cwd)
}

fn exec_line(line: &str) -> Result<(), ExitCode> {
    let parts: alloc::vec::Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() { return Ok(()); }

    let cmd  = parts[0];
    let args = &parts[1..];

    match cmd {
        // basic
        "help"    => cl::basic::help::run(),
        "echo"    => cl::basic::echo::run(args),
        "cd"      => cl::basic::cd::run(args),
        "info"    => cl::basic::info::run(),

        // fs
        "ls"          => cl::fs::ls::run(args),
        "cat"         => cl::fs::cat::run(args),
        "write"       => cl::fs::write::run(args),
        "mkdir"       => cl::fs::mkdir::run(args),

        // ChilenaFS — disk-based persistent filesystem
        "chfs-format" => cl::chfs::format::run(),
        "chfs-ls"     => cl::chfs::ls::run(),
        "chfs-write"  => cl::chfs::write::run(args),
        "chfs-cat"    => cl::chfs::cat::run(args),
        "chfs-rm"     => cl::chfs::rm::run(args),

        // disk (raw VirtIO sector access)
        "disk-ping"   => cl::disk::ping::run(),
        "disk-read"   => cl::disk::read::run(args),
        "disk-write"  => cl::disk::write::run(args),

        // ipc
        "send"    => cl::ipc::send::run(args),
        "recv"    => cl::ipc::recv::run(),

        // system
        "install" => cl::system::install::run(),
        "reboot"  => cl::system::reboot::run(),

        "exit"    => return Err(ExitCode::Success),

        other => {
            println!("Unknown command: '{}'. Type 'help' for a list.", other);
        }
    }
    Ok(())
}
