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

    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, no_op, no_op, no_op);

    unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
}

use std::sync::Mutex;

// each function in the vtable can
fn make_waker(index: usize, thread: usize) -> Waker {
    unsafe fn clone(ptr: *const ()) -> RawWaker {
        RawWaker::new(ptr, &VTABLE)
    }

    unsafe fn wake(ptr: *const ()) {
        get_server().wake(ptr);
    }

    unsafe fn wake_by_ref(ptr: *const ()) {
        get_server().wake(ptr);
    }

    unsafe fn drop(ptr: *const ()) {
        // get_server().drop(ptr);
    }

    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

    let v = (thread << 24) + index;
    let raw = RawWaker::new(v as *const (), &VTABLE);
    unsafe { Waker::from_raw(raw) }
}

pub async fn handle_connection(thread: usize, connection: usize) {
    let buf = [0u8; 1024];
    let n = get_server().read_some(thread, connection, &buf).await;
}
