# Typed channels and state ownership

## Purpose

Stateful Okkama-компонент реализуется как async-задача, единолично владеющая
текущим immutable state. Состояние изменяется созданием следующего значения и
rebinding локальной переменной. Остальные компоненты взаимодействуют с
владельцем посредством типизированных сообщений.

Immutable objects are values, not storage locations. A repository, cache,
registry, or session component therefore keeps its current value in a local
binding owned by one long-lived task:

```okkama
async run() is int {
    let state = InitialState();

    loop {
        let command = await commands.receive();
        state = state.applied(command);
    }
}
```

Clients never receive that binding. An immutable client object stores only a
`Sender<Command>`. This serializes state transitions without mutable fields,
mutable maps, global user storage, locks, or a new actor syntax.

## Public API

The `async/channel` module exports:

```okkama
Channel<T>
Sender<T>
Receiver<T>
ChannelClosed
InvalidChannelCapacity
```

Channel construction uses an expected type:

```okkama
import "async/channel" take Channel, Sender, Receiver;

let channel is Channel<int> = Channel(capacity: 16);
let sender is Sender<int> = channel.sender();
let receiver is Receiver<int> = channel.receiver();
```

The endpoint operations are:

```okkama
async Sender<T>.send(value is T);
Sender<T>.close();
async Receiver<T>.receive() is T;
Receiver<T>.close();
```

`Channel<T>`, `Sender<T>`, and `Receiver<T>` are invariant. Endpoint fields,
parameters, and results retain the same concrete `T`.

A channel has identity and stores only messages awaiting delivery and pending
channel operations. It is not a repository, cache, or general user storage.
Messages remain ordinary Okkama values. The channel does not serialize them or
expose its queue as an Okkama collection.

## Capacity and backpressure

Every channel is bounded. `capacity` must be greater than zero. Capacity zero
is not a rendezvous channel in this version; construction throws
`InvalidChannelCapacity`, and `capacity()` returns the rejected value.

`send(value)` follows these rules:

1. `value` must have type `T`.
2. If a receiver is waiting, the message is delivered directly to the oldest
   receiver.
3. If buffer space exists, the message is appended to the FIFO buffer.
4. If the buffer is full, the sending task is suspended behind older senders.

Receiving one buffered message admits the oldest blocked sender. Sending and
receiving do not poll and do not block unrelated tasks.

Channel enqueue and dequeue operations, including waiter management, must be
amortized O(1). An implementation must not require copying or serializing a
message for ordinary in-program delivery.

## Delivery and ordering

The first version guarantees:

- FIFO order for buffered messages;
- FIFO order among blocked senders;
- FIFO order among waiting receivers;
- exactly one successful receive for each delivered message;
- no delivery duplication or implicit broadcast;
- direct handoff does not leave a copy in the buffer;
- freeing capacity resumes a blocked sender;
- separate channels have separate identity and state.

Multiple `Sender<T>` and `Receiver<T>` values may refer to one channel. The
language does not statically enforce a unique receiver. With several
receivers, each message goes to exactly one receive operation in waiter FIFO
order. Strict global fairness between all tasks is not guaranteed.

## Closing

Closing is explicit and may be initiated through either endpoint:

```okkama
sender.close();
receiver.close();
```

`close()` is idempotent. After closing:

- new sends fail with typed `ChannelClosed`;
- blocked senders fail with `ChannelClosed`;
- already buffered messages remain receivable;
- waiting receivers fail if no buffered message can satisfy them;
- after the buffer is drained, every receive fails with `ChannelClosed`;
- completed operations no longer remain channel waiters.

There is no automatic close based on the lifetime of sender values. Owners and
their callers use an explicit stop protocol, await task completion, and then
close the command channel.

## Request-response

Request-response uses ordinary channels rather than special RPC syntax. A
command carries a capacity-one reply sender:

```okkama
object FindBook {
    has id is int;
    has reply is Sender<BookResult>;
    FindBook(:id, :reply);
}

async book(id is int) is BookResult {
    let reply is Channel<BookResult> = Channel(capacity: 1);
    await commands.send(FindBook(id: id, reply: reply.sender()));
    await reply.receiver().receive();
}
```

Each request creates a different reply channel, so concurrent responses cannot
be received by the wrong client. Domain failures are represented by a typed
result or caught by the owner and translated into one. Channel lifecycle
failures are typed thrown errors.

No special `ask`, `call`, RPC, or promise-response syntax is introduced.

## Owner lifecycle and failures

A normal owner lifecycle is:

1. Construct a bounded command channel.
2. `spawn` the async owner action.
3. Give immutable clients copies of its sender.
4. Send a typed `Stop` command carrying a reply sender.
5. Let the owner reply and leave its loop.
6. Await the `Future<T>` returned by `spawn`.
7. Close the command channel.

Expected command-level failures are caught inside the owner so it can reply
and continue serving later commands. A fatal failure settles the owner task
Future and is observed by awaiting it.

Task failure does not automatically close channels used by that task. An owner
that may fail while serving a request must register cleanup before doing
fallible work:

```okkama
defer commands.close();
defer command.reply().close();
```

The first close wakes blocked command operations. The second guarantees that
the active requester observes `ChannelClosed` if no reply is produced.
Expected failures should still be translated into domain replies. Requests
already buffered but not yet selected require an application cancellation or
supervision policy; automatic task-channel lifecycle linking, cancellation,
timeouts, and supervision are not part of this version.

## Memory and ownership

A channel retains only undelivered messages and pending channel operations.
After delivery, a message is removed from the queue. After an operation
completes, it is removed from the waiter set. Closing completes and removes
pending waiters.

Application state belongs to the owner task's lexical environment. Updating it
uses an immutable successor or another newly created value:

```okkama
state = state.saved(book);
```

No mutable object fields, mutable maps, hidden mutable closures, `Store`,
`Cell`, `Ref`, or global user-state registry are introduced by channels.

## Sharding

Independent owners may partition state without a global lock. A router stores
an immutable array of client objects and chooses one:

```okkama
shards[key % shards.size()]
```

Commands within one shard remain serialized. Tasks for different shards may
make progress independently.

## Examples

Each example's `ExampleResult.value()` computes `42` when its demonstrated
invariant holds. The `async main() is int` entry awaits that action and returns
`0` for successful process completion. A main result is an exit status in
`0..255`, not printed output:

- [`producer-consumer.ok`](../examples/async/producer-consumer.ok):
  bounded producer/consumer flow and explicit close.
- [`state-owner-counter.ok`](../examples/async/state-owner-counter.ok):
  local state rebinding and request-response.
- [`books-repository.ok`](../examples/async/books-repository.ok):
  immutable repository state, shared clients, and typed missing-book failure.
- [`cache-with-eviction.ok`](../examples/async/cache-with-eviction.ok):
  bounded commands and deterministic oldest-entry eviction.
- [`caching-proxy.ok`](../examples/async/caching-proxy.ok):
  async upstream called once across a miss and subsequent hit.
- [`sharded-cache.ok`](../examples/async/sharded-cache.ok):
  independent owners and integer-key routing without a global lock.

These examples use real `async`, `await`, `spawn`, and typed channels.

## Resource payload ownership (14 October 2026)

A send retains actual resource identities in its payload independently of the
sender's scope. Buffered and rendezvous delivery attach that retention to the
receiving scope; a returned value transfers it to the caller. Existing live
source aliases keep their owner. Repeated message identities are destroyed
once, after all retained ownership finishes. The rule includes resource graphs
hidden behind roles, collections and opaque `any`; no downcast is introduced.

Close preserves buffered values. Cancellation before handoff releases only the
undelivered retention; cancellation after handoff cannot revoke the message.
Ending all owner/endpoint capabilities cleans unread values. Root and test
teardown finish remaining channel ownership before native registry shutdown.

Implementation and verification.

Fresh endpoints are tracked even before an enclosing argument list or array
finishes. If a later expression fails, the endpoint capability retires with its
lexical scope, so it cannot keep unread payloads alive until program teardown.
This does not change explicit close/drain or delivery ownership rules.
