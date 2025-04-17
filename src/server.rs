use std::sync::Arc;

use rustls::ServerConfig;

use crate::thread::WorkerThread;

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
                    tasks: Slab::new(),
                    cpu_socket: 0,                            // This will be set later
                    clients: slab::Slab::with_capacity(1024), // Adjust capacity as needed
                }
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();

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

    pub fn wake(&self, ptr: *const ()) {
        let index = ptr as usize;
        let thread = index >> 24;
        let index = index & 0x00FFFFFF;
    }
}
