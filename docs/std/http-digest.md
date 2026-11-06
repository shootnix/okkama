# Scoped Digest authentication

Import Digest objects from `http/digest`; generic received field parsing lives
in `http/auth_challenge`. Existing `http/auth` Basic/Bearer APIs stay intact.

- `DigestCredentials(username is string,password is SecretBytes)` exposes only
  username(). The username is nonempty, at most 1024 scalars, without colon or controls;
  password is at most 4096 input bytes. ASCII is required without charset; an
  explicit UTF-8 challenge normalizes username/password to NFC before hashing.
  Malformed UTF-8 passwords fail with a fixed reason. The caller owns the secret.
- `DigestChallenge(AuthenticationChallenge)` selects SHA-256/SHA-256-sess or SHA-512-256/SHA-512-256-sess and
  qop=auth, retaining realm, nonce, opaque, algorithm, stale and userhash.
  An absent algorithm means unsupported legacy MD5, never an inferred SHA-256.
  Unknown algorithm, missing/unsupported qop or unavailable charset raises
  UnsupportedDigestChallenge. Realm/nonce are required; controls/sizes are checked
  without placing their values in diagnostics. An explicit UTF-8 charset enables Unicode credentials/NFC. UTF-8 realm
  octets remain exact (realm is not normalized); logical Header values retain
  the usual one-scalar-per-wire-octet representation.
- `DigestAuthorization(credentials,challenge,request,cnonce is string,count is int)`
  is an explicit wire capability implementing HttpAuthorizing. H1.1 and H2 source
  requests share the actual origin-form path/query. It emits required quoted
  parameters, algorithm/qop/nc tokens, fixed-width eight-digit nc, and optional
  userhash/opaque. A non-ASCII normalized username without userhash is emitted
  through `username*=UTF-8''...`, never as an invalid quoted Unicode string. count is 1..2^32-1; cnonce is printable ASCII, 1..256 characters.
- `DigestExchanging(origin is Exchanging,scope is Url,credentials,random is SecureRandom)`
  implements Exchanging. HTTPS origin/path scope uses AuthenticationScope. Out of
  scope it delegates unchanged without secret export. An explicit Authorization
  in scope conflicts before delegation or entropy use.

Every direct call starts unauthenticated. Only an in-scope direct 401 at the same
URL, with a replayable body, can cause authorization. The first supported Digest
challenge in server order is chosen. Unsupported-only challenges and one-shot
bodies return the original 401 with its readable body. Before replay, the unread
challenge body closes, releasing its actual client admission. There is one
normal authorization attempt and at most one further stale=true refresh using a
different nonce. Wrong credentials cannot create an authentication loop.

A fresh 128-bit SecureRandom cnonce is generated per authorization. A bounded
atomic ledger gives increasing counts for each nonce; 128 retained nonces and
uint32 exhaustion raise a typed limit rather than silently reusing counts.
This is invocation-local challenge handling, with no preemptive nonce cache,
domain-based scope expansion or nextnonce reuse. Same nonce use remains counted
across calls and concurrent reservations. Caller cancellation cancels/drains the
actual exchange Future; failures close discarded response bodies. Generated
Authorization stays internal to its direct hop; ClientResponse reports the
caller's original Request and retains streamed body and actual details.

If Authentication-Info is present, qop/cnonce/nc/rspauth must bind to the actual
authorization. Rspauth is compared through existing constant-time secure_equal.
A bad header proof closes the response body before return. A BodyReading adapter
checks optional trailer proof at EOF, closes on failure and retains trailer
metadata. The saved expected wire proof requires no later password export.
Absent Authentication-Info is accepted; qop=auth supplies no response-body
integrity. Final trailer validation requires reading to clean EOF.

Use a conforming direct verified-HTTPS Exchanging consumer, normally HttpClient.
Put Redirecting outside DigestExchanging so each hop re-evaluates scope. An
internally redirecting delegate is rejected after an authorized exchange; it
cannot uphold per-hop scope and must not be used for this purpose. Put Retrying
outside the Digest adapter so a replacement exchange generates fresh nc/cnonce.

`AuthenticationChallenges(string[,bytes,challenges,parameters]).values()` returns
immutable received challenge snapshots. Defaults: 8192 octets, 32 challenges,
32 parameters each; at most 64 consecutive empty list members. Scheme and
parameter names match case-insensitively, duplicate names reject, quoted pairs
unescape once, token68 and quoted commas are distinct. Token/quoted variants
are accepted on input; Digest emission follows its historical sender formats.
InvalidAuthenticationChallenge exposes only fixed reasons. Snapshot observations:
AuthenticationChallenge.scheme()/token68()/present(name)/parameter(name)/quoted(name).

[Digest calculations and negotiation, RFC 7616](https://www.rfc-editor.org/rfc/rfc7616.html#section-3.4).
[Received list/parameter syntax, RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html#section-11.1).
Evidence: 18 files /144 assertions PASS,
inline and actual HTTPS CLI PASS. The modern profile required one native digest dispatch addition and the full
727-file /12773-assertion frozen-source gate PASS. Auth-int and preemptive nonce
caching remain open.


The SHA-512/256 implementation uses the distinct FIPS initial state. RFC7616's
original 3.9.2 constants instead came from truncated SHA-512; erratum 4897 is
Reported. Fixed abc and independent OpenSSL computation verify the algorithm
and corrected Unicode example; investigation.
