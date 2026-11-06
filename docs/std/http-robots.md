# Robots policy and explicit fetching

`RobotsPolicy(text)` constructs immutable rules, compiled wildcard patterns,
agent groups, optional Crawl-delay and sitemap URLs. Parsing accepts CR/LF/CRLF,
one leading BOM and comments, ignores malformed/unknown records, orphan rules and
empty Allow/Disallow. Consecutive User-agent lines share their following rules;
sitemaps do not terminate a group. Agent rule tokens contain ASCII letters,
underscore or hyphen, plus the wildcard fallback.

`allowed(agent,url)` receives an explicit agent identification and HTTP/HTTPS URL.
The longest case-insensitive product-token substring selects groups; equally
specific groups combine. Supply the exact product token for RFC9309's ordinary
matching contract; substring selection is an explicit compatibility choice for
full identification strings. Wildcard groups apply only without a specific
match. No group/rule match permits access; /robots.txt is implicitly permitted.
Paths are case-sensitive, query participates, fragment does not. Unreserved
escapes normalize once, reserved escapes stay distinct, non-ASCII compares as
UTF-8 percent encoding. Wildcard * and terminal $ are special only in patterns;
escaped %2A/%24 match literal target characters. Longest normalized literal
pattern wins; allow wins an equal-specificity tie. This is crawler coordination,
not authorization or a filesystem trust boundary.

`delay(agent)` returns the largest valid delay from the selected groups, or
MissingCrawlDelay. Crawl-delay is an explicit nonstandard extension, accepts
nonnegative decimal seconds up to 86400 with nanosecond precision, and never
waits implicitly. `sitemaps()` retains ordered valid absolute HTTP/HTTPS URLs
without fragments; invalid URLs are ignored.

Budgets: 512 KiB UTF-8 input, 16384 lines, 8 KiB line, 256 groups/sitemaps,
4096 retained records and 262144 wildcard matching steps per rule. Exhaustion
throws RobotsLimitExceeded; there is no catastrophic regexp expansion. Patterns
are compiled once; trim stops at the first non-whitespace character.

Literal patterns use exact prefix comparison, or full equality for terminal $.
Their wildcard flag and specificity are retained at construction. Escaped %2A
stays literal; wildcard patterns use the bounded native string_star_match
primitive. It treats only '*' specially and preserves the greedy retry/anchor
machine and its262144 logical-step budget. UTF-8 scalar advances use byte
cursors without a full character array; native exhaustion remains the typed
RobotsLimitExceeded("match_steps"). URL normalization and rule policy remain in
this module.
allowed() uses cached monotonically ordered rule group ids. Two binary bounds
select each chosen group range; duplicate selected ids are processed once.
selected() still exposes its original records, including repeats, and applicable
rules keep source order. Agent records are scored once per selected() call.
Validated Crawl-delay Duration values are retained at construction, and delay()
uses the same indexed group-range strategy without reparsing text. Duplicate ids
do not repeat a delay record; zero is present, and the largest selected delay
still wins. Agent substring selection and policy parsing retain their separate costs.
Wildcard matching remains a synchronous bounded operation; the cap is not a
scheduler latency guarantee.

`FetchedRobots(exchange,site,agent)` is lazy and memoizes one operation. It
requests the site's rooted /robots.txt with explicit User-Agent and Accept.
It adopts/closes the returned body and caps reads before complete publication.
2xx strict UTF-8 is parsed; 4xx yields allow-all, 5xx yields deny-all. Other
statuses throw RobotsFetchFailed. Network/encoding/quota errors propagate, never
become allow-all. Cancelling policy() cancels and joins the owned memoized work.
Redirects, access/deadline/credentials and caching must be supplied deliberately
through Exchanging; the adapter creates none of those policies.

[RFC9309](https://www.rfc-editor.org/rfc/rfc9309.html),
[runnable example](../../examples/std-v4/http-robots.ok) and
tests/performance.
