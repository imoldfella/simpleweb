use std::{future::Future, net::SocketAddr, path::Path, pin::Pin, sync::Arc};

use crate::error::Result;
use mio::{
    net::{TcpListener, UdpSocket},
    Events, Interest, Poll, Token,
};
use rustls::ServerConfig;
//use s2n_quic::provider::dc::Path;
use slab::Slab;

use crate::tls::TlsClient;
const SERVER_TOKEN: Token = Token(usize::MAX);
const UDP_TOKEN: Token = Token(usize::MAX - 1);

pub struct MyConfig {
    pub threads: usize,
    pub host: String,
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

struct Task {
    future: Pin<Box<dyn Future<Output = ()>>>,
    woken: bool, // optionally, to dedup wakeups
}

// this is shared worker state; there is more thread local state in the run functions
pub struct WorkerThread {
    // executor
    // tasks: Slab<Task>,
    // cpu_socket: usize,
}
unsafe impl Sync for WorkerThread {}
unsafe impl Send for WorkerThread {}
impl WorkerThread {}

pub struct Server {
    // one entry for each socket, lets us steal from a thread that's on the same socket.
    cores_per_socket: usize,
    config: MyConfig,
    worker: Box<[WorkerThread]>,
    tls_config: Arc<ServerConfig>,
}
static mut SERVER: *const Server = std::ptr::null();
pub fn get_server() -> &'static Server {
    unsafe { &*SERVER }
}
pub struct Supervisor {
    join_handle: Vec<std::thread::JoinHandle<()>>,
}
pub fn init_server(config: MyConfig) -> Supervisor {
    let server = Arc::new(Server::new(config).unwrap());
    let mut join_handle = Vec::with_capacity(server.worker.len());
    for id in 0..server.worker.len() {
        let server = server.clone();
        join_handle.push(std::thread::spawn(move || {
            _ = server.run_mio(id);
        }));
    }
    Supervisor {
        join_handle: join_handle,
    }
}
impl Supervisor {
    pub fn join(&mut self) {
        for handle in self.join_handle.drain(..) {
            handle.join().unwrap();
        }
    }
}

impl Server {
    pub fn get_same_cpu(&self, thread: usize) -> std::ops::Range<usize> {
        // this is a range of worker threads that share the same CPU socket
        // we can steal tasks from these threads
        let cpu_socket = thread / self.cores_per_socket;
        let start = cpu_socket * self.cores_per_socket;
        let end = start + self.cores_per_socket;
        start..end
    }
    // every read is going to return a boxed future? lots of allocations.
    pub fn read_some(
        &self,
        thread: usize,
        connection: usize,
        buf: &[u8],
    ) -> Pin<Box<dyn Future<Output = i32>>> {
        todo!()
    }
    pub fn new(config: MyConfig) -> std::io::Result<Self> {
        let worker = (0..config.threads)
            .map(|_| {
                WorkerThread {
                    // tasks: Slab::new(),
                    // cpu_socket: 0,                            // This will be set later
                    // clients: slab::Slab::with_capacity(1024), // Adjust capacity as needed
                }
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();

        let tls_config = crate::crypto::pki::load_tls_config();
        let o = Server {
            // for now assume one cpu socket.
            cores_per_socket: config.threads,
            config,
            worker,
            tls_config,
        };
        Ok(o)
    }

    pub fn wake(&self, ptr: *const ()) {
        let index = ptr as usize;
        let thread = index >> 24;
        let index = index & 0x00FFFFFF;
    }
}
impl Server {
    #[cfg(target_os = "linux")]
    pub fn run(&self, thread: usize) -> Result<()> {
        use io_uring::{opcode, types, IoUring};
        use std::os::fd::{AsRawFd, FromRawFd};
        let ring = IoUring::new(8);
        if ring.is_err() {
            return self.run_mio(thread);
        }
        let mut ring = ring.unwrap();
        let listener = std::net::TcpListener::bind(self.config.host)?;
        let fd = listener.as_raw_fd();

        listener.set_nonblocking(true)?;
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

            // this isn't useful; we need to replace this.
            ring.submit_and_wait(1)?;

            let cqe = ring.completion().next().expect("no cqe");
            if cqe.user_data() == 0x42 {
                let conn_fd = cqe.result();
                if conn_fd >= 0 {
                    let stream = unsafe { std::net::TcpStream::from_raw_fd(conn_fd) };
                    crate::linux::uring::uring_handle_tls(stream, self.tls_config.clone());
                }
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    pub fn run(&self, thread: usize) -> Result<()> {
        self.run_mio(thread)
    }
    // spawn a task for each connection; this task will start a new task for each stream (if it's a websocket or webtransport)
    fn run_mio(&self, thread: usize) -> Result<()> {
        use socket2::{Domain, Socket, Type};
        use std::io::ErrorKind::Interrupted;
        use std::io::ErrorKind::WouldBlock;
        _ = &self.worker[thread];

        // let tls = s2n_quic::provider::tls::default::Server::builder()
        //     .with_certificate(Path::new("cert.pem"), Path::new("key.pem"))?
        //     .with_key_logging()? // enables key logging
        //     .build()?;
        // let server = s2n_quic::Server::builder()
        //     .with_tls(tls)?
        //     // .with_io(Mio::builder(poll.registry(), "0.0.0.0:4433")?)?
        //     .start()?;

        let addr: SocketAddr = self.config.host.parse().unwrap();
        let socket = Socket::new(Domain::IPV4, Type::STREAM, None)?;
        //socket.set_reuse_address(true)?;
        socket.bind(&addr.into())?;
        socket.listen(128)?;
        let mut listener = TcpListener::from_std(socket.into());
        let mut udp_socket = UdpSocket::bind(addr)?;

        let mut poll = Poll::new()?;
        poll.registry()
            .register(&mut listener, SERVER_TOKEN, Interest::READABLE)?;
        poll.registry().register(
            &mut udp_socket,
            UDP_TOKEN,
            Interest::READABLE.add(Interest::WRITABLE),
        )?;

        let mut events = Events::with_capacity(2048);
        let mut clients: Slab<TlsClient> = Slab::with_capacity(1024);
        //let mut next_token = Token(SERVER_TOKEN.0 + 1);

        println!("TLS server listening on https://{}", addr);

        loop {
            match poll.poll(&mut events, None) {
                Ok(_) => {}
                Err(ref e) if e.kind() == Interrupted => continue,
                Err(e) => return Err(e.into()),
            }

            for event in events.iter() {
                println!("Got event: {:?}", event);
                match event.token() {
                    UDP_TOKEN => {
                        let mut buf = [0u8; 1500];
                        match udp_socket.recv_from(&mut buf) {
                            Ok((len, src)) => {
                                // Process incoming QUIC packet, etc.
                            }
                            Err(ref e) if e.kind() == WouldBlock => {}
                            Err(e) => return Err(e.into()),
                        }
                    }
                    SERVER_TOKEN => match listener.accept() {
                        Ok((mut stream, addr)) => {
                            println!("Accepted connection from {}", addr);
                            let entry = clients.vacant_entry();
                            let token = Token(entry.key());
                            poll.registry().register(
                                &mut stream,
                                token,
                                Interest::READABLE.add(Interest::WRITABLE),
                            )?;
                            let client = TlsClient::new(stream, self.tls_config.clone());
                            entry.insert(client);
                        }
                        Err(ref e) if e.kind() == Interrupted => break,
                        Err(ref e) if e.kind() == WouldBlock => break,
                        Err(e) => return Err(e.into()),
                    },

                    // the
                    token => {
                        println!("Token {}", token.0);
                        if let Some(client) = clients.get_mut(token.0) {
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
                            }
                        }
                    }
                }
            }
        }
    }
}
