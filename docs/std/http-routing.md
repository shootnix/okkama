# Immutable HTTP routing

`RoutePattern(path)` compiles at most128 raw path segments /8192 UTF8 bytes. Literal segments, `:name` and final `*name` are supported. Parameter names are distinct ASCII identifiers up to32 bytes. Queries/fragments, invalid percent/UTF8 and decoded controls are rejected. Split occurs before one percent decode; `%2F` is data inside one segment. Literal markers can be escaped (`/%3Aid`); Unicode and case are exact, with no normalization. Trailing/empty literal segments remain significant. Final wildcard may consume zero segments and joins decoded remainder with `/`; this parameter must not be reused as a trusted filesystem path.

`pattern.match(rawPath)` returns `RouteMatch`. `matched()` is explicit; `parameters()` throws InvalidRoute on no match. The immutable string map has the builtin deterministic key order. Low-level decoded helpers are bounded. `Route(method,pattern,handling)` binds capabilities without invoking them. `Routes(sequence,fallback)` snapshots at most256 routes and rejects all overlapping match languages for the same exact HTTP method. It provides declaration-order dispatch among the validated configuration; there is no implicit priority or shadowing. Different methods may share paths. Literal versus parameter/wildcard overlap for the same method also rejects; callers must partition those paths explicitly.

`Routes` does `Handling`. It accepts origin targets and H2 reconstructed absolute targets, decodes path once per request and ignores query for matching. Exact HEAD routes take precedence; absent one, HEAD uses GET while the request remains HEAD for wire suppression. Matched parameters overlay existing context parameters via successor, keeping transport observations/cancellation unchanged; count128 /aggregate8192 character bounds remain enforced. Only the chosen match creates parameter maps; failed candidates allocate none. Path/control/budget errors produce400. Matching path with unsupported method produces405 with deterministic, deduplicated Allow (GET adds HEAD). Missing path and CONNECT/asterisk targets use the explicit fallback. Host dispatch is intentionally an outer Handling composition and grants no TLS/Origin trust.

Requests Routes rejects itself go through `RouteRejecting`: `malformed(request, reason)` for path, control and parameter-budget errors (reasons `invalid route path`, `route parameter budget exceeded`) and `disallowed(request, allowed)` for 405. `Routes(sequence, fallback)` uses `EmptyRouteRejection` (empty 400; 405 with `Allow`); `Routes(sequence, fallback, rejection)` lets an application layer answer with a body, as `web/*` does. A 405 rejection must keep `Allow` built from `allowed`.

Handler futures are registered, cancelled and joined in their scope. No mutable route registration, resolver, socket or infrastructure acquisition occurs in a route constructor. [Example](../../examples/std-v4/http-routing.ok) prints200 and `a/b` for HEAD with an encoded-slash parameter. Focused/wire evidence includes actual H1/H2 HEAD suppression.

The root path `/` is zero segments: it matches `RoutePattern("/")` and a lone final wildcard (empty capture), and no literal or parameter pattern. Before 2026-10-13 a root request aborted the server with a map inference runtime error; root path fix and evidence. A runnable [web server example](../../examples/web/server.ok) listens on 127.0.0.1:8000.

Dynamic immutable map construction uses the bounded pure `runtime/collections.string_map_snapshot` provider rather than environment or JSON conversion. Its required full runtime gate is recorded separately; std-only routing gates do not rerun the entire interpreter suite. Graph validation groups literal prefixes by exact method and first decoded
segment; only potential intersections undergo full overlap checks. Root and
leading parameter/wildcard patterns keep conservative comparisons. Within one
prefix group or among broad patterns the worst case remains quadratic; performance investigation records one32-route sample and candidate improvements, without claiming request throughput.

## Validation partitions

Graph validation keeps source-order route snapshots for dispatch. Disjoint
first-literal groups stay cheap. Larger same-method families split on a later
position only when every member has a literal there and decoded values differ.
A worklist processes equal-value subgroups without host recursion. Parameters,
short paths, roots and terminal wildcards never become unsafe separators; groups
without a common separating literal retain exact pairwise overlap checks.

Capture names, including a final wildcard, use separate sorted uniqueness
validation. Segment grammar errors still precede duplicate names; parts keep
source order and parameter names remain case-sensitive. No handler or rejection
policy is invoked during validation. Request dispatch and the pair fallback
retain their separate costs. See the audit
and CPU backlog.
