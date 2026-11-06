# Qualified export checking (experimental)

The opt-in prototype in improvements-roadmap/workspace/module-api uses the current
resolver and module#major::Name declaration identities. A public RemoteState in
one module no longer collides with a private RemoteState elsewhere. Public
signatures are traversed as resolved Type AST, including generic arguments and
bounds, action types, roles, successors and explicitly exported caps. Importer
permissions expose names only; object fields remain private.

SUPPORTED.md describes supported forms. Unknown/native public surfaces fail;
unpolicied legacy modules produce explicit unchecked diagnostics. The policy is
not installed by default. Physical import identity is the ordinary compiler rule
(documented in import-identity.md); package export policy remains experimental.
Exact source-pinned checks and A/B provider substitution: ../improvements-roadmap/
results/IMP-25/RESULT.md. Frozen research sources were not changed.
