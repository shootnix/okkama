# Host names and pinned public suffix policy

`net/endpoint::HostName(origin)` eagerly converts a bounded source to canonical
ASCII and validated Unicode spelling. `ascii()`, `unicode()` and
`Comparing<HostName>.order()` observe that immutable name. Case/NFC/IDNA-equivalent
spellings have structural equality. `HostEndpoint(host,port)` retains a HostName
and an integer 0..65535; `host()`, `port()` and `string()` observe it. Port zero
remains representable for bind policy; an actual destination consumer must reject
it. These values perform no DNS, service lookup or connecting.

The backend is Net-IDN-Encode 2.591 (installed release 2.591-TRIAL), with strict
UTS46 nontransitional processing, STD3 ASCII rules, assigned-codepoint checks,
bidi and ContextJ validation. This is the UTS46 compatibility profile, not a claim
that every accepted character is IDNA2008 PVALID: the pinned profile accepts
some symbol labels. Mapping data are Unicode 10.0; current Perlbrew 5.42.2 uses
Unicode 16.0 properties and normalization. Newer Unicode mappings are not claimed.
The version requirement excludes known buggy older punycode backends. The native
adapter additionally checks canonical ACE round trips, scalar validity, DNS label
width and complete width. No permissive Encode wrapper that silently passes
invalid ASCII through is used.

Raw source limit is 1024 scalars, applied before conversion in std and direct
native calls. Canonical labels are 1..63 ASCII bytes, complete name at most 253
bytes excluding one terminal root dot. The terminal dot is preserved and remains
part of name identity. Internal/leading empty labels, separators, controls,
non-scalars, malformed UTF-8 and invalid/fake A-labels fail. `InvalidHostName(origin)`
implements Error with `origin()` and `text()`. Missing/obsolete backend is a
portable unsupported NativeFailure, never misreported as bad input.

`net/public_suffix::PublicSuffixList()` represents the bundled ICANN **and
PRIVATE** policy. `version()` identifies the exact data. `suffix(HostName)` and
`registrable(HostName)` return canonical unrooted HostName values; `public(HostName)`
is true exactly when no registrable domain exists. `MissingRegistrableDomain(host)`
implements Error with `host()`/`text()`. These are domain-name operations;
an URL host typed as a numeric address must use address policy, not PSL inference.

Matching uses longest exact/wildcard rule, exception precedence, and the default
single-label wildcard for unknown TLDs. The terminal DNS root marker does not
participate. Private suffixes such as github.io/blogspot.com remain boundaries.
The generated immutable hash index needs at most three rule observations per
input label, independent of dataset size; this is a key lookup bound, not a total
VM latency claim. Table construction is lazy once per Perl process. No live list
download, ambient system database or mutable resolver state occurs at runtime.

Pinned source: version **2026-10-07_07-28-19_UTC**, commit
`3929462652695bad04f0a27afb600974014a3c8b`, 10336 canonical rules.
SHA256 `ba7d836c0ea57a8bcf9f1fcb868066a7554bed029b441ecc6204ad8494a32612`.
The raw MPL-2.0 source is `okkama-pl/share/public_suffix_list.dat`; generated
provider data preserve version, source hash and license. Regenerate explicitly:

```sh
perl okkama-pl/script/build-public-suffix.pl \
  okkama-pl/share/public_suffix_list.dat \
  okkama-pl/lib/Okkama/Runtime/Native/Core/PublicSuffixData.pm
```

Install the reviewed backend with the current Perlbrew:
`cpanm --dev Net::IDN::Encode@2.591`. It is recorded in cpanfile. At this time the
fixed upstream version is a trial release; deployment must install that explicit
version rather than silently falling back to the older stable version. Backend
installation runs its upstream tests; project tests cover native failures,
canonicalization, expected Unicode vectors and consumer lifetimes.

Sources: [UTS46](https://www.unicode.org/reports/tr46/),
[strict backend](https://metacpan.org/pod/Net::IDN::UTS46),
[upstream fixes](https://metacpan.org/release/PJCJ/Net-IDN-Encode-2.590-TRIAL),
[official PSL](https://publicsuffix.org/list/).
[Example](../../examples/std-v4/host-policy.ok),
evidence.
