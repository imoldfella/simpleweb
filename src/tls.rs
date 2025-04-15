use mio::net::{TcpListener, TcpStream};
use mio::{Events, Interest, Poll, Token};
use rustls::{ServerConfig, ServerConnection, StreamOwned};

use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::future::Future;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

pub struct TlsClient {
    pub conn: ServerConnection,
    pub socket: TcpStream,
}

impl TlsClient {
    // fn new(socket: TcpStream, config: Arc<ServerConfig>) -> Self {
    //     let conn = ServerConnection::new(config).unwrap();
    //     TlsClient { conn, socket }
    // }

    pub fn new(socket: TcpStream, config: Arc<ServerConfig>) -> Self {
        let conn = ServerConnection::new(config).unwrap();
        TlsClient { conn, socket }
    }

    pub fn write_page(&mut self) -> std::io::Result<bool> {
        let resp = b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nHello, world!";
        let writer = &mut self.conn.writer();
        writer.write_all(resp)?;
        writer.flush()?;
        _ = self.conn.write_tls(&mut self.socket)?;
        Ok(false) // Close after writing
    }

    pub fn ready(&mut self) -> std::io::Result<bool> {
        // Read encrypted data into the TLS connection
        match self.conn.read_tls(&mut self.socket) {
            Ok(0) => return Ok(false), // Connection closed
            Ok(_) => {}
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(true),

            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => return Ok(true),
            Err(_) => return Ok(false),
        }

        // Process decrypted packets
        match self.conn.process_new_packets() {
            Ok(_) => {}
            Err(_) => return Ok(false),
        }

        _ = self.conn.write_tls(&mut self.socket)?;

        if self.conn.is_handshaking() {
            return Ok(true); // Wait for more data
        }

        // Read decrypted application data
        let mut buf = [0u8; 1024];
        let op = self.conn.reader().read(&mut buf);
        match op {
            Ok(_) => self.write_page(),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(true),
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => return Ok(true),
            Err(_) => Ok(false),
        }
    }
}

pub fn load_tls_config() -> Arc<ServerConfig> {
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};
    use rustls::server::ServerConfig;
    use std::io::BufReader;

    let cert_file = &mut BufReader::new(File::open("cert.pem").unwrap());
    let key_file = &mut BufReader::new(File::open("key.pem").unwrap());

    let certs: Vec<CertificateDer> = rustls_pemfile::certs(cert_file)
        .collect::<Result<_, _>>()
        .unwrap();
    let keys = rustls_pemfile::private_key(key_file).unwrap().unwrap();

    ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, keys)
        .map(Arc::new)
        .expect("bad certificate or key")
}