# Import identity

Filesystem imports use the canonical physical pathname of the existing source
file. `owner`, `./owner.ok`, `nested/../owner` and a symlink to owner share a module
identity. Relative imports in a symlinked module resolve beside its physical
source, so relocating the link does not change its dependencies.

Configured import directories retain priority, followed by the importing source
directory and std/. Two different physical version files or isolated root copies
remain different modules, even with identical contents. Hardlink inode identity
and content deduplication are not part of this rule. Virtual module loaders keep
their own keys. Import directories configure search, not a filesystem sandbox;
explicit relative imports outside them retain their existing behavior.

The shared resolver supplies this identity to parsing, module caches and cycle
checks. A canonicalization failure reports an error at the import site instead
of falling back to a second identity. See improvements-roadmap/results/IMP-17
for the alias, cycle, version, root-isolation and CLI regression evidence.
