use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

//static mut IS_URING : bool = false;

pub struct ConnectionInner {
}
pub struct Connection{
    inner: *mut ConnectionInner,
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
