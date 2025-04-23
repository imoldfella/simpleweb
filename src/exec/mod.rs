use std::{future::Future, pin::Pin, task::{Context, Poll}};

//static mut IS_URING : bool = false;

pub struct CountFuture {
   
}

impl CountFuture {

}

pub struct OkFuture {
}

// uring always uses i32 error? or mostly?
type CountResult = std::result::Result<usize,i32>;
type OkResult = std::result::Result<(),i32>;    

impl Future for CountFuture {
    type Output = CountResult;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}


pub struct ThreadExec {
    is_uring: bool,
}

impl ThreadExec {
    pub fn read_some(
        &self,
        thread: usize,
        connection: usize,
        buf: &[u8],
    ) -> Pin<Box<dyn Future<Output = CountResult>>> {
        Box::pin(CountFuture{})
    }
}