use core::task::{Context, Poll};
use core::pin::Pin;
use futures::future::Future;
use heapless::spsc::Queue;

const QUEUE_CAPACITY: usize = 128;

pub struct Message {
    // message fields
}

pub struct Sender {
    queue: &'static Queue<Message, QUEUE_CAPACITY>,
}

pub struct Receiver {
    queue: &'static Queue<Message, QUEUE_CAPACITY>,
}

impl Sender {
    pub fn send(&self, msg: Message) -> SendFuture {
        SendFuture {
            queue: self.queue,
            msg: Some(msg),
        }
    }
}

pub struct SendFuture {
    queue: &'static Queue<Message, QUEUE_CAPACITY>,
    msg: Option<Message>,
}

impl Future for SendFuture {
    type Output = Result<(), ()>;

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(msg) = self.msg.take() {
            if self.queue.enqueue(msg).is_ok() {
                Poll::Ready(Ok(()))
            } else {
                self.msg = Some(msg);
                Poll::Pending
            }
        } else {
            Poll::Ready(Ok(()))
        }
    }
}

impl Receiver {
    pub fn recv(&self) -> Option<Message> {
        if let Some(msg) = self.queue.dequeue() {
            Some(msg)
        } else {
            None
        }
    }
}

pub struct SpscMatrix {
    sender: Sender,
    receiver: Receiver,
}

impl SpscMatrix {
    pub fn new() -> Self {
        let queue: &'static Queue<Message, QUEUE_CAPACITY> = Box::leak(Box::new(Queue::new()));
        Self {
            sender: Sender {
                queue,
            },
            receiver: Receiver { queue },
        }
    }
}
