# Blade: projects and dependencies

Blade creates Okkama projects and manages their dependencies. The compiler
side — how imports find packages — is in spec §5.2.2.

## A project

```
app/
  blade.toml    the manifest
  blade.lock    exact installed versions (written by blade)
  src/main.ok   module app;   (no version: it is in blade.toml)
  pkg/          installed dependencies, managed by blade
```

`blade new app` creates this layout. Commit `blade.toml` and `blade.lock`;
`pkg/` can be rebuilt with `blade install`.

## blade.toml

```toml
[module]
name = "app"
version = "1.0.0"

[build]
target = "sparrow"
output = "app"

[dependencies]
web = { git = "https://example.org/web", version = "12.3.0" }
util = { path = "../util", version = "2.*" }
```

A dependency comes from exactly one source:

- `git` — a repository; its versions are its `vX.Y.Z` tags. A local
  repository works too: `git = "/home/user/pkg/web"`.
- `path` — a local package directory, relative to the manifest or absolute;
  its version is the one in its own `blade.toml`. It is copied into `pkg/`
  on every install, so edits to it show up after the next `blade install`.

`version` is a requirement:

| Requirement | Accepts |
|---|---|
| `12.3.0` | compatible: `>=12.3.0, <13.0.0` (below 1.0: `0.3.1` is `>=0.3.1, <0.4.0`) |
| `12.*`, `12.3.*` | any version with that prefix |
| `=12.3.1` | exactly that version |
| `*` | any version |

## Commands

```
blade new NAME                   create a project
blade install                    install blade.toml's dependencies into pkg/
blade install SOURCE [VERSION]   add a dependency and install it
blade install --git DIR [VERSION]  a local directory used as a git repository
blade remove NAME                remove a dependency
blade update [NAME...]           move to the newest allowed versions
```

`blade install SOURCE` names the dependency by the `[module] name` of the
package. Without `VERSION` it takes the newest version and records it as the
requirement. A SOURCE with `://`, `git@` or a `.git` suffix is a git
repository; anything else is a local directory unless `--git` is given.

Blade works from the nearest directory with `blade.toml` at or above the
current one.

In VS Code, `import "` suggests `pkg/`, then the dependencies of the file's
package, then the folders and modules of the installed package.

## Resolution and the lock

A project has one version of each package. Requirements on a package from
the project and from other packages meet at the newest version that
satisfies all of them; when none does, blade names every requirement and who
made it:

```
no version of "json" satisfies 1.* (by web 13.0.0), 2.* (by other 1.0.0); available: 1.0.0, 1.2.0
```

`blade.lock` records each installed package: version, source, git commit and
a SHA-256 of its files. `blade install` keeps the locked versions while they
still satisfy the manifests; `blade update` picks new ones. If a git tag now
points at a different commit than the lock says, or a locked package's files
changed, `blade install` stops; `blade update NAME` accepts the new state.

Installation is all or nothing: the new `pkg/` is assembled as
`pkg.blade-new/` and swapped in only when every package was fetched and
checked. A failed `blade install SOURCE` also leaves `blade.toml` unchanged.
`pkg.blade-busy` marks a running blade.

## What a package must be

Blade refuses a package that breaks any of these rules:

- it has `blade.toml` with `[module] name` equal to its dependency name and a
  `version` equal to the installed version (for git — the tag);
- its module files declare no version, and their module names are the
  package name or start with `name/`;
- it declares no `native` actions or objects — natives come from the Okkama
  runtime;
- it contains plain files only (no symbolic links) and no two files whose
  names differ only in letter case.

A package's own `pkg/` and `.git` are not installed: all dependencies of a
project, direct and transitive, live side by side in the project's `pkg/`.
