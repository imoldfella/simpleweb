use std::{
    collections::{hash_map::Entry, HashMap},
    future::Future,
    ops::{Deref, DerefMut},
    pin::Pin,
    task::{Context, Poll},
};

use env_logger::Env;
use rustls::pki_types::Der;

// probably want something like dashmap but with more memory control

//static mut IS_URING : bool = false;

pub struct Ptr<T> {
    ptr: *mut T,
}
impl<T> Copy for Ptr<T> {}
impl<T> Clone for Ptr<T> {
    fn clone(&self) -> Self {
        Self { ptr: self.ptr }
    }
}
impl<T> Ptr<T> {
    // pub fn new(ptr: *mut T) -> Self {
    //     Self { ptr }
    // }
    pub fn new(slice: &[T], index: usize) -> Result<Ptr<T>, DbError> {
        if index >= slice.len() {
            return Err(DbError::InvalidArgument);
        }
        let ptr = slice.as_ptr() as *mut T;
        Ok(Self {
            ptr: unsafe { ptr.add(index) },
        })
    }
}
impl<T> Deref for Ptr<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}
impl<T> DerefMut for Ptr<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.ptr }
    }
}

pub enum ConnectionType {
    Udp,
    Tcp,
}

pub struct User {}

// we could restrict to environment, but passing the extra arguments allows them to stay in registers

// we should probably take an arena as an argument, to allocate the pin future in it.
// we want to allocate the transaction procedure inside the memory pool of the transaction.
// where do helper tasks allocate in?
type Proc = fn(
    env: Ptr<Environment>,
    db: Ptr<Db>,
    thr: Ptr<Thread>,
    cn: Ptr<Connection>,
) -> Pin<Box<dyn Future<Output = ()>>>;
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
    pub statement: Box<[Statement]>,
    pub stream: HashMap<u64, Ptr<Statement>>,
}

pub struct Db {
    // is it plausible to have users assigned to a thread? the problem is that the user does not show up in source. we could potentially have multiple ports and then webtransport to the port that the user is assigned to. CID, but only with quic, user routing to port, but makes deployment more complex.
    pub user: Box<[User]>,
    pub iface: Box<[Iface]>,
    pub thread: Box<[Thread]>,
}

// when a statement begins, it will get a memory block with the initial packet? an async function allocates before it even begins, so we might want to move the spawn into statement, that way the spawn (with allocation) might be avoided.

pub struct Statement {
    // just a pointer to linked list of memory blocks?
    // are we going to start the procedure before we have the whole statement?
    // might need annotations for rolling back? 
}
pub struct Thread {
    is_uring: bool,
    pub connection: Box<[Connection]>,
    pub env: Box<[Environment]>,
    pub statement: Box<[Statement]>,


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
    pub fn spawn(&self, fut: Pin<Box<dyn Future<Output = ()>>>) {
        // spawn the future on the thread
        // this is a no-op for now
    }
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

pub struct NetworkBuffer {
    ptr: *mut u8,
    len: usize,
}
impl Copy for NetworkBuffer {}
impl Clone for NetworkBuffer {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr,
            len: self.len,
        }
    }
}

impl Db {
    // we don't know when we get the first packet of a stream in quic, we have to look in a map to see if we have an existing stream.
    pub fn handle_read(
        &self,
        mut thread: Ptr<Thread>,
        connection: Ptr<Connection>,
        streamid: u64,
        buf: NetworkBuffer,
        first: bool,
        last: bool,
    ) -> DbResult<()> {
        let (stmt, first) =  match thread.stream.entry(streamid) {
            Entry::Occupied(mut occ) => {
                // Already existed
                ( occ.get_mut(), false)
                
            }
            Entry::Vacant(vac) => {
                let o = Statement {};
                vac.insert(o);
                (o, true)
            }
        }
        // the first packet in a stream must be at least 8 bytes, (aside from the stream id in the header)
        // probably don't need to check here?
        if buf.len() < 8 {
            return Err(DbError::InvalidArgument);
        }
        // read 4 byte little endian environment and 4 byte little endian procid
        let env = read_u16(buf, 0..2)?;
        let iface = read_u16(buf, 2..4)?;
        let procid = read_u16(buf, 4..6)?;
        let continues = read_u16(buf, 6..8)?;
        let will_continue = continues & 1;
        // continues = 0 means that this statement is autocommit; when the stream is closed, the transaction is committed or rolled back.
        // continues = 1 means that this is the first statement of a transaction. the return value of the first statement will include a handle that allows the transaction to be continued with an additional stream.
        // note that waiting for this continuation handle is intentional; if you don't need to wait, just make a more complex statement.
        // the server will only return even handles. Set the low bit (+1) of the handle to indicate that the transaction is continuing.

        // indirect through the connection; the connection has dense vector that points to pool of envionments in the thread.
        let env = connection
            .env
            .get(env as usize)
            .ok_or(DbError::InvalidArgument)?;
        let env = Ptr::new(&thread.env, *env as usize)?;

        // .get(*env as usize)
        // .ok_or(DbError::InvalidArgument)?;

        let interface = env
            .iface
            .get(iface as usize)
            .ok_or(DbError::InvalidArgument)?;
        let interface = Ptr::new(&self.iface, *interface as usize)?;
        // let interface = self
        //     .iface
        //     .get(*interface as usize)
        //     .ok_or(DbError::InvalidArgument)?;

        let proc = interface
            .proc
            .get(procid as usize)
            .ok_or(DbError::InvalidArgument)?;
        let dbp = Ptr {
            ptr: self as *const _ as *mut _,
        };
        // we need to execute the procedure in a transaction, allocate its memory there.
        let fut = (*proc)(env, dbp, thread, connection);

        thread.spawn(fut);
        std::result::Result::Ok(())
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
