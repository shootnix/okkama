# Application, configuration and CLI contract

Stage 24 is implemented as explicit capabilities. Configuration and CLI policy
stay in Okkama; native terminal operations expose owned, validated capabilities.

A SettingKey is a nonempty immutable sequence of exact nonempty segments.
Dots, case, underscores and Unicode normalization have no implicit meaning.
`string()` is diagnostic; comparison uses the segments. Defaults: 64 segments,
4096 scalars per key, 4096 entries and 1024 values per entry. Values are strings;
typed conversion uses existing strings/parse objects. A source identifies the
winning entry. Later layers replace its complete values, preserving first key
appearance order. Empty and absent values remain distinct.

Environment mappings are explicit EnvironmentBinding objects, in declaration
order. Duplicate target keys are rejected even when variables are absent.
Document adapters use the existing nominal JSON/TOML/INI visitor protocols.
File opening, decoding and budgets remain with the caller.

Argument objects snapshot exact argument boundaries. UTF-8 decoding is separate
per argument; failures retain zero-based argument index and byte offset. No
shell expansion, re-tokenization or quote interpretation occurs.

Command specifications are immutable and validated before parsing. Parsing
returns an Invocation containing the chosen command, command path and Settings.
It performs no I/O, callback, implicit help/version action or process exit.

## Configuration

`SettingReading` is a confined reading capability: membership, entry and keys.
`Settings(entries)` rejects duplicate segment identities; `LayeredSettings(layers)`
keeps the first key position and the last complete entry, including its source.
`Setting`, `ScalarSetting` and `ScalarSettingOr` observe a supplied reading role.
Scalar observations require exactly one value. The fallback applies only to an
absent key and has a defaults source. MissingSetting, DuplicateSetting,
NonScalarSetting, InvalidSettingKey and SettingLimitExceeded remain typed errors.

Settings retains a stable index of actual SettingKey values and original
positions. Duplicate validation uses sorted adjacent comparisons; lookup uses
binary search. keys() retains declaration order, and the earliest repeated
original key is still the DuplicateSetting payload. Dotted diagnostic spelling
never replaces exact segment identity. Environment target validation uses the
same index before observing variables. LayeredSettings updates a persistent AVL index, copying one O(log n) path per
entry; final publication materializes original insertion order once in
O(n log n). Replacements retain the first position and replace the complete
values/provenance. At the 4096-key limit a replacement remains valid; a new
identity fails before the next source entry is observed. This complexity bound
is not a latency promise for the maximum quota.

The construction helper SettingOverlay also implements the confined
SettingReading protocol. membership/entry query the tree directly, keys() keeps
insertion order, and entry() reports MissingSetting for absence. Its immutable
replaced(entry) leaves earlier versions intact. CLI value options use this
validated reading surface rather than exporting/revalidating a full Settings
snapshot for every token; Invocation still publishes ordinary Settings.

`EnvironmentSettings(environment, bindings, source)` accepts an ordered sequence
of `EnvironmentBinding(name,key)`. Environment already supplies strict Unicode
snapshots; mapped names use its platform normalization. No process mutation or
implicit prefix/case/dot policy is introduced.

`JsonSettings(document,source)`, `TomlSettings(document,source)` and
`IniSettings(document,source)` use existing document objects. Strings remain decoded
text, JSON null is `null`, booleans use `true`/`false`, and other scalars use their
canonical format text. Arrays become repeated values; empty arrays remain present
with zero values. Empty tables create no entries. JSON/TOML traversal is bounded
at 64 nodes deep; key segments remain literal, in insertion order. INI uses exactly
two nonempty segments, section and entry. An empty/default INI section therefore
needs an explicit application mapping rather than an invented key convention.
Nominal custom printers must emit exactly one value. Visitors latch swallowed
protocol failures and seal before publishing; silent, double or late emissions
cannot publish a writable or partial snapshot.

## Arguments and commands

`ProcessArguments()` uses the existing provider argv observation. Embedders supply
`process_arguments`; the executable supplies its remaining arguments after source
selection. Its historical runner may consume additional existing `.ok` files as
source files. Do not insert a separator before options: a literal `--` is passed
through and intentionally terminates recognition in ParsedCommand.

The explicit constructor accepts `Sequencing<@byte>`; its final two arguments can
replace the default 4096 argument / 1 MiB aggregate byte budgets. Sources are read
by size once and item once in index order. `Utf8Arguments` independently decodes
these snapshots and exposes ordinary Cursor/Sequencing semantics. Invalid UTF-8
reports zero-based index and byte offset; quotes, spaces and shell metacharacters
are literal data.

A `CommandSyntax(options,flags,operands)` snapshots at most 1024 declarations.
Option aliases are unique, short names are one visible ASCII character and long
names match `--[a-z0-9][a-z0-9-]*`. Each value option consumes one value. Only
explicit distinct flag alternatives may share a key. Required operands precede
optional operands, and a zero-or-more repeated operand is last. Branch commands
have options/flags and children, with no operands; their first non-option token
must select a child. Ancestor aliases stop at child selection. The tree is limited
to 32 levels / 4096 nodes and 1024 children per node.

CliCommand validates unique child names through a separate sorted name list.
Its children remain in declaration order for parsing, help and completion; case
remains exact. CommandSyntax prepares a sorted exact-alias index and segment-key validation
indexes. option_index(name)/flag_index(name) return the original index in their
own declaration group or -1 when absent. Parsing reuses this immutable lookup,
including after selecting a child. Validation retains the original first-error
priority: declaration order, aliases before keys, then operands with their
original duplicate/repeated/required ordering. Index preparation costs
O(A log A + D log D) comparisons for A aliases and D declarations; alias lookup
costs O(log A). These bounds do not promise maximum-quota latency.

`ParsedCommand(command,arguments).value()` accepts exact long names with a separate
value or `=`, including empty values and further equals signs. Short flags cluster;
a value option must be the final character and consumes the next argv item (even
when that value starts with a dash). An unmatched finite negative numeric token
is an operand. `--` affects all remaining argv, including child commands. Scalar
options cannot repeat; repeated option values preserve order. Later alternative
flags replace the previous declared value. Missing/extra operands, unknown names,
flag assignments and incomplete branch paths throw CliFailure(index,token,message).
No implicit help/version bypass or required-configuration check is performed.

`Usage(command,program,width).string()` produces stable plain text in declaration
order, with every alias, placeholder and repetition marker. Width must be at least
eight cells; wrapping preserves text and breaks only between grapheme clusters.
`ShellCompletion(command,program,shell)` emits a static union of declared aliases
and child names for bash/zsh/fish. It does not select context, suggest operand
values, call the application or inspect files. Bash/zsh literals are quoted directly;
fish option aliases use -l/-s metadata, and its deferred child-name expressions are quoted twice. Empty specifications remain
valid. Shell completion installation is the caller's choice.

## Terminal ownership

`OpenedStandardTerminal(output,policy[,environment]).session()` and
`OpenedControllingTerminal(policy[,environment]).session()` are async acquisitions.
Each owns separate descriptors/handles, leaving the selected original open. A
session exclusively controls its POSIX descriptor identities or Windows console
devices while open; do not mix its reads/modes with ambient standard input actions.
Standard input and selected stdout/stderr interactivity are independent. A
controlling terminal opens `/dev/tty` or CONIN$/CONOUT$, independently of redirected
standard streams. Its absence is TerminalUnavailable.

Session input/output are borrowed. Closing/dropping the parent revokes children,
wakes its pending reader and restores outstanding modes and screen changes.
`TerminalInput.bytes(limit)` is async, bounded to 1..65536 bytes.
`line(limit)` is async and incrementally bounded to 1..1048576 bytes before LF;
a preceding CR counts toward the bound and is removed from the result. Remaining
bytes after a newline are retained for the next read. EOF can return a final line.
Invalid UTF-8 stays typed; no whitespace trimming occurs. Overflow closes the
session so a truncated line cannot be resumed as a successful next line.

TerminalOutput implements Stage07 Writing/WritingText, including `<-`, StyledText,
write/writeln/flush. Host writes are bounded to 64 KiB and report a prefix;
IncompleteWrite retains the complete source's accepted prefix. Large sources are
split through ByteBuffer, with no implicit flush/retry after a short write. Flush
is an observation/no-op for unbuffered output. Interactive Windows console output
requires strict UTF-8 and uses WriteConsoleW; redirected output preserves bytes.

`TerminalModeApplying` exposes only apply(mode). `AppliedTerminalMode(...).lease()`
applies cooked/cbreak/raw/secret (cooked with all echo disabled) mode. Cooked and
secret input accept Enter through CR-to-LF processing even inside raw mode;
cbreak preserves terminal signal processing. POSIX cooked/secret profiles retain
the acquisition's other control characters, speeds and preferences, and restore
output postprocessing. Closing restores the complete immediately preceding
snapshot, including an outer raw lease. Explicit closes
must follow LIFO; rejecting an out-of-order close retains its authority. Automatic
drops can defer restoration until nested leases end. Session close restores every
remaining lease. Neither termios masks nor console flags cross the std API.

`TerminalScreenApplying` exposes clear, move, display_cursor and
use_alternate_screen. Coordinates are zero-based and bounded. Screen control
requires interactive, VT-capable output regardless of ColorPolicy.always.
Escape construction and cleanup instructions are private std actions. Closing
restores a visible cursor and leaves the alternate screen; host delivery errors
are reported after mode/descriptor restoration is attempted.

`TerminalResizeOpening` supplies a confined TerminalResizeReading capability.
`TerminalResizes(session).size()` lazily acquires its source, starts with current
geometry and then reports/coalesces resize observations. Closing/dropping the
wrapper closes its source; parent closure revokes it. POSIX uses an owned wake pipe
and preserved/chained SIGWINCH. The existing native poll endpoint contract is
supported; Windows uses scheduler timers and has no select-compatible descriptor.

`PromptedLine(session,prompt,limit).string()` flushes before awaiting a line.
`PromptedSecret` requires a controlling terminal and guards no-echo. Normal return,
error, cancellation and panic restore modes. Failed/cancelled secret prompts close
the session and never return partial secret input. Ordinary cancelled line reads
release their lock/readiness and leave the session reusable.

## Display and color profile

Width is a deterministic Unicode16 profile, not a promise about every terminal's
font. Combining/default-ignorable characters are zero alone, W/F characters are
wide, ambiguous characters are narrow, emoji presentation/ZWJ/flags/keycaps form
one two-cell cluster. C0/C1, DEL and line/paragraph separators are rejected by
DisplayWidth; WrappedDisplay handles explicit LF separately. TruncatedDisplay
preserves graphemes and charges its ellipsis only when needed. ANSI bytes never
participate: StyledText keeps plain text and style separately.

ColorPolicy.never always disables styling. Automatic requires selected output TTY,
native VT capability, no NO_COLOR variable (including empty) and TERM!=dumb.
Always permits style sequences on redirected output. TERM/COLORTERM select a depth;
validated COLUMNS/LINES are fallback geometry only. Session color decisions and
interactivity remain pinned; geometry is observed afresh. Indexed colors require
0..255, RGB components are bytes, and downgrading uses a fixed cube/grayscale/basic
palette with deterministic distance ties. Every styled value ends a complete reset.

## Platform validation and limits

POSIX uses POSIX::Termios, nonblocking owned descriptors, scheduler readiness and
IO::Tty1.31 for geometry and PTY tests. Shared file-description flags are restored
on close. Darwin can set kernel-managed PENDIN after returning to canonical input;
tests compare all user-controlled coordinates and account for that transient bit.
Windows uses lazy FFI::Platypus2.12/Win32API::File, guarded console modes, UTF-16
input events and Unicode output. Its minimal cooked handling supports line/erase/
echo; non-text keyboard events are ignored. Windows console input and viewport
observation poll through 10ms scheduler timers; unavailable nonblocking redirected
output pipes reject with unsupported. Small regular-file calls and console output
can still have host latency. Live Windows validation requires a Windows runner;
Mac validation covers the fake API contract and real POSIX integration separately.

Primary references: [Unicode16 width](https://www.unicode.org/reports/tr11/tr11-43.html),
[Perl POSIX](https://perldoc.perl.org/POSIX),
[Apple tty implementation](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/tty.c),
[Windows console modes](https://learn.microsoft.com/windows/console/setconsolemode),
[Windows console input](https://learn.microsoft.com/en-us/windows/console/low-level-console-input-functions),
[Win32API::File handles](https://metacpan.org/pod/Win32API%3A%3AFile),
[bash completion](https://www.gnu.org/s/bash/manual/html_node/Programmable-Completion.html),
[zsh completion](https://zsh.sourceforge.io/Doc/Release/Completion-System.html),
[fish completion](https://fishshell.com/docs/current/cmds/complete.html).

### Persistent settings updates

SettingOverlay performs lookup and path copying in one AVL traversal. An update
returns the new node and whether a distinct identity was added; replacements keep
the first position and replace the complete entry and source. At capacity4096,
replacements remain valid and a new key fails at its missing leaf before the next
source is observed. Existing versions and publication order are preserved. The
maximum quota is a cardinality bound, not a latency guarantee; see the
single-traversal audit.
