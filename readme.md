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

start with environment id:16, interface handle:16, proc id:32.

not worth the effort? we could have varlen ids in the header, then use first fit for parameters? nah.

for a server using vm for memory management is the way?

a large memory area could be associated with the connection (and the number of connections per thread could be fixed at a high number).

the thread could then commit its actual memory to mmap as needed.

if it runs out of memory it can suspend a connection, and even swap it to disk.

one downside is that every madvise comes a shootdown. exmap shows a way to have no shootdowns on allocation, and to batch shootdowns on free. claim is that they read protect the evicted page - but why doesn't this require a shootdown?
eviction does do a shootdown. when allocating, the other cores don't necessarily see the new page right away, but they won't get accidental access to the old page.

another downside is repurposing to wasm.

in wasm maybe there is a single connection and a single environment, so there is nothing to steal memory from anyway.

does it make sense to have multiple strands inside an environment? each strand is a series of transactions.

maybe in wasm we resort to killing transactions when we run out of memory.
we could do that on the server.

Can we use wasm itself to model the strands, and then we have a form of vm?
what if we have a webassembly module (with its own memory) for each strand?

the worker is the thread, and it schedules to different wasm images.
It can throw away entire strands.

option 1: Strand holds the memory; it can grow and shrink using vm

option 2: statements hold (most) memory, the statement memory can grow or shrink. statement releases its memory block back to a pool when it completes, the strand gets another one when it begins a new statement.



# open transactions
Generally a network round trip should not be inside a transaction, but it could have uses. Transactions can be autocommit = true|false.

stream header = (environment, interface, procedure, continues)
if continues = 0 then use autocommit
if continues = 1 then the first statement returns a continuation handle.
else continues is the continuation handle, add 1 to continue again or send the handle (+0) to autocommit.

the final commit or roll back can be a special interface/procedure.

aside: we really only need interface and procedure in the header, giving us the schema of the parameter block. the other things (environment, continues) could be normally fit in the parameter block.

client sends versions it can speak, server picks one.

we can probably allocate all the memory from the thread that owns the transaction, then use helper tasks to manipulate that memory. many algorithms take advantage of local memory, maybe we need a thread scratchspace for tasks that won't be interrupted.

# host udfs and stored procedures
udfs: natively vector, arrow.

stored procedures.

host:
  register_procedure(
     "schema.interface.name",
     callback
  )

  maybe there should be a pipe instead of a callback, go does not like callbacks.

  if we model after io uring we have 

  submit_and_wait() // wait for cqe.

  does this work well for go? we are blocking a thread, but we only do this when there isn't anything to do?


 main ( ) {

var dispatch = map[string]func(args ...interface{}) error {
    "start": startHandler,
    "stop": stopHandler,
    "restart": restartHandler,
}

    db := initialize_database(config, dispatch )    

the stored procedure should get some basic handles to the transaction?



what about just using the database directly, as if over web sockets, but without callbacks?

we probably want a preprocessor for most things.



   any go routine 

      let stmt = mod.prepare(db);
      

      ( result, error, continue) := stmt.some_call(1,tx, more args...);
      // do something with result while holding locks.
      // now commit
      ( result, error, _) =  stmt.another_call(continue+1, ...);

      
      stmt[0](tx, ); // variadic?

      tx.commit();

      another way to do it would be to buffer one statement ahead until commit? you can't because by definition you need the result.

      what about sending a stream (reader?) as an argument? this could be an annotation in the preprocessor.

// is there a need for sending a blob before we know the lenghth?
// what would it look like?

// in most cases we know the length, or the transaction needs to mutate a blob by appends. 

option 1; start with tuple, follow with blobs
option 2; start with blobs, follow with tuple.

starting with the tuple potentially allows us to compile special support into the proc, but do we want that complication?


how to get the fastest possible lookup: packet in to packet out.

good to reuse packet? I think erpc allows this.

the packet that we get from uring is a packet that we posted. we can reuse it in the response. esp with ktls?

