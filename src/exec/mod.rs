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

// each procedure has n blobs, and up to nnK bytes of inline data.
// the blobs may be written to disk (for some procedures we might actually want this as early as possible? can that be a hint? seperate count for eager or lazy writing of blobs?)
// is it too limiting to force the parameter block to be reified before the procedure starts? In general this is a good thing, since it allows us to build it outside a transaction. We can retry the transaction if it fails without rebuilding the parameter block.

pub struct Blob {}
pub struct ParameterBlock {
    pub blob: Box<[Blob]>,
    pub inline: Box<[u8]>,
}

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

pub struct Vecb<T> {
    pub vec: Box<[T]>,
}
impl<T> Vecb<T> {
    pub fn get(&self, index: usize) -> Result<Ptr<T>, DbError> {
        if index >= self.vec.len() {
            return Err(DbError::InvalidArgument);
        }
        let ptr = self.vec.as_ptr() as *mut T;
        Ok(Ptr {
            ptr: unsafe { ptr.add(index) },
        })
    }
}
// we should probably take an arena as an argument, to allocate the pin future in it.
// we want to allocate the transaction procedure inside the memory pool of the transaction.
// where do helper tasks allocate in?

// either returns a result or spawns a task that returns a result.
type Proc = fn(
    db: Ptr<Db>,
    thr: Ptr<DbThread>,
    cn: Ptr<Connection>,
    pm: &ParameterBlock, // reference or pointer? the proc might need it move it into the closure.
    streamid: u64, // we need this to return, but we have already looked in the connection map and know that this does not exist.
                   // buf: &[u8],
                   // fin: bool,
);
// should we share these globally and deal with locks?
// should we compile procedures dynamically or AOT?
type Iface = Vecb<Proc>;
// capabilities are injected into the environment

// interface = schema or no? is there a better security language we can use?
// each schema has multiple partitions; the user is authorized to access a subset of the partitions (like row level security) why not use the schema as the interface? we can always create
pub struct Connection {
    pub connection_type: ConnectionType,
    pub user: u32,

    // authorize connection, allows other rules than simply user (location, time)
    pub iface: Vecb<Iface>,
    pub statement: Box<[DbStream]>,
    pub stream: HashMap<u64, DbStream>,
}

pub struct Db {
    // is it plausible to have users assigned to a thread? the problem is that the user does not show up in source. we could potentially have multiple ports and then webtransport to the port that the user is assigned to. CID, but only with quic, user routing to port, but makes deployment more complex.
    pub user: Box<[User]>,
    pub iface: Box<[Iface]>,
    pub thread: Box<[DbThread]>,
}

// when a statement begins, it will get a memory block with the initial packet? an async function allocates before it even begins, so we might want to move the spawn into statement, that way the spawn (with allocation) might be avoided.

// pub struct DbStream {
//     // just a pointer to linked list of memory blocks?
//     // are we going to start the procedure before we have the whole statement?
//     // might need annotations for rolling back?
// }
//type DbStream = Box<dyn Future<Output = ()>>;

// instead of futures, can we use something that takes our slice? but then if that future needs to block for something, how would we manage that? otoh how do we (temporarily) store the new network packet if we can't immediately feed it to the future? what if the future is not ready to accept it, eg. is still writing the previous packet?
// would it make more sense to use an intermediate layer that attempts to cache the entire stream in memory, but falls back to swapping to disk? such a layer would need to understand something about the layout of the procedure block, at least the blobs.
pub struct DbStream {}

pub struct DbThread {
    is_uring: bool,
    pub connection: Box<[Connection]>,
    pub statement: Box<[DbStream]>,
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

impl DbThread {
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
    pub fn result_error(&self, connection: Ptr<Connection>, streamid: u64, error: i32) {}
    pub fn result(&self, connection: Ptr<Connection>, streamid: u64, buf: &[u8], complete: bool) {
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

// use this for recv -> send
pub struct NetworkBuffer {
    ptr: *mut u8,
    len: usize,
}
impl NetworkBuffer {
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
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
#[repr(C)]
pub struct StreamHeader {
    env: u16,
    iface: u16,
    procid: u16,
    continues: u16,
}
impl StreamHeader {
    pub fn new(nb: &[u8]) -> Result<Self, DbError> {
        let env = read_u16(nb, 0..2)?;
        let iface = read_u16(nb, 2..4)?;
        let procid = read_u16(nb, 4..6)?;
        let continues = read_u16(nb, 6..8)?;
        Ok(Self {
            env,
            iface,
            procid,
            continues,
        })
    }
}

// quic can pack multiple frames (multiple streams) into a packet on a connection. if we model this using websockets we could send multiple rpcs in a single websocket frame.
// more to the point though, we don't control if chrome decides to send multiple websocket frames in a single tcp packet. This makes it difficult to use the same buffer to return the result.

// we might as well just take the slice then, having the network buffer does us no good. should we force a minimum size of the stream, or handle it here? we clearly are going to have at least the header before calling here,

type TryMaybeFuture = Result<Option<Box<dyn Future<Output = ()>>>, DbError>;
// handle start of a stream
// we might resolve immediately (fast path) or we might return a future
// if not fin, then we will always return a future or error.
// if fin, then we can return a future or error or Done.
// simple lookups and writes to temporary tables don't need to be async
// these create no log entries, and if cached require no io.
// we don't need to return the future? just spawn it from here?
// what about a list of varints, (streamvbyte? that is primarily 32 bit)
//

// somproc(x: LazyBlob, y: EagerBlob, z: u64)
pub fn handle_read(
    db: Ptr<Db>,
    thread: Ptr<DbThread>,
    connection: Ptr<Connection>,
    streamid: u64, // we need this to return, but we have already looked in the connection map and know that this does not exist.
    buf: &[u8],
    fin: bool,
) -> Result<(), DbError> {
    let str = connection.stream.get(&streamid);
    if let Some(str) = str {
        // we need to build the parameter block on packet at a time.
        // try to cache the the lazy blobs, aggresively write the eager blobs.

        if fin {
            // invoke the procedure.
        }
    }
    // stream does not exist, treat as a new stream
    let header = StreamHeader::new(buf)?;

    // now hopefully we can simply execute the procedure and schedule a packet to be sent back with no async needed. otherwise the procedure will spawn a task that will send the result back.

    // getting the procedure might require an async call, so in that case we will need to spawn.
    //db.exec_procedure(header)?
    // the interface must already be injected into the environment.
    let iface = connection.iface.get(header.iface as usize)?;
    let proc = iface.get(header.procid as usize)?;
    // if not finished we need the procedure to accept more packets as they arrive. it will need to spawn a task to do this. o

    proc(db, thread, connection, streamid, pb);
    Ok(())
}

// for web sockets we use messages to frame
impl Db {
    // optimizize the case where we get the entire stream in a single read.
    // we don't know when we get the first packet of a stream in quic, we have to look in a map to see if we have an existing stream.
    pub fn handle_read(
        &self,
        mut thread: Ptr<DbThread>,
        connection: Ptr<Connection>,
        //
        streamid: u64,
        first8: u64,
        first: bool,
        last: bool,
    ) -> DbResult<()> {
        let (stmt, first) = match connection.stream.entry(streamid) {
            Entry::Occupied(mut occ) => {
                // Already existed
                (occ.get_mut(), false)
            }
            Entry::Vacant(vac) => {
                // optimize for single packet.
                let o = DbStream {};
                vac.insert(o);
                (o, true)
            }
        };
        // the first packet in a stream must be at least 8 bytes, (aside from the stream id in the header)
        // probably don't need to check here?

        // read 4 byte little endian environment and 4 byte little endian procid
        let header = match StreamHeader::new(buf) {
            Ok(header) => header,
            Err(e) => {
                thread.result_error(connection, streamid, e.into());
                return;
            }
        };
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

pub async fn some_fn(os: DbThread, connection: Connection) -> CountResult {
    let buf = vec![0; 1024];
    let result = os.read_some(connection, &[]).await;
    result
}

// an rpc will have some number of blobs; the final blob is the parameter block.
// the initial blobs maybe stored to disk depending on memory pressure.
//
pub async fn read_rpc(os: DbThread, connection: Connection) -> CountResult {
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
