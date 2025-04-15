use mio::net::{TcpListener, TcpStream};
use mio::{Events, Interest, Poll, Token};
use rustls::{ServerConfig, ServerConnection, StreamOwned};

use simpleweb::tls::{load_tls_config, TlsClient};
use slab::Slab;
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::future::Future;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

use std::task::{Context, RawWaker, RawWakerVTable, Waker};


const SERVER_TOKEN: Token = Token(0);

// each worker thread has its own executor. No stealing/helping.

struct Task {
    future: Pin<Box<dyn Future<Output = ()>>>,
    woken: bool, // optionally, to dedup wakeups
}

pub struct WorkerThread {
    // executor
    tasks: Slab<Task>,
    cpu_socket: usize,
    clients: slab::Slab<TlsClient>,
}
unsafe impl Sync for WorkerThread {}
unsafe impl Send for WorkerThread {}
impl WorkerThread {


    pub fn spawn<F: Future<Output = ()> + 'static>(&mut self, fut: F) {
        self.tasks.insert(Task{
            woken: false,
            future: Box::pin(fut)
        });
    }

    pub fn run(&mut self) {
        let waker = dummy_waker();
        let mut cx = Context::from_waker(&waker);

        while let Some(mut task) = self.tasks.try_remove(1) {
            match task.future.poll(&mut cx) {
                std::task::Poll::Pending => self.tasks.insert(task),
                std::task::Poll::Ready(()) => {}
            }
        }
    }
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
            _ = server.run_thread(id);
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
    pub fn new(config: MyConfig) -> std::io::Result<Self> {
        let worker = (0..config.threads).map(
        |_| {
            WorkerThread { 
                tasks: Slab::new(),
                cpu_socket: 0, // This will be set later
                clients: slab::Slab::with_capacity(1024), // Adjust capacity as needed
            }
        }).collect::<Vec<_>>().into_boxed_slice();
        

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
            .register(&mut listener, SERVER_TOKEN, Interest::READABLE)?;

        let mut events = Events::with_capacity(128);
        let mut clients = HashMap::new();
        let mut next_token = Token(SERVER_TOKEN.0 + 1);

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

    let mut server = init_server(config);
    server.join();
}

fn dummy_waker() -> Waker {
    fn no_op(_: *const ()) {}
    fn clone(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }

    static VTABLE: RawWakerVTable =
        RawWakerVTable::new(clone, no_op, no_op, no_op);

    unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
}

use std::sync::{ Mutex};


fn make_waker(index: usize, thread: usize) -> Waker {
    unsafe fn clone(ptr: *const ()) -> RawWaker {
        let (index, exec): (usize, Arc<Mutex<SingleThreadExecutor>>) =
            (*(ptr as *const (usize, Arc<Mutex<SingleThreadExecutor>>))).clone();
        let boxed = Box::new((index, exec));
        RawWaker::new(Box::into_raw(boxed) as *const (), &VTABLE)
    }

    unsafe fn wake(ptr: *const ()) {
        let (index, exec): (usize, Arc<Mutex<SingleThreadExecutor>>) =
            *Box::from_raw(ptr as *mut (usize, Arc<Mutex<SingleThreadExecutor>>));
        exec.lock().unwrap().wake(index);
    }

    unsafe fn wake_by_ref(ptr: *const ()) {
        let (index, exec): &(usize, Arc<Mutex<SingleThreadExecutor>>) =
            &*(ptr as *const (usize, Arc<Mutex<SingleThreadExecutor>>));
        exec.lock().unwrap().wake(*index);
    }

    unsafe fn drop(ptr: *const ()) {
        drop(Box::from_raw(ptr as *mut (usize, Arc<Mutex<SingleThreadExecutor>>)));
    }

    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

    let boxed = Box::new((index, thread));
    let raw = RawWaker::new(Box::into_raw(boxed) as *const (), &VTABLE);
    unsafe { Waker::from_raw(raw) }
}