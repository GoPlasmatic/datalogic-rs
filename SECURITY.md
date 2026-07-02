# Security Policy

## Supported versions

Security fixes are provided for the latest released major version. The
project ships a single coordinated version across the Rust core and every
language binding (see [CHANGELOG.md](./CHANGELOG.md)).

| Version | Supported |
|---------|-----------|
| 5.x     | Yes       |
| < 5.0   | No        |

## Reporting a vulnerability

Please report suspected vulnerabilities **privately**, not in a public
issue or pull request.

- Preferred: open a private report via GitHub's
  [security advisories](https://github.com/GoPlasmatic/datalogic-rs/security/advisories/new)
  ("Report a vulnerability"). This keeps the report confidential until a fix
  is available.

Please include enough to reproduce: the affected version(s) and binding, a
minimal rule + data that triggers the issue, and the impact you observed
(for example a panic, a stack overflow, or an unexpectedly unbounded run).

## Scope

datalogic-rs evaluates untrusted rules over trusted data. The engine has no
`eval`, no I/O, and no code execution beyond its compiled-in operators (and
any custom operators the host application registers). For the guarantees and
limits of that sandbox, and for guidance on running untrusted rules safely,
see the [Security and Sandboxing](https://goplasmatic.github.io/datalogic-rs/advanced/security.html)
documentation.

Reports that are in scope include, for example: a rule that causes a panic
or stack overflow, a way to escape the read-only data sandbox, or memory
unsafety in the core. Resource exhaustion driven purely by attacker-sized
**input data** (which the host application is responsible for bounding) is
documented behavior rather than a vulnerability; see the sandboxing docs for
mitigations.

## Disclosure

We aim to acknowledge a valid report promptly, agree on a coordinated
disclosure timeline, and credit reporters who wish to be named once a fix
ships.
