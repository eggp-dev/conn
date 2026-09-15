#![cfg(unix)]
use std::io::{Read, Write};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, CommandBuilder, PtySize};

fn read_until(rx: &mpsc::Receiver<Vec<u8>>, needle: &str, timeout: Duration) -> String {
    let mut acc = String::new();
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(chunk) = rx.recv_timeout(Duration::from_millis(50)) {
            acc.push_str(&String::from_utf8_lossy(&chunk));
            if acc.contains(needle) {
                return acc;
            }
        }
    }
    panic!("timed out waiting for {needle:?}; got: {acc:?}");
}

#[test]
fn spawn_write_read_resize_interrupt_exit() {
    let pty = native_pty_system();
    let pair = pty.openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 }).unwrap();
    let mut cmd = CommandBuilder::new("/bin/sh");
    cmd.env("PS1", "$ ");
    let mut child = pair.slave.spawn_command(cmd).unwrap();
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().unwrap();
    let mut writer = pair.master.take_writer().unwrap();

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });

    writer.write_all(b"echo hello-$((1+1))\n").unwrap();
    read_until(&rx, "hello-2", Duration::from_secs(5));

    pair.master.resize(PtySize { rows: 30, cols: 100, pixel_width: 0, pixel_height: 0 }).unwrap();
    // bash may be mid-redisplay when SIGWINCH lands; the new size must show up shortly.
    let mut seen = false;
    let mut log = String::new();
    for attempt in 0..5 {
        if attempt == 3 {
            // diagnosis: re-issue the ioctl
            pair.master.resize(PtySize { rows: 30, cols: 100, pixel_width: 0, pixel_height: 0 }).unwrap();
            log.push_str("[re-resized]\n");
        }
        writer.write_all(b"stty size\n").unwrap();
        std::thread::sleep(Duration::from_millis(100));
        let mut acc = String::new();
        while let Ok(chunk) = rx.recv_timeout(Duration::from_millis(200)) {
            acc.push_str(&String::from_utf8_lossy(&chunk));
        }
        let ms = pair.master.get_size().unwrap();
        log.push_str(&format!("attempt {attempt}: master={}x{} out={acc:?}\n", ms.rows, ms.cols));
        if acc.contains("30 100") {
            seen = true;
            break;
        }
    }
    assert!(seen, "resize never propagated to the slave\n{log}");

    writer.write_all(b"sleep 30\n").unwrap();
    std::thread::sleep(Duration::from_millis(200));
    writer.write_all(b"\x03").unwrap();
    writer.write_all(b"echo after-interrupt\n").unwrap();
    read_until(&rx, "after-interrupt", Duration::from_secs(5));

    writer.write_all(b"exit 7\n").unwrap();
    let status = child.wait().unwrap();
    assert_eq!(status.exit_code(), 7);
}
