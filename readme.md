tasks will be part of the thread,
transactions will be a subset of the connections, allocated in the thread
allocated memory will mostly be part of the strand; we can easily reset the memory when the strand is committed or rolled back.

remote execution by default cannot choose a transaction; each statement (encapsulated by a stream) is executed inside a transaction. the general approach should be to build an rpc that manages the transaction and call that instead. host rpcs are allowed execute multi-statement transactions.

connection.ready() {
     // process tls
     // clear out the plain text buffer


pub struct WebSocketProcessor {
    state: State,
    buffer: Vec<u8>,
}

impl WebSocketProcessor {
    pub fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        input: &[u8],
    ) -> Poll<Result<Option<Message>, WsError>> {
        // FSM logic here...
    }
}

enum {
    start(&[u8], length)
    middle(&[u8])
    finish(&[u8])
}

one other challenge is how to write this into a tree

// but don't these need to be async?
// disk writes?
trait WebTransportProcessor
  start(stream, &[u8], length)
  middle(stream, &[u8])
  finish(stream, &[u8])
