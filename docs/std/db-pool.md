# Database pool: FIFO leases and explicit cleanup

`DbPool(DbConnecting,PoolOptions,Timing)` owns file/external physical connections.
`SqliteConnector(SqliteDatabase)` supplies the built-in SQLite adapter. A fourth
constructor argument supplies an optional DbChecking health capability. The default
only observes closed; no magic SQL query is injected. There is no built-in adapter
which pools independent SqliteMemory databases.

PoolOptions takes minimum, maximum, idle_timeout, lifetime, acquisition_timeout.
Minimum must be0..maximum; maximum1..64; durations nonnegative with a positive
acquisition timeout. Zero idle/lifetime disables that expiry policy. Minimum is
warmed lazily during first acquisition under the same deadline, not synchronously
in a constructor. Expiry is checked before reuse/return; there is no idle sweeper.

`connection()` returns PooledConnection with portable execution, rows, transaction,
closed, close and settled. Semaphore ownership serves waiters FIFO. The explicit
monotonic acquisition deadline covers queueing, opening, minimum warmup and health
checks. Cancellation/shutdown removes the queue position and disposes any unclaimed
grant exactly once. Acquisitions in PoolSnapshot are current issued leases,
idle_connections current idle records, waiting_requests pending acquisitions
(including opening), capacity configured maximum. Observations are not locks.

Lease close immediately revokes the borrower's reader/transaction wrappers and
schedules reset. Active readers close and transactions roll back before a healthy
physical connection enters idle. The capacity permit is returned after that reset.
The next borrower therefore cannot see an earlier transaction's uncommitted work.
Expired, broken or reset-failing physical connections are closed and reaped rather
than pooled. Temp tables/session-local state on a healthy physical connection are
preserved; resetting user SQL/session settings requires an external connector
policy, not a hidden ambient statement.

Pool close wakes waiters, rejects new acquisition, revokes outstanding leases and
closes idle/returned physical connections. It does not wait for users to return
leaked leases. `settled()` joins outstanding reset/close tasks and reports cleanup
failures. Use `defer await pool.settled(); defer pool.close();` in this order.
Lexical lease cleanup returns its physical connection; using a closed lease fails.
Pool records retain acquisition futures so resource lifetime is not entrusted to
an Atomic payload alone. Completed retirement records are pruned; the first cleanup
failure is retained for settled rather than lost during pruning.

Tests cover actual SQLite reuse/rollback, FIFO, cancellation/timeouts, both expiry
policies, minimum warmup, plus a dependency-free fake connector for reset failure,
opt-in health, invalid options and shutdown with queued requests/leaked leases.
See [SQLite integration](db-sqlite.md) and the
[runnable complete example](../../examples/std-v4/database-stack.ok).


## Explicit preparation facets

Portable DbConnecting erases extra methods by design. `DbPreparedPool` accepts a
DbAcquiring source which returns DbAcquisition: the portable connection plus
explicit preparation and prepared-transaction capabilities for that same physical
connection. `SqliteAcquiring(SqliteDatabase)` is the built-in source. The pool
retains these facets without downcasting its DbConnection role.

```okkama
let pool=DbPreparedPool(SqliteAcquiring(database),options,clock);
let lease=await pool.connection();
let prepared=await Prepared(lease,"SELECT ?").statement();
let rows=await prepared.rows([DbInteger(BigInt("7"))]);
let row=await rows.row();
rows.close();
prepared.close();
lease.close();
```

`PooledTransactions(lease).transaction(options)` returns a concrete PooledTransaction
which also implements DbPreparing. Use Prepared(transaction,text) for compiled
execution inside it. Ordinary lease.transaction returns the confined portable
Transaction role. Transaction end revokes transaction-bound prepared statements;
lease close revokes/closes preparation and readers before reset. A portable source
which supplies no preparation facet reports unsupported; it does not silently
pretend a repeatedly parsed Statement is a native prepared statement.

Connector implementations must bind the supplied facets to the same owned physical
connection. This is an explicit connector contract, not runtime reflection or a
universal object downcast. DbPreparedOpening.prepared_transaction returns a
DbPreparedTransaction wrapper containing the two explicitly granted capabilities.
