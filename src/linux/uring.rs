use std::{io::{Read, Write}, net::{TcpListener, TcpStream}, os::fd::{AsRawFd, FromRawFd}, sync::Arc};

use io_uring::{opcode, types, IoUring};
use rustls::{ServerConfig, ServerConnection, StreamOwned};

use crate::crypto::load_tls_config;

pub fn uring_handle_tls(mut stream: TcpStream, tls_config: Arc<ServerConfig>) {
    let conn = ServerConnection::new(tls_config).unwrap();
    let mut tls = StreamOwned::new(conn, stream);

    let mut buf = [0u8; 1024];
    if let Ok(n) = tls.read(&mut buf) {
        println!("Received: {}", String::from_utf8_lossy(&buf[..n]));

        let body = b"Hello, world!";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            std::str::from_utf8(body).unwrap()
        );

        let _ = tls.write_all(response.as_bytes());
        let _ = tls.flush();
    }
}

pub fn web_hello(addr: String) -> std::io::Result<()> {
    let tls_config = load_tls_config();
    let listener = TcpListener::bind(addr)?;
    listener.set_nonblocking(true)?;

    let mut ring = IoUring::new(8)?;
    let fd = listener.as_raw_fd();

    loop {
        let accept_e =
            opcode::Accept::new(types::Fd(fd), std::ptr::null_mut(), std::ptr::null_mut())
                .build()
                .user_data(0x42);

        unsafe {
            ring.submission()
                .push(&accept_e)
                .expect("submission queue is full");
        }

        ring.submit_and_wait(1)?;

        let cqe = ring.completion().next().expect("no cqe");
        if cqe.user_data() == 0x42 {
            let conn_fd = cqe.result();
            if conn_fd >= 0 {
                let stream = unsafe { TcpStream::from_raw_fd(conn_fd) };
                uring_handle_tls(stream, tls_config.clone());
            }
        }
    }
}
