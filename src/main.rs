use mio::net::{TcpListener, TcpStream};
use mio::{Events, Interest, Poll, Token};
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::sync::Arc;

const SERVER: Token = Token(0);

#[derive(Debug, Clone)]
pub struct WorkerThread {
    cpu_socket: usize,
}
struct TlsClient {
    conn: ServerConnection,
    socket: TcpStream,
}

impl TlsClient {
    // fn new(socket: TcpStream, config: Arc<ServerConfig>) -> Self {
    //     let conn = ServerConnection::new(config).unwrap();
    //     TlsClient { conn, socket }
    // }

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
        match op {
            Ok(_) => self.write_page(),
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

pub struct MyConfig {
    threads: usize,
    host: String,
}
impl Default for MyConfig {
    fn default() -> Self {
        let cpu_count = num_cpus::get();
        MyConfig {
            threads: cpu_count,
            host: "127.0.0.1:8444".to_string(),
        }
    }
}
pub struct Server {
    // one entry for each socket, lets us steal from a thread that's on the same socket.
    cpu_socket: Vec<(usize, usize)>,
    config: MyConfig,
    worker: Box<[WorkerThread]>,
    tls_config: Arc<ServerConfig>,
}

pub struct Supervisor {
    server: Arc<Server>,
    join_handle: Vec<std::thread::JoinHandle<()>>,
}
impl Supervisor {
    pub fn new(config: MyConfig) -> Self {
        let server = Arc::new(Server::new(config).unwrap());
        let mut join_handle = Vec::with_capacity(server.worker.len());
        for id in 0..server.worker.len() {
            let server = server.clone();
            join_handle.push(std::thread::spawn(move || {
                server.run_thread(id);
            }));
        }
        Supervisor {
            server,
            join_handle: join_handle,
        }
    }
    pub fn join(&mut self) {
        for handle in self.join_handle.drain(..) {
            handle.join().unwrap();
        }
    }
}

impl Server {
    pub fn new(config: MyConfig) -> std::io::Result<Self> {
        let worker = vec![WorkerThread { cpu_socket: 0 }; config.threads].into_boxed_slice();
        let cpu_socket = vec![(0 as usize, config.threads)];
        let tls_config = load_tls_config();
        let o = Server {
            cpu_socket,
            config,
            worker,
            tls_config,
        };
        Ok(o)
    }

    fn run_thread(&self, thread: usize) -> std::io::Result<()> {
        use socket2::{Domain, Socket, Type};

        let addr: SocketAddr = self.config.host.parse().unwrap();
        let socket = Socket::new(Domain::IPV4, Type::STREAM, None)?;
        //socket.set_reuse_address(true)?;
        socket.bind(&addr.into())?;
        socket.listen(128)?;
        let mut listener = TcpListener::from_std(socket.into());

        let mut poll = Poll::new()?;
        poll.registry()
            .register(&mut listener, SERVER, Interest::READABLE)?;

        let mut events = Events::with_capacity(128);
        let mut clients = HashMap::new();
        let mut next_token = Token(SERVER.0 + 1);

        println!("TLS server listening on https://{}", addr);

        loop {
            match poll.poll(&mut events, None) {
                Ok(_) => {}
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }

            for event in events.iter() {
                println!("Got event: {:?}", event);
                match event.token() {
                    SERVER => match listener.accept() {
                        Ok((mut stream, addr)) => {
                            println!("Accepted connection from {}", addr);
                            let token = next_token;
                            next_token.0 += 1;

                            // Register after ownership is clear
                            poll.registry().register(
                                &mut stream,
                                token,
                                Interest::READABLE.add(Interest::WRITABLE),
                            )?;

                            let client = TlsClient::new(stream, self.tls_config.clone());
                            clients.insert(token, client);

                            // poll.registry()
                            //     .register(&mut stream, token, Interest::READABLE)?;
                            // poll.registry().register(
                            //     &mut stream,
                            //     token,
                            //     Interest::READABLE.add(Interest::WRITABLE),
                            // )?;
                            //clients.insert(token, client);
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => break,
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(e) => return Err(e),
                    },
                    token => {
                        println!("Token {}", token.0);
                        if let Some(mut client) = clients.remove(&token) {
                            let keep = match client.ready() {
                                Ok(keep) => keep,
                                Err(_) => false,
                            };
                            if keep {
                                let mut interest = Interest::READABLE;
                                if client.conn.wants_write() {
                                    interest = interest.add(Interest::WRITABLE);
                                }

                                poll.registry()
                                    .reregister(&mut client.socket, token, interest)?;
                                clients.insert(token, client);
                                // poll.registry().reregister(
                                //     &mut client.socket,
                                //     token,
                                //     Interest::READABLE,
                                // )?;
                                // poll.registry().register(&mut stream, token, Interest::READABLE.add(Interest::WRITABLE))?;
                                //clients.insert(token, client);
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn main() {
    let config = MyConfig {
        host: "127.0.0.1:8321".to_string(),
        threads: 1,
    };

    let mut server = Supervisor::new(config);
    server.join();
}
