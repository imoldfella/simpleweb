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


maybe with config we have different futures for the uring or mio reactor.


how to define interfaces?

partition sets, tokens.


get_token('schema.set', 'name') -> once we have a token we can use it an token field.
create_token('schema.set', 'name', 'from', restrictions)
-- implies that we coul
grant_token('schema.set', 'name', user[]) 
revoke_token('schema.set', 'name', )

being able to create a secured schema, not just a function set, is ideal; allows fluidity for guis.

you should only be able read and write tuples that you have a capability for. 


a connection is a process, we inject schema partitions (tokens) into the process.

1. Each schema has a set of tokens. Each token stores the user, a partition id, a parent token, and restrictions. Tokens are created immutably, they can be revoked by the owner or the owner of a parent token. There is a set of root tokens for the schema, thus the schema creator(s) can revoke any token in the schema. 
2. Every tuple has a partition id as part of its primary key. You can read the tuple if you have a read token, you can delete or insert the tuple if you have a write token. 
3. after creating a connection, the user must inject appropriate tokens before calling procedures. 
4. tokens can be queried from a read only system table. Users can query their own tokens and descendants of those tokens. 
5. stored procedures are associated with orthogonal partitions; a partition of stored procedures is called an interface. Stored procedure tokens can be restricted to a subset of read/write/execute. Stored procedures may execute with tokens of their own that are stored when they are created.
   

QA
How are tokens expired or garbage collected? 
Tokens may have indefinite lifetime. The capability formed from injecting it into the connection is deleted when the connection is closed. Tokens may be restricted to device, location, time range, date range, these restrictions are checked when inserting into the connection and if accepted the connection is then limited by the intersection of these restrictions.

How are token uses logged?
Tokens inserted into the connection are logged as part of the connection logging; you can recover the database to any point in time, thus storing the query (or DML) and the timestamp allows recovery of who viewed or modified which tuples.


Write tokens allow insert/delete — but what if a user has a write token for partition X, and inserts data “on behalf of” someone else?
The tuple must be inserted into a partition. There is no way to insert something on behalf of someone else. The logs will always track back to to the inserting user. An application level field could be used to record the intent.

Is the partition_id auto-filled, or must it be enforced during INSERT?
Autofilling is left for future work, for this proposal it must be provided with every DML statement as part of the primary key.


future work?
should tokens allow restriction on table.attribute, eg spend.amount < 5000. This can be worked around with stored procedures.


problem of leakage - If I write with my id, I can see that the key exists. A partial solution is to use automatic row ids as the primary key. then the real primary key is (partition, rowid). equivalently the partition is always considered part of the primary key.

special token to allow querying the token table

maybe have a process id in each rpc stream? it could be associated with the connection, but then we would need an extra connection if we wanted to have multiple processes.

