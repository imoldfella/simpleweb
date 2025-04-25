use std::{
    future::Future,
    ops::Deref,
    pin::Pin,
    task::{Context, Poll},
};

use env_logger::Env;

// probably want something like dashmap but with more memory control

//static mut IS_URING : bool = false;

pub struct Ptr<T> {
    ptr: *mut T,
}
impl<T> Deref for Ptr<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

pub enum ConnectionType {
    Udp,
    Tcp,
}

pub struct User {}

// we could restrict to environment, but passing the extra arguments allows them to stay in registers
type Proc = fn(
    env: Ptr<Environment>,
    db: Ptr<Db>,
    thr: Ptr<Thread>,
    cn: Ptr<Connection>,
) -> Pin<Box<dyn Future<Output = OkResult>>>;
// should we share these globally and deal with locks?
// should we compile procedures dynamically or AOT?
pub struct Iface {
    pub proc: Box<[Proc]>,
}
// capabilities are injected into the environment
pub struct Environment {
    iface: Box<[u32]>,
}

// interface = schema or no? is there a better security language we can use?
// each schema has multiple partitions; the user is authorized to access a subset of the partitions (like row level security) why not use the schema as the interface? we can always create
pub struct Connection {
    pub connection_type: ConnectionType,
    pub user: u32,

    // authorize connection, allows other rules than simply user (location, time)
    pub env: Box<[u32]>,
}

pub struct Db {
    pub user: Box<[User]>,
    pub iface: Box<[Iface]>,
    pub thread: Box<[Thread]>,
    pub env: Box<[Environment]>,
}
pub struct Thread {
    is_uring: bool,
    pub connection: Box<[Connection]>,
}

pub struct CountFuture {}

impl CountFuture {}

pub struct OkFuture {}

// uring always uses i32 error? or mostly?
type CountResult = std::result::Result<usize, i32>;
type OkResult = std::result::Result<(), i32>;

impl Future for CountFuture {
    type Output = CountResult;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}

// how should we code routines? pass thread explicitly, or use thread local storage?

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("I/O error")]
    Io(#[from] std::io::Error),

    #[error("Invalid argument")]
    InvalidArgument,
}
type DbResult<T> = std::result::Result<T, DbError>;

impl Thread {
    pub fn read_some(
        &self,
        connection: Connection,
        buf: &[u8],
    ) -> Pin<Box<dyn Future<Output = CountResult>>> {
        Box::pin(CountFuture {})
    }
    // maybe these should be on the connection? do they need the thread?
    pub fn result_error(&self, connection: Connection, streamid: u64, error: i32) {}
    pub fn result(&self, connection: Connection, streamid: u64, buf: &[u8], complete: bool) {
        // the first packet in a stream must be at least 8 bytes, (aside from the stream id in the header)
    }
}
fn read_u16(input: &[u8], range: std::ops::Range<usize>) -> Result<u16, DbError> {
    input
        .get(range)
        .and_then(|s| s.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(DbError::InvalidArgument)
}

fn read_u32(input: &[u8], range: std::ops::Range<usize>) -> Result<u32, DbError> {
    input
        .get(range)
        .and_then(|s| s.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or(DbError::InvalidArgument)
}
impl Db {
    pub fn request(
        &self,
        thread: Ptr<Thread>,
        connection: Ptr<Connection>,
        streamid: u64,
        buf: &[u8],
        complete: bool,
    ) -> DbResult<()> {
        // the first packet in a stream must be at least 8 bytes, (aside from the stream id in the header)
        // probably don't need to check here?
        if buf.len() < 8 {
            return Err(DbError::InvalidArgument);
        }
        // read 4 byte little endian environment and 4 byte little endian procid
        let env = read_u16(buf, 0..2)?;
        let iface = read_u16(buf, 2..4)?;
        let procid = read_u32(buf, 4..8)?;

        let env = connection
            .env
            .get(env as usize)
            .ok_or(DbError::InvalidArgument)?;
        let env = self
            .env
            .get(*env as usize)
            .ok_or(DbError::InvalidArgument)?;

        let interface = env
            .iface
            .get(iface as usize)
            .ok_or(DbError::InvalidArgument)?;
        let interface = self
            .iface
            .get(*interface as usize)
            .ok_or(DbError::InvalidArgument)?;

        let proc = interface
            .proc
            .get(procid as usize)
            .ok_or(DbError::InvalidArgument)?;

        let fut = proc();

        self.spawn(thread, connection, proc, buf, complete)

        // return error is schema.procid is not authorized.
    }
}

pub async fn some_fn(os: Thread, connection: Connection) -> CountResult {
    let buf = vec![0; 1024];
    let result = os.read_some(connection, &[]).await;
    result
}

// an rpc will have some number of blobs; the final blob is the parameter block.
// the initial blobs maybe stored to disk depending on memory pressure.
//
pub async fn read_rpc(os: Thread, connection: Connection) -> CountResult {
    let buf = vec![0; 1024];
    let result = os.read_some(connection, &buf).await;
    result
}

// where should buffers live?
// most connection data is going to be encrypted, but could be decrypted on the nic?
// memcpy into l1 cache is fast anyway.
// buffers in connection cost for idle connections.
// in the common case we want to convert to page aligned buffers (from net) and from these buffers to net.
// get a network buffer, copy into it, release it to the nic.
