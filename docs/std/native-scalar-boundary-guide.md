# Native error and Unicode scalar boundaries

Contract before correction of defects exposed during TOML conformance work.

The portable NativeFailure codes are closed and already defined in the runtime.
An out-of-range child/event/attribute index is invalid_input. Observing a regex
substitution before its result is stored is invalid_input. Requesting the root of
invalid JSON is invalid_data. These expected errors must never fail while
constructing NativeFailure or become an unexpected provider exception.
An absent regex match is not_found; malformed patterns are invalid_input and
compile budgets are resource_exhausted. Failed query preparation must remain
a deterministic failed observation, never a ready query without a result.
The public domain modules retain their existing typed error protocols.

Okkama strings and UTF codecs preserve Unicode scalar values: U+0000..U+10FFFF
excluding surrogate codepoints U+D800..U+DFFF. Noncharacters are scalars too;
codecs must neither reject, replace nor delete them. This applies equally to
UTF-8, both UTF-16 orders, both UTF-32 orders and text-to-byte stream adaptation.
Malformed byte sequences, isolated surrogates and values above U+10FFFF remain
errors. Validation and decoding must agree. No BOM inference or normalization.
See the [Unicode FAQ](https://www.unicode.org/faq/private_use.html#nonchar1).

The defect comes from Perl Encode's strict interchange policy, which is narrower
than the language's scalar domain. Fix only that boundary; do not hide warnings
from unrelated operations or make decoding accept Perl's extended non-Unicode
codepoints. Native/legacy UTF-8 entry points must retain the same scalar contract.

Tests: exact independent byte fixtures (not only encode/decode round trips),
typed native failures, malformed sequences, all 66 noncharacters, both orders,
stream adaptation and repeated observations. Add embedded tests/comments to
runtime/json, runtime/html, runtime/regex, runtime/utf and encoding/utf.
Audit other native UTF-8 boundaries (filesystem/environment/process) separately
with concrete fixtures before changing their resource-specific behavior.

Full regression of commit 96e29d2: **403 files / 5352 tests, PASS**.
