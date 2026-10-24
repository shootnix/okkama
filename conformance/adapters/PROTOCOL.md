# Adapter protocol `ocs-adapter/1`

An adapter is any executable that lets the runner drive one Okkama
implementation. The runner is written in Python only as a portable baseline; an
adapter may be written in any language and the implementation is never linked
into the runner.

## Configuration

`adapters/<name>/adapter.json` (schema `ocs-adapter-config/1`):

```json
{
  "schema": "ocs-adapter-config/1",
  "protocol": "ocs-adapter/1",
  "name": "okkama-perl",
  "command": ["perl", "{adapter_dir}/okkama_perl_adapter.pl"],
  "env": {"OCS_PERL_IMPLEMENTATION_ROOT": "{suite_root}/../okkama-pl"},
  "pass_env": ["PATH", "PERL5LIB", "PERL_LOCAL_LIB_ROOT", "PERLBREW_*"]
}
```

* `{adapter_dir}`, `{suite_root}` and `{host_home}` are expanded by the runner
  (`{host_home}` lets launch variables locate host toolchains such as
  `~/.rustup` while the program still runs with an isolated `HOME`).
* `env` sets launch variables; `pass_env` lists host variables the
  implementation needs in order to start (they also become visible to the
  program, so cases never assert them).
* `ocs run --adapter-env KEY=VALUE` overrides launch variables, e.g. to point at
  another checkout. This is the **only** thing that changes between
  implementations: case sources and expectations stay identical.

## Invocations

```
<command> --info
<command> --request REQUEST.json --response RESPONSE.json
```

`--info` prints one JSON object describing the implementation (name, revision,
configuration, supported modes). It is recorded in every run summary.

For a case the runner starts the command with:

* working directory = the case's isolated `work/` directory;
* stdin = the case's stdin file or `/dev/null`;
* stdout/stderr captured by the runner — the adapter MUST pass the program's
  standard streams through unchanged and MUST NOT write anything else to
  stdout (adapter diagnostics go to the response file or to stderr only for
  failing cases);
* environment = the isolated case environment plus the adapter's launch
  variables.

### Request (`schema/adapter-request.schema.json`)

```json
{
  "protocol": "ocs-adapter/1",
  "mode": "run",
  "sources": ["/tmp/ocs-x/work/main.ok", "/tmp/ocs-x/work/part.ok"],
  "args": ["alpha", ""],
  "cwd": "/tmp/ocs-x/work",
  "stdlib_root": "/path/to/std",
  "timeout_s": 30.0
}
```

The request carries no case id and no expectation. `sources[0]` is the entry
file; several sources form one multi-file module.

### Response (`schema/adapter-response.schema.json`)

```json
{
  "protocol": "ocs-adapter/1",
  "adapter": {"name": "okkama-perl", "version": "1"},
  "mode": "run",
  "outcome": "failed",
  "stage_reached": "runtime",
  "failure": {
    "stage": "runtime",
    "category": "uncaught-error",
    "error_type": "NotFound",
    "message": "not found: x",
    "raw": "..."
  },
  "elapsed_ms": 183
}
```

`outcome` is one of:

| outcome | when | required fields | adapter process exit |
|---|---|---|---|
| `completed` | run/test mode finished | `exit_status` 0..255 | equal to `exit_status` |
| `accepted` | parse/check mode accepted the sources | — | 0 |
| `rejected` | parse or check stage rejected the program | `failure.stage` ∈ {parse, check}, `failure.category` | any |
| `failed` | a runtime failure ended the program | `failure.stage` = runtime, `failure.category` | nonzero |
| `internal-error` | the implementation itself broke (not a language failure) | `failure` with raw detail | any |
| `unsupported` | the implementation cannot perform the mode | `unsupported_reason` | any |

`failure.category` uses the logical categories of SPECIFICATION.md §6
(`syntax`, `semantic`, `resolution`, `runtime`, `panic`, `uncaught-error`).
`unclassified` is allowed but will not match any expectation.

The runner rejects (HARNESS_ERROR) any response with a wrong protocol or mode,
an unknown outcome, a verdict-like key (`verdict`, `pass`, `status`), a
completed exit status that disagrees with the adapter's process exit code, a
runtime failure reported with process exit 0, or an acceptance in run/test
mode. A missing response file is HARNESS_ERROR, or CRASH if the process was
killed by a signal.

## Trust boundary

Adapters translate protocols; they do not decide conformance. In particular an
adapter MUST NOT: alter, wrap or pre-process sources; add imports, mains or
harness code; repair invalid entry points; retry; emulate missing language or
library behaviour; read `case.json` or expected files. Raw requests,
responses, commands and streams are retained for audit, and adapter code is
part of the hashed suite tree.

## Provided adapters

### `okkama-perl` (Perl reference interpreter)

`adapters/perl/okkama_perl_adapter.pl` loads `okkama-pl/lib` and calls the same
public entry object as `bin/okkama` (`Okkama->main(@files, @args)`, or
`-t` for test mode), with argv re-encoded to UTF-8 bytes exactly as the
operating system would deliver them. It wraps `Okkama::parse_files` and
`Okkama::Analyzer::analyze` only to record *whether they completed*, which
attributes a failure to the parse, check or runtime stage. Failure carriers
are mapped to categories (Panic → panic, Thrown → uncaught-error with the
diagnostic type name, Runtime error → runtime, Semantic error → semantic,
import/native link messages → resolution, Perl `at X.pm line N` → internal
error). Diagnostics are rendered to stderr exactly as the CLI does.

### `okkama-rust` (developing Rust implementation)

`adapters/rust/okkama_rust_adapter.py` builds `okkama-rs` into
`adapters/rust/.target` (never inside the implementation tree) and asks the
binary for `--ocs-capabilities`. It forwards only modes the binary declares,
using the same protocol natively (`--ocs-request R --ocs-response P`). The
workspace builds the `okkama` binary (`okkama-cli`). `OCS_RUST_ENGINE` selects
`ir` (default), `sparrow`, or `jvm`; use `--adapter-env OCS_RUST_ENGINE=sparrow`
to obtain an independent Sparrow report. IR and Sparrow declare `parse`,
`check`, and `run`; JVM also declares `test`. Sparrow execution includes
code generation and SPB encode/decode before running the VM. JVM explicitly
compiles temporary class files and starts `java`. There is no engine fallback.
Unsupported constructs are reported as `unsupported`, runtime traps as
`failed`, and invalid internal state as `internal-error`. The selected engine
is recorded in adapter configuration and participates in the capability cache.
The probe
fingerprint covers the embedded JVM runtime classes (`*.class`) as well as the
Rust sources.

## Writing a new adapter

1. Implement `--info` and the request/response cycle above.
2. Map your implementation's failure carriers to logical categories and stages
   honestly; anything you cannot classify is `unclassified` or
   `internal-error`.
3. Declare unsupported modes with `outcome: unsupported` and a reason.
4. Run `python3 runner-tests/test_runner.py` (protocol self-tests) and
   `./ocs mutate --adapter adapters/<name>/adapter.json`.
