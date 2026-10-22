# Security Policy

## Supported versions

The project ships one coordinated version across the Rust core and every
language binding. Releases are cut from `main` under a single `vX.Y.Z` tag (see
[CHANGELOG.md](./CHANGELOG.md)). Security fixes ship in the next release from
`main` (either a patch on the current minor or the next minor release) and are
not backported to earlier 5.x minors. Because 5.x follows SemVer, code written
against an earlier 5.x release compiles against the current one. See
[MIGRATION.md](./MIGRATION.md) for behavior changes between minor versions.

| Version         | Status                                                            |
|-----------------|-------------------------------------------------------------------|
| 5.9.x           | Current: receives security fixes                                  |
| 5.0 to 5.8      | Upgrade to 5.9.x to get fixes                                     |
| 4.x and earlier | End-of-life: migrate to 5.x ([MIGRATION.md](./MIGRATION.md))      |

This policy applies to every package shipped from this repository:

- [`datalogic-rs`](./crates/datalogic-rs/README.md) (crates.io)
- [`@goplasmatic/datalogic-node`](./bindings/node/README.md), [`@goplasmatic/datalogic-wasm`](./bindings/wasm/README.md),
  and [`@goplasmatic/datalogic-ui`](./ui/README.md) (npm)
- [`datalogic-py`](./bindings/python/README.md) (PyPI)
- [`datalogic-go`](./bindings/go/README.md) (Go modules)
- [`io.github.goplasmatic:datalogic`](./bindings/jvm/README.md) (Maven Central)
- [`Goplasmatic.Datalogic`](./bindings/dotnet/README.md) (NuGet)
- [`goplasmatic/datalogic`](./bindings/php/README.md) (Packagist)
- [`datalogic-c`](./bindings/c/README.md) (the C ABI in GitHub release tarballs, loaded by the Go, JVM, .NET, and PHP bindings)

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

`datalogic-rs` evaluates untrusted rules over trusted data. The engine has
no `eval`, no I/O, and no code execution beyond its compiled-in
operators and any custom operators registered by the host application. For
sandbox guarantees, resource limits, and guidance on running untrusted rules
safely, see the
[Security and Sandboxing](https://goplasmatic.github.io/datalogic-rs/advanced/security.html)
and [Operation Budget](https://goplasmatic.github.io/datalogic-rs/advanced/operation-budget.html)
guides (source in [`docs/src/advanced/security.md`](./docs/src/advanced/security.md)
and [`docs/src/advanced/operation-budget.md`](./docs/src/advanced/operation-budget.md)).

In scope, for example:

- Panics, stack overflows, or undefined behavior in safe Rust paths
  triggered by rules or data.
- A way to escape the read-only data sandbox.
- Soundness bugs in the arena allocator, the custom-operator boundary,
  or any binding's FFI surface (see [ARCHITECTURE.md](./ARCHITECTURE.md)).
- Unbounded compile or evaluation time or memory driven by a crafted
  **rule**.
- Cross-binding inconsistencies that produce different evaluation
  outcomes for the same JSONLogic input.

Out of scope:

- Resource exhaustion driven purely by attacker-sized **input data**.
  The host application is responsible for bounding its inputs. This is
  documented behavior, with mitigations in the [Security and Sandboxing](./docs/src/advanced/security.md) docs.
- Vulnerabilities in user-supplied `CustomOperator` implementations.
- Behavior of upstream dependencies (report upstream; we will coordinate if it
  affects this repository).
- The React UI debugger ([`@goplasmatic/datalogic-ui`](./ui/README.md)) is a developer
  tool. Self-XSS via rules pasted into the debugger is out of scope unless it
  escapes the sandbox.

## Disclosure

We aim to agree on a coordinated disclosure timeline and credit
reporters who wish to be named once a fix ships. For non-security bugs or questions, see [CONTRIBUTING.md](./CONTRIBUTING.md) and [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md).
