# Launch & Distribution Playbook

Maintainer-facing checklist for promoting datalogic-rs. Not user docs.
Baselines captured 2026-07-03; update them when you snapshot metrics.

## Gates — do not promote before these are true

1. Maven Central and Packagist actually serve the packages (see the
   registry-ops checklist in [DEVELOPMENT.md](../DEVELOPMENT.md)). Two of
   eight advertised install commands failing is a launch-killing HN
   comment.
2. The stale `@goplasmatic/datalogic` v4 npm package is deprecated.
3. The redesigned README and restructured docs site are deployed
   (docs.yml runs on push to main).
4. GitHub Discussions is enabled (Q&A category exists; the issue-template
   contact link points at it).
5. `scripts/conformance-count.sh` output matches every quoted stat.

## External listings (start first — longest latency)

- [ ] **jsonlogic.com implementations list**: PR against jwadhams'
  json-logic site repo (find via the site footer's GitHub link) adding
  datalogic-rs and its bindings to the supported-languages section. Cite
  the conformance battery. Fallback: email the maintainer.
- [ ] **json-logic GitHub org**: the org already mirrors/forks this repo
  (github.com/json-logic/datalogic-rs). Open a discussion/issue asking to
  be listed in their compatibility matrix / README. This org is where the
  community spec effort lives; being listed there is durable SEO.
- [ ] **OpenFeature / flagd**: post in flagd's GitHub Discussions and CNCF
  Slack `#openfeature`: datalogic implements flagd's `fractional` +
  `sem_ver` with byte-compatible murmur3 bucketing across 8 runtimes
  (including PHP/.NET/Java where in-process options are thin). Ask how
  compatible evaluation engines get listed. Longer-term unlock: shipping
  actual OpenFeature *provider* packages per language.
- [ ] **Awesome lists** (one PR each, after badges/examples are live):
  - Now: awesome-rust, awesome-dotnet, awesome-php,
    awesome-react-components (`datalogic-ui`), awesome-wasm.
  - After traction: awesome-nodejs (strict bar), awesome-go (wants Go
    Report Card; monorepo-subdir module may face pushback),
    awesome-python (very selective; wait for download curve).
  - Skip: awesome-selfhosted (libraries excluded).
- [ ] **lib.rs** already lists the crate (automatic from crates.io).

## Launch wave (order matters)

Week 1 — Rust channel:
- [ ] Blog post (a) or (d) published (see titles below).
- [ ] r/rust text post: "datalogic-rs v5 — JSONLogic engine, ~10 ns geomean,
  8 language bindings from one core". Lead with the one-core-many-registries
  architecture; r/rust loves release-engineering detail. Maintainer in
  comments all day.
- [ ] This Week in Rust: PR to `this-week-in-rust` (Updates from the Rust
  Community) linking the post. Submit by Tuesday for Wednesday's issue.
- [ ] users.rust-lang.org: reply on the two existing datalogic threads with
  the v5 update; one new announcement topic.

Week 2 — Show HN (the anchor):
- [ ] Submit **the playground URL** (Show HN guidelines favor something
  people can try): title
  `Show HN: One JSONLogic engine for 8 languages (Rust core, ~10 ns/eval)`.
- [ ] Prepared first comment: what it is, why one core (drift between
  ports), benchmark table + repro command, honest limits (rules are
  data-plane only; WASM is 88x slower than native; resource bounding is
  the host's job), link to comparison page.
- [ ] Pre-written answers for: vs CEL / vs ZEN/GoRules / vs OPA; why
  JSONLogic at all; `#![forbid(unsafe_code)]`; WASM bundle size;
  DoS/resource bounding; who uses it in production (point to Who's-using
  section); license/monetization (Apache-2.0, Plasmatic uses it in its
  own products).
- [ ] Tue–Thu, 8–10 AM ET; maintainer available 6+ hours.

Week 3+ — per-ecosystem:
- [ ] r/node post + blog (e): the safe-eval / json-logic-js-43x angle.
- [ ] Blog (c) + OpenFeature follow-through.
- [ ] r/golang, r/dotnet, r/PHP, r/java staggered weekly as each
  language's examples land; each post uses that language's snippet, not
  Rust.

## Blog titles (map to searcher intent; publish on dev.to or a Plasmatic blog, cross-post excerpts)

- (a) "json-logic-js is 43x slower than it needs to be" — perf/alternative
  intent. Respectful of the reference impl; methodology + repro mandatory.
- (b) "Same rule, eight runtimes: one JSONLogic engine across your whole
  stack" — the positioning anchor; links the parallel examples/ folders.
- (c) "Feature flags without a flag service: flagd-compatible evaluation
  in-process" — openfeature/flagd intent.
- (d) "Shipping one Rust core to nine registries in a single CI run" —
  release-engineering trust piece; r/rust + HN material.
- (e) "Let users write formulas without eval(): sandboxed expressions in
  Node and Python" — high-volume "safe eval alternative" searches.

## Ongoing

- Release syndication: every GitHub release auto-creates an Announcements
  discussion (wire `--discussion-category` into release.yml's release
  step); condensed notes cross-posted to dev.to. Standard footer:
  conformance stat + playground link + "Running datalogic-rs in
  production? Add yourself: <who's-using issue link>".
- Refresh BENCHMARK.md quarterly; never quote numbers older than the last
  refresh in new posts.

## Metrics — snapshot fortnightly as comments on a pinned "Adoption metrics" issue

GitHub traffic has a 14-day retention window; capture on schedule:
`gh api repos/GoPlasmatic/datalogic-rs/traffic/views` and `/traffic/popular/referrers`.

| # | Metric | Baseline (2026-07-03) | 90-day target |
|---|--------|----------------------|---------------|
| 1 | npm weekly: -wasm / -node / -ui | ~52 / ~5 / ~77 | 500 / 120 / 200 |
| 2 | crates.io 90-day downloads | 24.2k | 35k |
| 3 | npm search rank "json-logic" & "jsonlogic" | absent / wasm #12, ui #3, node absent | node+wasm top-10 both |
| 4 | GitHub stars / referrers | 71 / — | 300 / jsonlogic.com appears |
| 5 | PyPI monthly downloads | establish at next snapshot | 10x baseline |
| 6 | Maven + NuGet + Packagist installs | 0 / unverified / 0 | nonzero + first external issue each |
| 7 | Docs/playground analytics | none (GitHub Pages has no analytics; consider GoatCounter, free for OSS, no cookies) | instrumented, trending up |
| 8 | Discussions Q&A threads / external Who's-using entries | 0 / 0 | 10 / 3 within 6 months |
