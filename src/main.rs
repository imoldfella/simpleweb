use mio::{Events, Interest, Poll, Token};
use mio::net::{TcpListener, TcpStream};
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::sync::Arc;
use std::net::SocketAddr;

const SERVER: Token = Token(0);

struct TlsClient {
    conn: ServerConnection,
    socket: TcpStream,
}

impl TlsClient {
    fn new(socket: TcpStream, config: Arc<ServerConfig>) -> Self {
        let conn = ServerConnection::new(config).unwrap();
        TlsClient { conn, socket }
    }

    fn write_page(&mut self) -> std::io::Result<bool> {
        let resp = b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nHello, world!";
        let writer = &mut self.conn.writer();
        writer.write_all(resp)?;
        writer.flush()?; 
        _ = self.conn.write_tls(&mut self.socket)?;
        Ok(false) // Close after writing
    }

    fn ready(&mut self) -> std::io::Result<bool> {
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
        match op{
            Ok(_) =>  self.write_page(),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(true),
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => return Ok(true),
            Err(_) => Ok(false),
        }
       
    }
}

fn load_tls_config() -> Arc<ServerConfig> {
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};
    use rustls::server::ServerConfig;
    use std::io::BufReader;

    let cert_file = &mut BufReader::new(File::open("cert.pem").unwrap());
    let key_file = &mut BufReader::new(File::open("key.pem").unwrap());

    let certs: Vec<CertificateDer> = rustls_pemfile::certs(cert_file).collect::<Result<_, _>>().unwrap();
    let keys = rustls_pemfile::private_key(key_file).unwrap().unwrap();
    

    ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, keys)
        .map(Arc::new)
        .expect("bad certificate or key")
}

fn main() -> std::io::Result<()> {
    let addr: SocketAddr = "127.0.0.1:8443".parse().unwrap();
    let mut listener = TcpListener::bind(addr)?;

    let mut poll = Poll::new()?;
    poll.registry().register(&mut listener, SERVER, Interest::READABLE)?;

    let mut events = Events::with_capacity(128);
    let mut clients = HashMap::new();
    let mut next_token = Token(SERVER.0 + 1);

    let tls_config = load_tls_config();

    println!("TLS server listening on https://{}", addr);

    loop {
        match poll.poll(&mut events, None) {
            Ok(_) => {}
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }

        for event in events.iter() {
            match event.token() {
                SERVER => loop {
                    match listener.accept() {
                        Ok((mut stream, addr)) => {
                            println!("Accepted connection from {}", addr);
                            let token = next_token;
                            next_token.0 += 1;

                            poll.registry().register(&mut stream, token, Interest::READABLE)?;
                            clients.insert(token, TlsClient::new(stream, tls_config.clone()));
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => break,
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(e) => return Err(e),
                    }
                },
                token => {
                    if let Some(mut client) = clients.remove(&token) {
                        let keep = match client.ready() {
                            Ok(keep) => keep,
                            Err(_) => false,
                        };
                        if keep {
                            poll.registry().reregister(&mut client.socket, token, Interest::READABLE)?;
                            clients.insert(token, client);
                        }
                    }
                }
            }
        }
    }
}