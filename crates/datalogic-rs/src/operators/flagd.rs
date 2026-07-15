//! flagd-spec operators (`feature = "flagd"`).
//!
//! Exposes:
//!
//! - [`evaluate_fractional`] — deterministic, weighted variant selection
//!   driven by murmurhash3_x86_32 of a bucketing key. Matches
//!   <https://flagd.dev/reference/custom-operations/fractional-operation/>.
//! - [`evaluate_sem_ver`] — semantic version comparison with the
//!   flagd-spec normalizations (strip `v`/`V` prefix, pad partial
//!   versions, coerce numeric input to string, ignore build metadata).
//!   Matches
//!   <https://flagd.dev/reference/custom-operations/semver-operation/>.
//!
//! Both match the algorithms shipped by every other OpenFeature flagd
//! in-process provider (Go/Java/Node/.NET/PHP/Python/Rust).
//!
//! Vendoring the hash inline (rather than depending on the abandoned
//! `murmurhash3 = "0.0.5"` crate, which uses an unsound
//! `mem::transmute::<&[u8], &[u32]>` for block reads) lets this binding
//! ship the same semantics on every target — including wasm32, where
//! `datalogic-rs` is published as `@goplasmatic/datalogic-wasm` — without
//! pulling in an unmaintained third-party dep.
//!
//! The algorithm itself is ~30 LOC of safe Rust: 4-byte little-endian
//! block reads via `u32::from_le_bytes`, the canonical MurmurHash3 mixing
//! constants, and a tail-byte fixup. Test vectors at the bottom of this
//! file pin it against the canonical SMHasher reference values.

use bumpalo::Bump;
use datavalue::DataValue;

use crate::Result;
use crate::arena::ContextStack;
use crate::engine::Engine;
use crate::node::CompiledNode;

/// MurmurHash3 x86_32. Spec-correct on every target (no unaligned reads,
/// no endianness dependency, no platform intrinsics).
fn murmurhash3_x86_32(bytes: &[u8], seed: u32) -> u32 {
    const C1: u32 = 0xcc9e_2d51;
    const C2: u32 = 0x1b87_3593;

    let mut h1 = seed;
    let block_count = bytes.len() / 4;

    // Body: 4-byte little-endian blocks.
    for i in 0..block_count {
        let start = i * 4;
        // `u32::from_le_bytes` lowers to a single unaligned little-endian
        // load on every target; on aarch64 / wasm32 / x86_64 it's the
        // same instruction the unsafe-transmute crate produced — just
        // without the UB.
        let mut k1 = u32::from_le_bytes([
            bytes[start],
            bytes[start + 1],
            bytes[start + 2],
            bytes[start + 3],
        ]);
        k1 = k1.wrapping_mul(C1);
        k1 = k1.rotate_left(15);
        k1 = k1.wrapping_mul(C2);

        h1 ^= k1;
        h1 = h1.rotate_left(13);
        h1 = h1.wrapping_mul(5).wrapping_add(0xe654_6b64);
    }

    // Tail: 0..=3 leftover bytes.
    let tail_start = block_count * 4;
    let mut k1: u32 = 0;
    let tail_len = bytes.len() - tail_start;
    if tail_len >= 3 {
        k1 ^= (bytes[tail_start + 2] as u32) << 16;
    }
    if tail_len >= 2 {
        k1 ^= (bytes[tail_start + 1] as u32) << 8;
    }
    if tail_len >= 1 {
        k1 ^= bytes[tail_start] as u32;
        k1 = k1.wrapping_mul(C1);
        k1 = k1.rotate_left(15);
        k1 = k1.wrapping_mul(C2);
        h1 ^= k1;
    }

    // Finalization.
    h1 ^= bytes.len() as u32;
    h1 ^= h1 >> 16;
    h1 = h1.wrapping_mul(0x85eb_ca6b);
    h1 ^= h1 >> 13;
    h1 = h1.wrapping_mul(0xc2b2_ae35);
    h1 ^= h1 >> 16;
    h1
}

/// Evaluate flagd's `fractional` operator.
///
/// Two shapes:
///
/// 1. **Explicit bucketing key.** First arg evaluates to a string —
///    that's the hash input. Remaining args are `[variant, weight]`
///    pairs.
/// 2. **Implicit bucketing key.** First arg evaluates to `null` (a
///    `{"var": ...}` for a missing field) or anything that isn't a
///    string (typically the first bucket-definition array). The hash
///    input is `flagKey + targetingKey` from the root context. If
///    `targetingKey` is missing, `null`, or empty, the operator returns
///    `null` — matching flagd's
///    [`core/pkg/evaluator/fractional.go`](https://github.com/open-feature/flagd/blob/main/core/pkg/evaluator/fractional.go)
///    parseFractionalEvaluationData.
///
/// Distribution algorithm matches the flagd canonical Go implementation
/// exactly: `bucket = (hash as u64 * total_weight as u64) >> 32`,
/// cumulative integer weight band lookup. Switching from the
/// float-percentage form used by the Rust contrib was load-bearing —
/// the two algorithms produce different variants for some inputs near
/// bucket boundaries and would break cross-provider conformance.
///
/// Weights default to 1 when omitted (`["red"]` is treated as
/// `["red", 1]`). Negative weights are clamped to 0.
///
/// Returns `null` on malformed input (no buckets, all weights zero,
/// missing targeting key in implicit form). The flagd evaluator
/// observes the `null` and falls back to the flag's default variant —
/// composing with `??` / `if` gives the same effect for non-flagd
/// callers.
#[inline]
pub(crate) fn evaluate_fractional<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(crate::arena::singletons::singleton_null());
    }

    // Evaluate the first arg eagerly; its shape tells us which call form
    // we're in.
    let first = engine.dispatch_node(&args[0], ctx, arena)?;

    // Match flagd Go's parseFractionalEvaluationData branching:
    //   - first is a string → explicit bucket key, args[1..] are buckets
    //   - first is nil/null → skip args[0], args[1..] are buckets, implicit key
    //   - else → all args are bucket definitions, implicit key
    let (bucket_key, distribution_args, skipped_first): (&str, &[CompiledNode], bool) =
        if let Some(s) = first.as_str() {
            (s, &args[1..], true)
        } else {
            let implicit = match implicit_bucket_key(ctx.root_input(), arena) {
                Some(k) => k,
                // Missing/null/empty targetingKey → return null and let
                // the flagd evaluator (or the caller's `??`) substitute
                // the default variant.
                None => return Ok(crate::arena::singletons::singleton_null()),
            };
            if matches!(first, DataValue::Null) {
                (implicit, &args[1..], true)
            } else {
                (implicit, args, false)
            }
        };

    if distribution_args.is_empty() {
        return Ok(crate::arena::singletons::singleton_null());
    }

    // Collect (variant, weight) pairs. flagd Go errors on malformed
    // distribution entries and returns nil from Evaluate — we mirror by
    // returning null here too rather than trying to recover.
    let mut buckets: bumpalo::collections::Vec<(&str, i64)> =
        bumpalo::collections::Vec::with_capacity_in(distribution_args.len(), arena);
    let mut total_weight: i64 = 0;
    for (i, node) in distribution_args.iter().enumerate() {
        // The first slot may already have been evaluated above (the
        // non-skipped implicit form). Reuse rather than re-dispatch.
        let v = if !skipped_first && i == 0 {
            first
        } else {
            engine.dispatch_node(node, ctx, arena)?
        };
        let arr = match v.as_array() {
            Some(a) => a,
            None => return Ok(crate::arena::singletons::singleton_null()),
        };
        if arr.is_empty() {
            return Ok(crate::arena::singletons::singleton_null());
        }
        let variant = match arr[0].as_str() {
            Some(s) => s,
            None => return Ok(crate::arena::singletons::singleton_null()),
        };
        // Weight defaults to 1 when omitted; clamp negatives to 0.
        let weight = if arr.len() >= 2 {
            arr[1].as_i64().unwrap_or(1).max(0)
        } else {
            1
        };
        total_weight += weight;
        buckets.push((variant, weight));
    }

    if buckets.is_empty() || total_weight <= 0 {
        return Ok(crate::arena::singletons::singleton_null());
    }

    let hash = murmurhash3_x86_32(bucket_key.as_bytes(), 0);
    // flagd canonical integer distribution: bucket lives in
    // [0, total_weight). The shift turns `hash * total_weight` (max
    // 2^32 * 2^31 = 2^63) into a value bounded by total_weight without
    // overflow.
    let bucket = ((hash as u64) * (total_weight as u64)) >> 32;

    let mut range_end: u64 = 0;
    for (variant, weight) in &buckets {
        range_end += *weight as u64;
        if bucket < range_end {
            // `variant` already borrows the arena-resident input string, so
            // return it directly instead of copying it back into the arena.
            return Ok(arena.alloc(DataValue::String(variant)));
        }
    }

    // Unreachable: bucket < total_weight by construction, and the loop
    // covers the full range. Return null defensively rather than panic.
    Ok(crate::arena::singletons::singleton_null())
}

/// Build the implicit bucketing key from the root context. Returns
/// `None` when `targetingKey` is missing, null, or empty — those map
/// to "fall back to default variant" in the flagd evaluator. The
/// `$flagd.flagKey` lookup tolerates a missing `$flagd` envelope (the
/// canonical flagd shape provides it, but standalone uses of the
/// operator may not).
fn implicit_bucket_key<'a>(root: &'a DataValue<'a>, arena: &'a Bump) -> Option<&'a str> {
    let targeting_key = lookup_string(root, "targetingKey")?;
    if targeting_key.is_empty() {
        return None;
    }
    let flag_key = root
        .as_object()
        .and_then(|obj| object_get(obj, "$flagd"))
        .and_then(|flagd| lookup_string(flagd, "flagKey"))
        .unwrap_or("");
    if flag_key.is_empty() {
        return Some(targeting_key);
    }
    let mut buf =
        bumpalo::collections::String::with_capacity_in(flag_key.len() + targeting_key.len(), arena);
    buf.push_str(flag_key);
    buf.push_str(targeting_key);
    Some(buf.into_bump_str())
}

/// `value.get(key)` for an object — `DataValue::Object` stores pairs as
/// a flat slice, so a small linear scan is fine for the keys we're
/// looking up (at most one or two per fractional call).
fn lookup_string<'a>(value: &'a DataValue<'a>, key: &str) -> Option<&'a str> {
    let pairs = value.as_object()?;
    object_get(pairs, key)?.as_str()
}

fn object_get<'a>(pairs: &'a [(&'a str, DataValue<'a>)], key: &str) -> Option<&'a DataValue<'a>> {
    pairs.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
}

// =============== sem_ver ===============

/// Evaluate flagd's `sem_ver` operator.
///
/// Shape: exactly three args — `[version1, op, version2]`.
///
/// Supported `op` values:
///
/// - `"="`  — strict equality
/// - `"!="` — strict inequality
/// - `"<"`, `"<="`, `">"`, `">="` — total ordering per SemVer 2.0
/// - `"^"` — same major version (caret-style "compatible")
/// - `"~"` — same major and minor (tilde-style "approximate")
///
/// Spec-compliant normalizations applied to both version inputs:
///
/// 1. Leading `v` or `V` is stripped (`"v1.2.3"` → `"1.2.3"`).
/// 2. Partial versions are padded to `major.minor.patch` (`"1"` →
///    `"1.0.0"`, `"1.2"` → `"1.2.0"`).
/// 3. Numeric inputs (`DataValue::Number`) are coerced to their string
///    representation before parsing.
/// 4. Build metadata (`+...`) is ignored — `semver::Version` already
///    drops it from comparisons per SemVer 2.0.
///
/// Returns `Null` on any malformed input (non-3-arg shape, unparseable
/// version, unknown operator) — matches the flagd providers' "graceful
/// fallback" behaviour, callers compose with `??` / `if` for defaults.
#[inline]
pub(crate) fn evaluate_sem_ver<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() != 3 {
        return Ok(crate::arena::singletons::singleton_null());
    }
    let v1_av = engine.dispatch_node(&args[0], ctx, arena)?;
    let op_av = engine.dispatch_node(&args[1], ctx, arena)?;
    let v2_av = engine.dispatch_node(&args[2], ctx, arena)?;

    // Normalize both version strings; bail to Null on parse failure.
    let v1 = match parse_version(v1_av, arena) {
        Some(v) => v,
        None => return Ok(crate::arena::singletons::singleton_null()),
    };
    let v2 = match parse_version(v2_av, arena) {
        Some(v) => v,
        None => return Ok(crate::arena::singletons::singleton_null()),
    };

    let op = match op_av.as_str() {
        Some(s) => s,
        None => return Ok(crate::arena::singletons::singleton_null()),
    };

    let result = match op {
        "=" => v1 == v2,
        "!=" => v1 != v2,
        "<" => v1 < v2,
        "<=" => v1 <= v2,
        ">" => v1 > v2,
        ">=" => v1 >= v2,
        // Caret: same major. SemVer 2.0 convention for "compatible
        // updates" within a single major. flagd's `^` is the boolean
        // form of that range check.
        "^" => v1.major == v2.major,
        // Tilde: same major + minor.
        "~" => v1.major == v2.major && v1.minor == v2.minor,
        _ => return Ok(crate::arena::singletons::singleton_null()),
    };
    Ok(crate::arena::singletons::singleton_bool(result))
}

/// Normalize a `DataValue` into a `semver::Version`. Returns `None` if
/// the input can't reasonably be coerced.
fn parse_version<'a>(value: &'a DataValue<'a>, arena: &'a Bump) -> Option<semver::Version> {
    let raw: &str = if let Some(s) = value.as_str() {
        s
    } else if let Some(n) = value.as_i64() {
        // Coerce numeric input to string — flagd-spec normalization #3.
        // `123` parses as version `123` then gets padded to `123.0.0`.
        let s = bumpalo::format!(in arena, "{}", n);
        s.into_bump_str()
    } else {
        // Float coercion is rarer but the spec covers it: `1.5` →
        // `"1.5"` → padded to `"1.5.0"`. SemVer's grammar rejects
        // scientific notation, so a pathological `1e10` falls through
        // to a parse error and we return None.
        let f = value.as_f64()?;
        let s = bumpalo::format!(in arena, "{}", f);
        s.into_bump_str()
    };

    // Strip leading v / V — flagd-spec normalization #1.
    let stripped = raw.strip_prefix(['v', 'V']).unwrap_or(raw);

    // Drop build metadata entirely — flagd-spec normalization #4. SemVer
    // 2.0 says build metadata is ignored when determining precedence,
    // but `semver::Version` keeps it on the parsed value and uses it in
    // `PartialEq`, so `1.2.3+a == 1.2.3+b` would return false unless we
    // strip it before parsing.
    let without_build = match stripped.find('+') {
        Some(i) => &stripped[..i],
        None => stripped,
    };

    // Pad partial versions: `"1"` → `"1.0.0"`, `"1.2"` → `"1.2.0"`.
    // Flag-spec normalization #2. Preserve any pre-release suffix
    // (`-alpha.1`) — split on the first `-` and pad only the numeric
    // core.
    let (core, pre_suffix) = match without_build.find('-') {
        Some(i) => (&without_build[..i], &without_build[i..]),
        None => (without_build, ""),
    };
    let dot_count = core.bytes().filter(|b| *b == b'.').count();
    let padded_owned;
    let padded: &str = match dot_count {
        0 => {
            padded_owned = bumpalo::format!(in arena, "{}.0.0{}", core, pre_suffix);
            padded_owned.into_bump_str()
        }
        1 => {
            padded_owned = bumpalo::format!(in arena, "{}.0{}", core, pre_suffix);
            padded_owned.into_bump_str()
        }
        // 2+ dots: leave the (build-metadata-stripped) form as-is. Two
        // dots is the canonical `major.minor.patch` shape; >2 is
        // malformed and `Version::parse` will reject it.
        _ => without_build,
    };

    semver::Version::parse(padded).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // SMHasher canonical test vectors for MurmurHash3_x86_32 with seed 0.
    // Cross-checked against the reference C implementation
    // (https://github.com/aappleby/smhasher/blob/master/src/MurmurHash3.cpp).
    #[test]
    fn murmurhash3_x86_32_empty_string() {
        assert_eq!(murmurhash3_x86_32(b"", 0), 0);
    }

    #[test]
    fn murmurhash3_x86_32_known_vectors() {
        // Vectors lifted from the contrib's test suite + reference
        // implementations; pinning these guarantees bucket compatibility
        // with other flagd providers.
        assert_eq!(murmurhash3_x86_32(b"hello", 0), 0x248bfa47);
        assert_eq!(murmurhash3_x86_32(b"foo", 0), 0xf6a5c420);
        assert_eq!(
            murmurhash3_x86_32(b"The quick brown fox jumps over the lazy dog", 0),
            0x2e4ff723
        );
    }

    #[test]
    fn murmurhash3_x86_32_with_seed_1() {
        // Same algorithm, non-zero seed — verifies seed propagation.
        // Cross-checked against the SMHasher reference.
        assert_eq!(murmurhash3_x86_32(b"hello", 1), 0xbb4abcad);
    }

    #[test]
    fn murmurhash3_x86_32_tail_lengths_round_trip() {
        // Exercise every tail-length residue class (0..=3 bytes left over
        // after the 4-byte block loop). No external reference for the
        // exact values — these regression-pin against the current impl
        // so future refactors don't silently change the bucketing.
        assert_eq!(murmurhash3_x86_32(b"a", 0), 0x3c2569b2);
        assert_eq!(murmurhash3_x86_32(b"ab", 0), 0x9bbfd75f);
        assert_eq!(murmurhash3_x86_32(b"abc", 0), 0xb3dd93fa);
        assert_eq!(murmurhash3_x86_32(b"abcd", 0), 0x43ed676a);
    }
}
