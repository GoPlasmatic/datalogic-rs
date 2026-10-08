# Security Policy

## Supported versions

The project ships one coordinated version across the Rust core and every
language binding, released from `main` under a single `vX.Y.Z` tag (see
[CHANGELOG.md](./CHANGELOG.md)). A security fix ships in the next release
from `main`: a patch on the current minor, or the next minor. Fixes are
not backported to earlier 5.x minors. 5.x follows semver, so code
written against an earlier 5.x compiles against the current one;
[MIGRATION.md](./MIGRATION.md) lists the behaviour changes between minors.

| Version         | Status                                                            |
|-----------------|-------------------------------------------------------------------|
| 5.8.x           | Current: receives security fixes                                  |
| 5.0 to 5.7      | Upgrade to 5.8.x to get fixes                                     |
| 4.x and earlier | End-of-life: migrate to 5.x ([MIGRATION.md](./MIGRATION.md))      |

This applies to every package shipped from this repository:

- `datalogic-rs` (crates.io)
- `@goplasmatic/datalogic-node`, `@goplasmatic/datalogic-wasm`,
  `@goplasmatic/datalogic-ui` (npm)
- `datalogic-py` (PyPI)
- `datalogic-go` (Go modules)
- `io.github.goplasmatic:datalogic` (Maven Central)
- `Goplasmatic.Datalogic` (NuGet)
- `goplasmatic/datalogic` (Packagist)
- `datalogic-c` (the C ABI: GitHub release tarballs, and the library the
  Go/JVM/.NET/PHP bindings load)

## Reporting a vulnerability

Report suspected vulnerabilities **privately**, not in a public issue or
pull request.

- Preferred: open a private report via GitHub's
  [security advisories](https://github.com/GoPlasmatic/datalogic-rs/security/advisories/new)
  ("Report a vulnerability"). This keeps the report confidential until a
  fix is available.
- If that isn't available to you, email `nharishankar@gmail.com` with
  `[datalogic-rs security]` in the subject.

Include enough to reproduce: the affected package and version (or
commit SHA), a minimal rule + data that triggers the issue, and the
impact you observed (for example a panic, a stack overflow, or an
unexpectedly unbounded run).

You should get an initial acknowledgement within **5 business days**.
We'll keep you updated as we triage the report, develop a fix, and
prepare a coordinated release. There is no bug bounty.

## Scope

datalogic-rs evaluates untrusted rules over trusted data. The engine has
no `eval`, no I/O, and no code execution beyond its compiled-in
operators (and any custom operators the host application registers). For
the guarantees and limits of that sandbox, and for guidance on running
untrusted rules safely, see the
[Security and Sandboxing](https://goplasmatic.github.io/datalogic-rs/advanced/security.html)
documentation.

In scope, for example:

- Panics, stack overflows, or undefined behavior in safe Rust paths
  triggered by rules or data.
- A way to escape the read-only data sandbox.
- Soundness bugs in the arena allocator, the custom-operator boundary,
  or any binding's FFI surface.
- Unbounded compile or evaluation time/memory driven by a crafted
  **rule**.
- Cross-binding inconsistencies that produce different evaluation
  outcomes for the same JSONLogic input.

Out of scope:

- Resource exhaustion driven purely by attacker-sized **input data**.
  The host application is responsible for bounding its inputs; this is
  documented behavior, with mitigations in the sandboxing docs.
- Vulnerabilities in user-supplied `CustomOperator` implementations.
- Behavior of dependencies (report upstream; we'll coordinate if it
  affects this repo).
- The React UI debugger (`@goplasmatic/datalogic-ui`) is a developer
  tool; XSS via rules pasted into the debugger is out of scope unless it
  escapes the sandbox.

## Disclosure

We aim to agree on a coordinated disclosure timeline and credit
reporters who wish to be named once a fix ships.
