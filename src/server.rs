use std::{future::Future, net::SocketAddr, pin::Pin, sync::Arc};

use mio::{net::TcpListener, Events, Interest, Poll, Token};
use rustls::ServerConfig;
use slab::Slab;

use crate::tls::TlsClient;
const SERVER_TOKEN: Token = Token(0);

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
    cpu_socket: Vec<(usize, usize)>,
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

        let cpu_socket = vec![(0 as usize, config.threads)];
        let tls_config = crate::crypto::pki::load_tls_config();
        let o = Server {
            cpu_socket,
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
    pub fn run(&self, thread: usize) -> std::io::Result<()> {
        use std::os::fd::{AsRawFd, FromRawFd};
        use io_uring::{opcode, types, IoUring};
        let ring = IoUring::new(8);
        if ring.is_err() {
            return self.run_mio(thread);
        }
        let mut ring = ring.unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:8443")?;
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
    pub fn run(&self, thread: usize) -> std::io::Result<()> {
        self.run_mio(thread)
    }
    // spawn a task for each connection; this task will start a new task for each stream (if it's a websocket or webtransport)
    fn run_mio(&self, thread: usize) -> std::io::Result<()> {
        let me = &self.worker[thread];
        use socket2::{Domain, Socket, Type};

        let addr: SocketAddr = self.config.host.parse().unwrap();
        let socket = Socket::new(Domain::IPV4, Type::STREAM, None)?;
        //socket.set_reuse_address(true)?;
        socket.bind(&addr.into())?;
        socket.listen(128)?;
        let mut listener = TcpListener::from_std(socket.into());

        let mut poll = Poll::new()?;
        poll.registry()
            .register(&mut listener, SERVER_TOKEN, Interest::READABLE)?;

        let mut events = Events::with_capacity(2048);
        let mut clients: Slab<TlsClient> = Slab::with_capacity(1024);
        //let mut next_token = Token(SERVER_TOKEN.0 + 1);

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
                        Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => break,
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(e) => return Err(e),
                    },
                    token => {
                        println!("Token {}", token.0);
                        if let Some(mut client) = clients.get_mut(token.0) {
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
