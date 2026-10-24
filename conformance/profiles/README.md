# Profiles

Profiles are fixed selections of cases by level, mode, environment tags and case
status (`schema/profile.schema.json`).

| Profile | Kind | Selection |
|---|---|---|
| `bootstrap` | milestone | L00 with stdout/stderr only |
| `frontend` | milestone | every parse/check-mode case; a frontend pass is never language conformance |
| `portable-core` | release member | L00–L08, L10 and L12 cases needing only standard streams, stdin, argv |
| `portable-stdlib` | release member | L09, L12, L13 cases needing only streams and pinned data |
| `platform-resources` | release member | cases needing files, processes, environment, worker processes, real clocks or randomness |
| `controlled-services` | release member | cases needing loopback networking, TLS or SQLite |
| `full-release` | release gate | union of the four members, plus coverage and unresolved-contract checks |
| `informative` | informative | unresolved and informative cases; never counted |

`ocs validate` fails if any mandatory case is selected by no release member.
