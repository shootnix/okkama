# Pinned IANA language data

Official Language Subtag Registry snapshot with File-Date 2026-09-17, retrieved
from the URL in manifest.json. The SHA256 fixes the exact input; no live registry
lookup occurs during generation/runtime. Complete source records are retained.

Run python3 tools/generate-iana-languages.py. The compiler emits preferred-value
lookup tables, grandfathered tags and extlang prefix data. Preferred subtag chains
are collapsed with cycle detection. Any upgraded source requires a new reviewed
manifest and independent registry-oracle tests. Registration/prefix validity and
Suppress-Script removal are not inferred from the preferred-value profile.

[RFC 5646 registry format](https://www.rfc-editor.org/rfc/rfc5646.html#section-3.1),
[official IANA registry](https://www.iana.org/assignments/language-subtag-registry/language-subtag-registry).
