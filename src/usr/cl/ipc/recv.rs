//! recv — receive an IPC message

pub fn run() {
    println!("recv: waiting for message...");
    let mut msg = crate::sys::ipc::Message::empty();
    let result = crate::api::syscall::recv(&mut msg);
    if result == 0 {
        let data = &msg.data[..msg.data.iter().position(|&b| b == 0).unwrap_or(64)];
        let text = alloc::string::String::from_utf8_lossy(data);
        println!("recv: message from PID {} > {}", msg.sender, text);
    } else {
        println!("recv: failed to receive message");
    }
}
