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
impl WorkerThread {}