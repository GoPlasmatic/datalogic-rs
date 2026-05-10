---
name: Feature request
about: Propose a new operator, API, or capability
title: "[feature] "
labels: enhancement
---

## Use case

<!-- What problem does this solve? Concrete scenario, not "would be nice". -->

## Proposal

<!-- Sketch the surface area. For a new operator, show the JSONLogic shape. -->

```json
{ "your-op": [ ... ] }
```

For a Rust API change, sketch the signature and how it slots into
`Engine` / `Logic` / `Session`.

## Alternatives considered

<!-- What did you try? Why didn't it work? Could a custom operator
     (`Engine::builder().add_operator(...)`) cover this without changes
     to the crate? -->

## Compatibility

- [ ] This is additive (new operator / new method)
- [ ] This changes existing behaviour (would need a major-version bump)
- [ ] This affects the JSONLogic spec compatibility surface

## Anything else

<!-- Links to similar features in other engines, prior art, related issues. -->
