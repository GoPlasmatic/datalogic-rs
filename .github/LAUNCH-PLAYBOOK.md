# Launch & Distribution Playbook

Maintainer-facing checklist for promoting datalogic-rs. Not user docs.
Baselines captured 2026-07-03; update them when you snapshot metrics.

Drafted launch content (blog posts, channel posts, listing PR texts) and
the external-action runbook live in `.github/launch/`, which is
deliberately untracked (gitignored): maintainer-local reference only.

## Gates: do not promote before these are true

1. ✅ (2026-07-15) Maven Central and Packagist serve the packages.
   Verified: `io.github.goplasmatic:datalogic` 5.0.1 on
   Central since 2026-07-07, `goplasmatic/datalogic` resolving on
   Packagist. Two of eight advertised install commands failing is a
   launch-killing HN comment.
2. ✅ (2026-07-15) The stale `@goplasmatic/datalogic` v4 npm package is
   gone: deprecated and removed from the registry (`npm view` returns
   404).
3. ✅ The redesigned README and restructured docs site are deployed
   (docs.yml runs on push to main).
4. ✅ (2026-07-15) GitHub Discussions is enabled with all categories
   (Announcements, Q&A, Ideas, Show and tell; the issue-template
   contact link points at Q&A).
5. ✅ (2026-07-15) `scripts/conformance-count.sh` output matches every
   quoted stat (this fixed a stale pre-refresh geomean in the crate
   README); `scripts/check-stats.sh` now guards this in CI.

## External listings (start first; longest latency)

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
  OpenFeature *provider* packages per language.
- [ ] **Awesome lists** (one PR each, after badges/examples are live):
  - Now: awesome-rust, awesome-dotnet, awesome-php,
    awesome-react-components (`datalogic-ui`), awesome-wasm.
  - After traction: awesome-nodejs (strict bar), awesome-go (wants Go
    Report Card; monorepo-subdir module may face pushback),
    awesome-python (very selective; wait for download curve).
  - Skip: awesome-selfhosted (libraries excluded).
- [ ] **lib.rs** already lists the crate (automatic from crates.io).

## Launch wave (order matters)

Week 1 (Rust channel):
- [ ] Blog post (a) or (d) published (see titles below).
- [ ] r/rust text post: "datalogic-rs v5: JSONLogic engine, 10.3 ns geomean,
  8 language bindings from one core". Lead with the one-core-many-registries
  architecture; r/rust loves release-engineering detail. Maintainer in
  comments all day.
- [ ] This Week in Rust: PR to `this-week-in-rust` (Updates from the Rust
  Community) linking the post. Submit by Tuesday for Wednesday's issue.
- [ ] users.rust-lang.org: reply on the two existing datalogic threads with
  the v5 update; one new announcement topic.

Week 2 (Show HN, the anchor):
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

Week 3+ (per-ecosystem):
- [ ] r/node post + blog (e): the safe-eval / json-logic-js-perf angle.
- [ ] Blog (c) + OpenFeature follow-through.
- [ ] r/golang, r/dotnet, r/PHP, r/java staggered weekly as each
  language's examples land; each post uses that language's snippet, not
  Rust.

## Blog titles (map to searcher intent; publish on dev.to or a Plasmatic blog, cross-post excerpts)

- (a) "json-logic-js is 80× slower than it needs to be": perf/alternative
  intent. Respectful of the reference impl; methodology + repro mandatory.
  (Pairwise 83.6× over 24 shared suites per BENCHMARK.md 2026-07-17;
  re-verify before publishing.)
- (b) "Same rule, eight runtimes: one JSONLogic engine across your whole
  stack": the positioning anchor; links the parallel examples/ folders.
- (c) "Feature flags without a flag service: flagd-compatible evaluation
  in-process": openfeature/flagd intent.
- (d) "Shipping one Rust core to nine registries in a single CI run":
  release-engineering trust piece; r/rust + HN material.
- (e) "Let users write formulas without eval(): sandboxed expressions in
  Node and Python": high-volume "safe eval alternative" searches.

## Ongoing

- Release syndication: every GitHub release auto-creates an Announcements
  discussion (wire `--discussion-category` into release.yml's release
  step); condensed notes cross-posted to dev.to. Standard footer:
  conformance stat + playground link + "Running datalogic-rs in
  production? Add yourself: <who's-using issue link>".
- Refresh BENCHMARK.md quarterly; never quote numbers older than the last
  refresh in new posts.

## Registry and release ops

Moved here from DEVELOPMENT.md, which now describes only the release
flow. Dated entries are a log, not a status: the release workflow run
for the latest `v*` tag is the source of truth.

### Open release-ops items

The one-time watch list for the first 5.0.1 release legs (added
2026-07-02) was retired after that release brought all nine registries
up. Still open:

- **JVM natives on a clean machine:** `publish-jvm`'s Maven Central
  deploy first ran with the classpath-root layout on 2026-07-07; verify
  once that the published JAR loads its bundled natives on a machine
  with no repo checkout and `datalogic.library.path` unset. Every release
  now runs that check on macOS and Windows against the JAR it built
  (`release-smoke-hosts.yml`); a Linux machine is still unchecked.
- **NuGet signing** remains unimplemented: needs org certificates and a
  signing decision (README embedding, SourceLink, and snupkg already ship).

### One-time registry / marketing ops (added 2026-07-03)

Registry state is a living figure; the release workflow run for the
latest `v*` tag is the source of truth, not this paragraph. Last
recorded check (2026-08-19, the 5.2.0 release): eight of the nine
registries served the tag (crates.io, npm ×3, PyPI, NuGet, the Go proxy,
and Maven Central, first published 2026-07-07); Packagist (registered
2026-07-03) lagged because the PHP dist push token had expired, so the
PHP leg needs `PHP_DIST_PUSH_TOKEN` rotated and `release.yml` rerun on
the tag. Done on 2026-07-03: Packagist
registration + webhook, GitHub Discussions enabled, wiki disabled. Done
on 2026-07-07: first Maven Central publish (`io.github.goplasmatic:datalogic`);
the root README's Maven row now carries the shields.io maven-central
badge. Done: Discussions categories created (Announcements, Q&A, Ideas,
Show and tell). Done on 2026-07-15: the stale v4 npm package
`@goplasmatic/datalogic` was deprecated and removed from the registry
(`npm view` now 404s); do **not** re-register or republish that name;
any new publish would resurrect its search-rank signal and split the
lineup three ways again. Still open:

- **Pin a "Who's using datalogic-rs? Add your project" thread** in the
  Show and tell Discussions category (the categories themselves exist;
  `.github/ISSUE_TEMPLATE/config.yml` already links to Q&A).
- **FUNDING.yml is intentionally absent**: add it only after enrolling
  the org (or a maintainer account) in GitHub Sponsors. A Sponsor
  button that 404s is worse than none.

## Metrics: snapshot fortnightly as comments on a pinned "Adoption metrics" issue

GitHub traffic has a 14-day retention window; capture on schedule:
`gh api repos/GoPlasmatic/datalogic-rs/traffic/views` and `/traffic/popular/referrers`.

| # | Metric | Baseline (2026-07-03) | 90-day target |
|---|--------|----------------------|---------------|
| 1 | npm weekly: -wasm / -node / -ui | ~52 / ~5 / ~77 | 500 / 120 / 200 |
| 2 | crates.io 90-day downloads | 24.2k | 35k |
| 3 | npm search rank "json-logic" & "jsonlogic" | absent / wasm #12, ui #3, node absent | node+wasm top-10 both |
| 4 | GitHub stars / referrers | 71 / none | 300 / jsonlogic.com appears |
| 5 | PyPI monthly downloads | establish at next snapshot | 10x baseline |
| 6 | Maven + NuGet + Packagist installs | 0 / unverified / 0 | nonzero + first external issue each |
| 7 | Docs/playground analytics | none (GitHub Pages has no analytics; consider GoatCounter, free for OSS, no cookies) | instrumented, trending up |
| 8 | Discussions Q&A threads / external Who's-using entries | 0 / 0 | 10 / 3 within 6 months |
