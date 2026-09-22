//! Portable native test peer. Not packaged in any desktop runtime.
use std::{
    io::{Read, Write},
    path::Path,
    time::Duration,
};
#[cfg(unix)]
unsafe extern "C" fn exit_seven(_: i32) {
    unsafe {
        libc::_exit(7);
    }
}
fn send(lane: u8, value: serde_json::Value) {
    let mut data = serde_json::to_vec(&value).unwrap();
    data.push(b'\n');
    let mut output = std::io::stdout().lock();
    output.write_all(b"RSS\x01").unwrap();
    output.write_all(&[lane]).unwrap();
    output
        .write_all(&(data.len() as u32).to_be_bytes())
        .unwrap();
    output.write_all(&data).unwrap();
    output.flush().unwrap();
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("absent") {
        std::thread::sleep(Duration::from_secs(30));
        return;
    }
    if args.first().map(String::as_str) == Some("--descendant") {
        #[cfg(unix)]
        unsafe {
            libc::signal(libc::SIGTERM, libc::SIG_IGN);
        }
        std::fs::write(
            Path::new(&args[1]).join("descendant.pid"),
            std::process::id().to_string(),
        )
        .unwrap();
        std::thread::sleep(Duration::from_secs(3));
        return;
    }
    let mode = std::fs::read_to_string(&args[0]).unwrap();
    let root = Path::new(&args[1]).parent().unwrap();
    std::fs::write(root.join("fixture.pid"), std::process::id().to_string()).unwrap();
    if let Some(code) = mode.strip_prefix("diagnostic_") {
        eprintln!(
            "{}",
            serde_json::json!({"schemaVersion":5,"kind":"hostProcessDiagnostic","code":code})
        );
        std::process::exit(1);
    }
    if mode == "exit" {
        std::process::exit(7);
    }
    #[cfg(unix)]
    unsafe {
        if mode == "ignore_term" {
            libc::signal(libc::SIGTERM, libc::SIG_IGN);
        }
        if mode == "nonzero_term" {
            libc::signal(libc::SIGTERM, exit_seven as *const () as usize);
        }
    }
    let mut descendant = if mode == "leak" {
        Some(
            std::process::Command::new(std::env::current_exe().unwrap())
                .arg("--descendant")
                .arg(root)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        )
    } else {
        None
    };
    if mode == "leak" {
        while !root.join("descendant.pid").exists() {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    send(
        1,
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","clientInfo":{"name":"fixture","version":"1"},"capabilities":{}}}),
    );
    send(
        1,
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    );
    let mut input = std::io::stdin().lock();
    let mut buffer = Vec::new();
    loop {
        let mut header = [0u8; 9];
        if input.read_exact(&mut header).is_err() {
            break;
        }
        assert_eq!(&header[..4], b"RSS\x01");
        let size = u32::from_be_bytes(header[5..].try_into().unwrap()) as usize;
        assert!(size <= 524288);
        let mut data = vec![0; size];
        input.read_exact(&mut data).unwrap();
        if header[4] != 0 {
            continue;
        }
        buffer.extend_from_slice(&data);
        while let Some(end) = buffer.iter().position(|b| *b == b'\n') {
            let value: serde_json::Value = serde_json::from_slice(&buffer[..end]).unwrap();
            buffer.drain(..=end);
            if value["kind"] != "nativeCall" {
                continue;
            }
            let health = value["method"] == "health";
            if health && mode == "no_health" {
                continue;
            }
            if health && mode == "delayed" {
                std::thread::sleep(Duration::from_millis(400));
            }
            let reply = if health {
                serde_json::json!({"schemaVersion":5,"kind":"hostHealth","ready":true,"protocol":if mode=="bad_health"{0}else{3}})
            } else {
                serde_json::json!(true)
            };
            send(
                0,
                serde_json::json!({"schemaVersion":5,"kind":"nativeReply","id":value["id"],"ok":true,"value":reply}),
            );
        }
    }
    if mode == "ignore_term" {
        std::thread::sleep(Duration::from_secs(60));
    }
    if mode == "nonzero_term" {
        std::process::exit(7);
    }
    // The leak scenario intentionally lets an owned descendant outlive this peer.
    // All other fixtures reap their child locally.
    if mode != "leak" {
        if let Some(child) = descendant.as_mut() {
            let _ = child.wait();
        }
    }
}
