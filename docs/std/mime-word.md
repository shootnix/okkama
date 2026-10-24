# MIME encoded words

`mime/word` implements a strict, explicitly contextual RFC 2047 codec, not a
parser for arbitrary structured headers. `EncodedWord(text)` defaults to UTF-8;
`EncodedWord(text,charset)` supports UTF-8 and US-ASCII, case-insensitively.
`string()` emits B words, at most 75 ASCII characters each, separated by CRLF SP.
Chunks end at Unicode scalar boundaries. Empty text emits an empty string.
ASCII controls other than HTAB, DEL and unrepresentable US-ASCII fail eagerly.

`DecodedWords(text)` operates on an **unfolded unstructured field body** such as
Subject. `DecodedWords(text,EncodedWordContext.forbidden)` returns the text
unchanged, without recognizing words. The other context is `unstructured`.
Structured address, quoted-string, comment and MIME parameter parsing must
choose their own grammar first; no automatic header-name/context guessing.
Phrase/comment decoding remains future consuming-parser work. MIME parameters
continue to use RFC 2231 and never call this codec.

In unstructured mode, only complete SP/HTAB-delimited tokens beginning `=?`
are candidates. Embedded syntax in ordinary tokens is literal. Candidates use
strict B or Q encoding, UTF-8/US-ASCII, and the 75-character bound. Q maps `_`
to space and `=HH` to bytes, with no soft line breaks. Bytes decode separately
per word, so a multibyte character split across words is invalid. SP/HTAB between
two decoded words disappears; other whitespace remains exact. Raw non-ASCII,
CR/LF and ASCII controls fail. Malformed candidates and unsupported charsets
throw `InvalidEncodedWord(offset,reason)`; offsets are original scalar positions,
with word-level charset/decoded-value errors pointing at the candidate start.
This strict codec does not implement a mail reader's tolerant display policy.

Third constructor argument `maximum_bytes` replaces the default 1 MiB budget
for input and output independently. Negative limits throw `InvalidArgument`;
`EncodedWordLimitExceeded(kind)` distinguishes input/output. Decoding is deferred
to `string()`, and repeated observations are deterministic. All memory is owned;
temporary buffers close on both success and failure. No I/O or native API.
Header rendering must separately enforce encoded-word field line limits.

Reference: [RFC 2047](https://www.rfc-editor.org/rfc/rfc2047.html).
