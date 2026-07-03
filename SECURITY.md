# Security Policy

## Supported versions

Security fixes are provided for the latest release line. The project
ships a single coordinated version across the Rust core and every
language binding (see [CHANGELOG.md](./CHANGELOG.md)).

| Version         | Status                                                            |
|-----------------|-------------------------------------------------------------------|
| 5.x             | Supported — receives fixes                                        |
| 4.x and earlier | End-of-life — please migrate ([MIGRATION.md](./MIGRATION.md))     |

This applies uniformly to every package shipped from this repository:

- `datalogic-rs` (crates.io)
- `@goplasmatic/datalogic-node`, `@goplasmatic/datalogic-wasm`,
  `@goplasmatic/datalogic-ui` (npm)
- `datalogic-py` (PyPI)
- `datalogic-go` (Go modules)
- `io.github.goplasmatic:datalogic` (Maven Central)
- `Goplasmatic.Datalogic` (NuGet)
- `goplasmatic/datalogic` (Packagist)
- `datalogic-c` (in-tree C ABI, consumed by the Go/JVM/.NET/PHP bindings)

## Reporting a vulnerability

Please report suspected vulnerabilities **privately** — not in a public
issue or pull request.

- Preferred: open a private report via GitHub's
  [security advisories](https://github.com/GoPlasmatic/datalogic-rs/security/advisories/new)
  ("Report a vulnerability"). This keeps the report confidential until a
  fix is available.
- If that isn't available to you, email `nharishankar@gmail.com` with
  `[datalogic-rs security]` in the subject.

Please include enough to reproduce: the affected package and version (or
commit SHA), a minimal rule + data that triggers the issue, and the
impact you observed (for example a panic, a stack overflow, or an
unexpectedly unbounded run).

You should expect an initial acknowledgement within **5 business days**.
We'll keep you updated as the report is triaged, a fix is developed, and
a coordinated release is prepared. We don't currently run a bug bounty.

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
