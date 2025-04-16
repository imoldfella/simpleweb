tasks will be part of the thread,
transactions will be a subset of the connections, allocated in the thread
allocated memory will mostly be part of the strand; we can easily reset the memory when the strand is committed or rolled back.

remote execution by default cannot choose a transaction; each statement (encapsulated by a stream) is executed inside a transaction. the general approach should be to build an rpc that manages the transaction and call that instead. host rpcs are allowed execute multi-statement transactions.

