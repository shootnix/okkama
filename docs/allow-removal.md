# Private fields: migration from removed grants

`allow` declarations are intentionally incompatible with the current language.
Object/role/special/generic targets are rejected. No flag restores them. Fields
remain private to lexical owner code, including `self.field`. Roles and caps
grant only their declared public behavior/computed fields.

Move the operation into the owner, then delegate from collaborators:

```okkama
object Amount {
    has minor is int;
    Amount(minor is int) { self.minor=minor; }
    matches(expected is int) is bool { minor==expected; }
}
object Receipt {
    has origin is Amount;
    Receipt(origin is Amount) { self.origin=origin; }
    settled() is bool { origin.matches(100); }
}
```

No scalar/handle getter is required for this comparison. For resource adapters,
retain the owner and invoke its operation; keep close, cancellation and failure
semantics in the operation. New acquisitions return typed resources with their
own lifecycle, never the owner's existing private native state.

| Migrated family | Replacement |
|---|---|
| File bytes/text/position | File performs reads/writes/seek/truncate; views delegate. |
| FileLock | File acquires FileLockLease; wrapper owns close and maps acquisition failures. |
| TemporaryFile | owner computes path/preservation; adapters retain it. |
| Process pipes | Process starts lazily and performs pipe operations once per call. |
| Unix/TCP/TLS halves | connection performs I/O and cleanup; shared read/write queues remain private. |
| TLS upgrade | owner executes drained-operation check and consuming upgrade, never returns SocketState. |
| UDP multicast | socket acquires a typed membership lease; group/interface observations stay on the wrapper. |
| SecretBytes | explicit export_bytes operation; ExportedSecret delegates, respecting close. |
| Basic/Digest credentials | owner computes wire header/digest; password/state never becomes a peer field. |
| Certificates | owner computes typed names/extensions; adapters expose metadata. |
| Regex | owner creates a behavioral memoized query; pattern state stays private. |
| SQLite | connection begins the requested transaction mode; SqliteOrigin delegates. |
| Cookie jar | validated text restoration successor and explicit persistence-text operation; no Atomic cell accessor. |
| MIME | part renders its own structured body with the existing budgets. |
| HTTP1/HTTP2 framing | owners run their state machines; validated observations and continuation operations replace field traversal. |

Historical research archives and source-pinned positive grant fixtures describe
the compiler that produced their evidence. They are historical inputs, excluded
from current language acceptance; execute current rejection/migrated fixtures
instead. The removal does not alter HTTP Allow headers, robots directives or
ordinary allowlist data. Task evidence and actual verification status are under
IMP-27.
