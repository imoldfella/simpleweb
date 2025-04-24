use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

//static mut IS_URING : bool = false;

pub struct ConnectionInner {}

pub enum ConnectionType {
    Udp,
    Tcp,
}

pub struct User {}

// interface = schema or no? is there a better security language we can use?
// each schema has multiple partitions; the user is authorized to access a subset of the partitions (like row level security) why not use the schema as the interface? we can always create
pub struct Connection {
    pub inner: *mut ConnectionInner,
    pub connection_type: ConnectionType,
    pub user: u32,

    // authorize connection, allows other rules than simply user (location, time)
    pub interface: Box<[u32]>,
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

pub struct ThreadExec {
    is_uring: bool,
}

impl ThreadExec {}

// how should we code routines? pass thread explicitly, or use thread local storage?

#[derive(Clone, Copy)]
pub struct Os {
    thr: *mut ThreadExec,
}

impl Os {
    pub fn read_some(
        &self,
        connection: Connection,
        buf: &[u8],
    ) -> Pin<Box<dyn Future<Output = CountResult>>> {
        Box::pin(CountFuture {})
    }

    pub fn request(&self, connection: Connection, streamid: u64, buf: &[u8], complete: bool) {
        // the first packet in a stream must be at least 8 bytes, (aside from the stream id in the header)
        // probably don't need to check here?
        if buf.len() < 8 {
            self.result_error(connection, streamid, -1);
            return;
        }
        // read 4 byte little endian schema and 4 byte little endian procid
        let interface_handle = u32::from_le_bytes(buf[0..4].try_into().unwrap());
        let procid = u32::from_le_bytes(buf[4..8].try_into().unwrap());

        if interface_handle > connection.interface.len() as u32 {
            self.result_error(connection, streamid, -1);
            return;
        }
        let interface = connection.interface[interface_handle as usize];
        if interface == 0 {
            self.result_error(connection, streamid, -1);
            return;
        }

        // return error is schema.procid is not authorized.
    }

    pub fn result_error(&self, connection: Connection, streamid: u64, error: i32) {}
    pub fn result(&self, connection: Connection, streamid: u64, buf: &[u8], complete: bool) {
        // the first packet in a stream must be at least 8 bytes, (aside from the stream id in the header)
    }
}

pub async fn some_fn(os: Os, connection: Connection) -> CountResult {
    let buf = vec![0; 1024];
    let result = os.read_some(connection, &[]).await;
    result
}

// an rpc will have some number of blobs; the final blob is the parameter block.
// the initial blobs maybe stored to disk depending on memory pressure.
//
pub async fn read_rpc(os: Os, connection: Connection) -> CountResult {
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
