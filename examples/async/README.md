# Async channel examples

These programs demonstrate bounded typed channels and the state-owner pattern.
They use the real `async`, `await`, and `spawn` language features.

Every example is self-checking and returns `42` when its invariant holds. The
producer/consumer example additionally prints the messages it receives.

## Running an installed Okkama

From the repository root:

```bash
okkama examples/async/producer-consumer.ok
okkama examples/async/state-owner-counter.ok
okkama examples/async/books-repository.ok
okkama examples/async/cache-with-eviction.ok
okkama examples/async/caching-proxy.ok
okkama examples/async/sharded-cache.ok
```

The installed command expects the standard library in its configured
installation location.

## Running from this checkout

The development runtime requires Perl 5.42.2 and an explicit checkout stdlib:

```bash
OKKAMA_STDLIB_DIR="$PWD/std" \
  perlbrew exec --with perl-5.42.2 \
  okkama-pl/bin/okkama examples/async/producer-consumer.ok
```

Replace the final filename with any example below.

## Producer/consumer

File: [`producer-consumer.ok`](producer-consumer.ok)

One producer sends integers `1..5` through a channel with capacity 1. One
consumer receives and prints all five values. This demonstrates:

- channel construction;
- spawned producer and consumer tasks;
- FIFO delivery;
- backpressure;
- explicit close.

Expected stdout:

```text
1
2
3
4
5
```

Expected program result: `42`.

## State-owner counter

File: [`state-owner-counter.ok`](state-owner-counter.ok)

One task owns an immutable `CounterState`. Concurrent clients send commands
with capacity-one reply channels. The owner serially rebinds its local state
and stops through an explicit request.

Expected stdout: none.

Expected program result: `42`.

## Books repository

File: [`books-repository.ok`](books-repository.ok)

`BooksInMemory` is an immutable client that stores only a
`Sending<BooksCommand>`. The owner implements save, find, remove, and stop.
Missing lookup is represented by typed `MissingBook`; the expected domain
failure does not terminate the owner.

Expected stdout: none.

Expected program result: `42`.

## Cache with eviction

File: [`cache-with-eviction.ok`](cache-with-eviction.ok)

One cache owner supports get, put, remove, size, and stop. Its command channel
is bounded. When the configured entry limit is exceeded, the oldest entry is
removed deterministically.

Expected stdout: none.

Expected program result: `42`.

## Caching proxy

File: [`caching-proxy.ok`](caching-proxy.ok)

The proxy first asks its cache owner. On a miss it awaits an asynchronous
upstream owner and saves the response. Repeating the same request is served
from cache. The upstream counter proves that the upstream is called once.

Expected stdout: none.

Expected program result: `42`.

## Sharded cache

File: [`sharded-cache.ok`](sharded-cache.ok)

An immutable router retains an array of cache clients and selects a shard using
integer modulo. Each shard has its own channel, owner task, and immutable
state. Requests for different shards can progress independently without a
global lock.

Expected stdout: none.

Expected program result: `42`.

## Dining philosophers

File: [`dining-philosophers.ok`](dining-philosophers.ok)

The classic mutual-exclusion problem with no locks and no mutable state. One
waiter task owns the whole table as a single immutable value and rebinds it to
a successor on every transition; the five philosophers hold nothing but a
sender and their own reply channel.

Deadlock is impossible because both forks are handed over together or not at
all, so no philosopher ever holds one fork while waiting for another.
Starvation is impossible because hungry philosophers take a ticket, and a seat
may only be served ahead of an older ticket when the two share no fork.

Thinking and eating use `SystemClock().wait`, which yields to the
scheduler instead of blocking it, so philosophers really do compete for forks.
Non-adjacent philosophers dine simultaneously, which the printed table shows
as two `E` marks at once.

Unlike the other examples, this one also carries `test` blocks that pin down
the admission rules on hand-built states, so the guarantees do not depend on
whichever interleaving the scheduler happened to choose:

```bash
cd okkama-pl
perl bin/okkama -t ../examples/async/dining-philosophers.ok
```

Expected stdout: a running commentary of each philosopher, plus a line per
published table state, where `E` is eating, `?` is waiting and `.` is
thinking. Two adjacent `E` marks never appear.

Expected program result: `42`.

## Smoke test

The smoke test reads and executes the example files themselves:

```bash
cd okkama-pl
perlbrew exec --with perl-5.42.2 prove -v \
  t/interpreter/163-channel-examples.t
```

It verifies:

- every example returns `42`;
- producer/consumer stdout is exactly `1\n2\n3\n4\n5\n`;
- all examples use the checkout standard library.

Expected summary:

```text
Files=1, Tests=6, PASS
```

`completion.ok` demonstrates an `async ... is void` command, including repeated
await of its inferred pending operation. Run it with the compatibility `stdlib/`
library, like the other examples in this directory. Await produces no value.
