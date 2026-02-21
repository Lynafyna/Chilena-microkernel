//! send — send an IPC message to a process

pub fn run(args: &[&str]) {
    if args.len() < 2 {
        println!("send: usage: send <pid> <message>");
        println!("example: send 1 hello");
        return;
    }
    let pid: usize = match args[0].parse() {
        Ok(p) => p,
        Err(_) => { println!("send: pid must be a number"); return; }
    };
    let message = args[1..].join(" ");
    let result = crate::api::syscall::send(pid, 0, message.as_bytes());
    if result == usize::MAX {
        println!("send: failed to send to PID {}", pid);
    } else {
        println!("send: message sent to PID {}", pid);
    }
}
