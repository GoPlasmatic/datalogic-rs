# Security policy

## Supported versions

Security fixes are issued for the latest minor of the current major line.

| Version | Status                            |
|---------|-----------------------------------|
| 5.x     | Supported — receives fixes        |
| 4.x     | End-of-life — please migrate; see [MIGRATION.md](../MIGRATION.md) |

This applies uniformly to every package shipped from this repo:

- `datalogic-rs` (crates.io)
- `@goplasmatic/datalogic-wasm`, `@goplasmatic/datalogic-node`,
  `@goplasmatic/datalogic-ui` (npm)
- `datalogic-py` (PyPI)
- `datalogic-go` (Go modules)

## Reporting a vulnerability

**Do not file a public GitHub issue for security reports.**

Please use GitHub's [private vulnerability reporting](https://github.com/GoPlasmatic/datalogic-rs/security/advisories/new)
to submit a report. If that isn't available to you, email
`nharishankar@gmail.com` with `[datalogic-rs security]` in the subject.

A useful report includes:

- The affected package and version (or commit SHA).
- A minimal JSONLogic rule + data input that triggers the issue, if
  applicable.
- The observed behavior and the impact you believe it has (e.g., DoS via
  unbounded memory, panic in safe code, sandbox escape from a custom
  operator).

You should expect an initial acknowledgement within **5 business days**.
We'll keep you updated as the report is triaged, a fix is developed,
and a coordinated release is prepared. We don't currently run a bug
bounty.

## Scope

In scope:

- Panics or undefined behavior in safe Rust paths.
- Soundness bugs in the arena allocator, custom operator boundary, or
  any binding's FFI surface.
- DoS via crafted rules or data (unbounded compile/evaluate time or
  memory).
- Cross-binding inconsistencies that result in different evaluation
  outcomes for the same JSONLogic input.

Out of scope:

- Vulnerabilities in user-supplied `CustomOperator` implementations.
- Behavior of dependencies (please report to the dependency upstream;
  we'll coordinate if it affects this repo).
- The React UI debugger (`@goplasmatic/datalogic-ui`) is a developer
  tool; XSS via rules pasted into the debugger is not in scope unless
  it escapes the iframe sandbox.
