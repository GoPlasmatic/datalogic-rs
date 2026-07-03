//! Object key equality and field lookup. Both are micro-optimised: the hot
//! path here is variable resolution, so `key_eq` sidesteps the libc
//! `memcmp` call that `<&str as PartialEq>` lowers to (size-class dispatch
//! covers up to 16-byte keys), and `object_lookup_field` adds a
//! length + first-byte prefilter so non-matching pairs reject in a single
//! byte compare before reaching the byte loop.
//!
//! Wide objects additionally get an *optimistic ordered probe*: a binary
//! search that is trusted only when it lands on a byte-verified key match,
//! with a full linear-scan fallback otherwise. Sortedness is **not** an
//! invariant of `DataValue::Object`: the serde_json ingestion path emits
//! key-sorted pairs (`serde_json::Map` is a `BTreeMap` under default
//! features), but `DataValue::from_str` keeps document order. The probe
//! therefore never concludes *absence*, only presence.

use core::cmp::Ordering;

use super::DataValue;

/// Pair count at or above which [`object_lookup_field`] attempts the
/// optimistic ordered probe before the linear scan. Below this the
/// prefiltered linear scan wins: a probe is a byte-wise key compare plus a
/// data-dependent branch (~7 of them at 128 pairs), while a prefilter
/// rejection is a length compare on sequential memory.
const ORDERED_PROBE_MIN_PAIRS: usize = 32;

/// Byte-wise lexicographic compare without the libc `memcmp` call that
/// `<[u8] as Ord>::cmp` lowers to. Probe keys are short JSONLogic field
/// names, so the call/trampoline overhead would dominate each descent
/// step, the same rationale as [`key_eq`]. Measured on the 131-pair
/// macro object: inlining this shaved ~15% off descent-bound lookups.
#[inline(always)]
fn key_cmp(a: &[u8], b: &[u8]) -> Ordering {
    let n = a.len().min(b.len());
    for i in 0..n {
        if a[i] != b[i] {
            return a[i].cmp(&b[i]);
        }
    }
    a.len().cmp(&b.len())
}

/// Optimistic binary search over `pairs` by byte-wise key order.
///
/// Soundness does not depend on `pairs` being sorted: a `Some` is returned
/// only after the landing key compares byte-equal to `target`, and `None`
/// means "not proven present", so the caller must still linear-scan,
/// because on unsorted pairs the descent can walk past a present key.
///
/// On byte-sorted pairs (the serde_json ingestion path) this finds the key
/// in O(log n). Duplicate keys: sorted duplicates are adjacent, and the
/// left-walk below returns the first of the run, matching the linear scan's
/// first-match semantics. Unsorted duplicates (reachable only via
/// `DataValue::from_str` on a document that repeats a key) can land on a
/// later occurrence than the linear scan would return; JSON leaves
/// duplicate-key semantics unspecified and the two ingestion paths already
/// disagree there (serde_json keeps the last occurrence, the arena parser
/// keeps all).
#[inline]
fn ordered_probe<'a>(
    pairs: &'a [(&'a str, DataValue<'a>)],
    target: &str,
) -> Option<&'a DataValue<'a>> {
    let tb = target.as_bytes();
    let mut lo = 0usize;
    let mut hi = pairs.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        match key_cmp(pairs[mid].0.as_bytes(), tb) {
            Ordering::Less => lo = mid + 1,
            Ordering::Greater => hi = mid,
            Ordering::Equal => {
                // Byte-verified hit. Walk left over an adjacent run of the
                // same key so sorted-with-duplicates objects keep the linear
                // scan's first-match answer.
                let mut i = mid;
                while i > 0 && key_eq(pairs[i - 1].0, target) {
                    i -= 1;
                }
                return Some(&pairs[i].1);
            }
        }
    }
    None
}

/// Inline byte-equality for object keys vs. lookup targets. Sidesteps the
/// libc `memcmp` call that `<&str as PartialEq>::eq` lowers to — for the
/// short keys typical in JSONLogic data, the call/trampoline overhead
/// dominates the actual byte compare. Size-class dispatch keeps each arm
/// down to a couple of instructions; longer keys fall back to slice eq.
#[inline(always)]
pub(crate) fn key_eq(a: &str, b: &str) -> bool {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    if ab.len() != bb.len() {
        return false;
    }
    let n = ab.len();
    match n {
        0 => true,
        1 => ab[0] == bb[0],
        2 => {
            let x = u16::from_ne_bytes(ab[..2].try_into().unwrap());
            let y = u16::from_ne_bytes(bb[..2].try_into().unwrap());
            x == y
        }
        3 => {
            let x = u16::from_ne_bytes(ab[..2].try_into().unwrap());
            let y = u16::from_ne_bytes(bb[..2].try_into().unwrap());
            x == y && ab[2] == bb[2]
        }
        4 => {
            let x = u32::from_ne_bytes(ab[..4].try_into().unwrap());
            let y = u32::from_ne_bytes(bb[..4].try_into().unwrap());
            x == y
        }
        5..=7 => {
            let x = u32::from_ne_bytes(ab[..4].try_into().unwrap());
            let y = u32::from_ne_bytes(bb[..4].try_into().unwrap());
            if x != y {
                return false;
            }
            // Tail: read trailing u32 from the last 4 bytes (overlaps).
            let xt = u32::from_ne_bytes(ab[n - 4..].try_into().unwrap());
            let yt = u32::from_ne_bytes(bb[n - 4..].try_into().unwrap());
            xt == yt
        }
        8 => {
            let x = u64::from_ne_bytes(ab[..8].try_into().unwrap());
            let y = u64::from_ne_bytes(bb[..8].try_into().unwrap());
            x == y
        }
        9..=16 => {
            let x = u64::from_ne_bytes(ab[..8].try_into().unwrap());
            let y = u64::from_ne_bytes(bb[..8].try_into().unwrap());
            if x != y {
                return false;
            }
            let xt = u64::from_ne_bytes(ab[n - 8..].try_into().unwrap());
            let yt = u64::from_ne_bytes(bb[n - 8..].try_into().unwrap());
            xt == yt
        }
        _ => ab == bb,
    }
}

/// Wide-object lookup: optimistic [`ordered_probe`], then the same linear
/// scan narrow objects use (a probe miss cannot prove absence because
/// pairs are not guaranteed sorted). Hits on sorted wide objects (every
/// serde_json-ingested payload) drop from O(n) to O(log n); unsorted wide
/// objects pay ~log2(n) extra key compares on top of the unchanged scan.
///
/// `cold` + `inline(never)`: `object_lookup_field` is `inline(always)`
/// into every traversal loop, and keeping the wide path a bare call keeps
/// the narrow-object codegen (register allocation included) at one
/// predictable length compare over what it was before the probe existed.
#[cold]
#[inline(never)]
fn wide_lookup<'a>(
    pairs: &'a [(&'a str, DataValue<'a>)],
    target: &str,
) -> Option<&'a DataValue<'a>> {
    if let Some(v) = ordered_probe(pairs, target) {
        return Some(v);
    }
    linear_lookup(pairs, target)
}

/// The prefiltered linear scan shared by narrow and wide lookups. Hoisting
/// `target.as_bytes()` and `target_first` out of the loop lets each
/// non-match reject in a single byte compare before reaching the (already
/// inlined) `key_eq` byte loop. Empty targets fall through to `key_eq`
/// directly, which length-checks correctly.
#[inline(always)]
fn linear_lookup<'a>(
    pairs: &'a [(&'a str, DataValue<'a>)],
    target: &str,
) -> Option<&'a DataValue<'a>> {
    let tb = target.as_bytes();
    let tlen = tb.len();
    if tlen == 0 {
        for (k, v) in pairs {
            if k.is_empty() {
                return Some(v);
            }
        }
        return None;
    }
    // tlen > 0 here (the empty-target branch returned above), so the
    // bounds-checked first-byte read folds away in release.
    let tfirst = tb[0];
    for (k, v) in pairs {
        let kb = k.as_bytes();
        if kb.len() != tlen {
            continue;
        }
        // kb.len() == tlen > 0, so kb[0] is in bounds — bounds check elided.
        if kb[0] != tfirst {
            continue;
        }
        if key_eq(k, target) {
            return Some(v);
        }
    }
    None
}

/// Object field lookup. Narrow objects (the overwhelmingly common case)
/// take the inline [`linear_lookup`] scan exactly as before; wide objects
/// (>= [`ORDERED_PROBE_MIN_PAIRS`] pairs) divert to the out-of-line
/// [`wide_lookup`], whose ordered probe makes hits O(log n) when the pairs
/// happen to be sorted. The dispatch compare is free-ish: the scan needs
/// `pairs.len()` anyway.
#[inline(always)]
pub(crate) fn object_lookup_field<'a>(
    pairs: &'a [(&'a str, DataValue<'a>)],
    target: &str,
) -> Option<&'a DataValue<'a>> {
    if pairs.len() >= ORDERED_PROBE_MIN_PAIRS {
        return wide_lookup(pairs, target);
    }
    linear_lookup(pairs, target)
}

/// Hinted object field lookup for homogeneous-row iteration loops — the
/// arena analog of a monomorphic inline cache. The caller keeps a pair
/// index across rows; when consecutive rows share one key layout (the
/// overwhelmingly common case for real data sources), every row after the
/// first resolves in a single `key_eq`. A hint miss falls back to
/// [`object_lookup_field`] and re-learns the position.
#[inline(always)]
pub(crate) fn object_lookup_field_hinted<'a>(
    pairs: &'a [(&'a str, DataValue<'a>)],
    target: &str,
    hint: &mut usize,
) -> Option<&'a DataValue<'a>> {
    if let Some((k, v)) = pairs.get(*hint) {
        if key_eq(k, target) {
            return Some(v);
        }
    }
    let idx = pairs.iter().position(|(k, _)| key_eq(k, target))?;
    *hint = idx;
    Some(&pairs[idx].1)
}

#[cfg(test)]
mod tests {
    use bumpalo::Bump;

    use super::*;

    /// Build `n` `("kNNN", String("vNNN"))` pairs in ascending key order.
    fn sorted_pairs<'a>(arena: &'a Bump, n: usize) -> Vec<(&'a str, DataValue<'a>)> {
        (0..n)
            .map(|i| {
                let k: &str = arena.alloc_str(&format!("k{i:03}"));
                let v: &str = arena.alloc_str(&format!("v{i:03}"));
                (k, DataValue::String(v))
            })
            .collect()
    }

    fn lookup_str<'a>(pairs: &'a [(&'a str, DataValue<'a>)], target: &str) -> Option<&'a str> {
        object_lookup_field(pairs, target).and_then(|v| v.as_str())
    }

    #[test]
    fn wide_sorted_hits_and_misses() {
        let arena = Bump::new();
        let mut pairs = sorted_pairs(&arena, 128);
        pairs.push(("list_a", DataValue::Bool(true)));
        pairs.push(("nested", DataValue::Bool(false)));
        assert_eq!(lookup_str(&pairs, "k000"), Some("v000"));
        assert_eq!(lookup_str(&pairs, "k064"), Some("v064"));
        assert_eq!(lookup_str(&pairs, "k127"), Some("v127"));
        assert_eq!(
            object_lookup_field(&pairs, "nested").and_then(DataValue::as_bool),
            Some(false)
        );
        // Misses before the first key, between keys, and after the last key.
        assert_eq!(lookup_str(&pairs, "a"), None);
        assert_eq!(lookup_str(&pairs, "k0640"), None);
        assert_eq!(lookup_str(&pairs, "zzz"), None);
    }

    #[test]
    fn wide_unsorted_hits_and_misses() {
        // Reverse order defeats the ordered probe everywhere; the linear
        // fallback must still find every key.
        let arena = Bump::new();
        let mut pairs = sorted_pairs(&arena, 128);
        pairs.reverse();
        for i in [0usize, 1, 63, 64, 126, 127] {
            let key = format!("k{i:03}");
            let want = format!("v{i:03}");
            assert_eq!(lookup_str(&pairs, &key), Some(want.as_str()));
        }
        assert_eq!(lookup_str(&pairs, "k128"), None);
        assert_eq!(lookup_str(&pairs, ""), None);
    }

    #[test]
    fn threshold_boundary_unsorted() {
        // Exactly ORDERED_PROBE_MIN_PAIRS pairs, unsorted: probe runs, may
        // miss, fallback answers.
        let arena = Bump::new();
        let mut pairs = sorted_pairs(&arena, ORDERED_PROBE_MIN_PAIRS);
        pairs.swap(0, ORDERED_PROBE_MIN_PAIRS - 1);
        assert_eq!(lookup_str(&pairs, "k000"), Some("v000"));
        assert_eq!(lookup_str(&pairs, "k031"), Some("v031"));
        assert_eq!(lookup_str(&pairs, "missing"), None);
    }

    #[test]
    fn wide_sorted_adjacent_duplicates_return_first() {
        // Sorted objects keep duplicate keys adjacent; the probe's left-walk
        // must return the first of the run, matching the linear scan.
        let arena = Bump::new();
        let mut pairs = sorted_pairs(&arena, 64);
        pairs.insert(33, ("k032", DataValue::String("dup")));
        assert_eq!(lookup_str(&pairs, "k032"), Some("v032"));
        // A run of three behaves the same.
        pairs.insert(34, ("k032", DataValue::String("dup2")));
        assert_eq!(lookup_str(&pairs, "k032"), Some("v032"));
    }

    #[test]
    fn wide_unsorted_scattered_duplicates_stay_key_correct() {
        // Scattered duplicates in unsorted pairs: which occurrence wins is
        // unspecified (see `ordered_probe` docs), but the value must belong
        // to the requested key.
        let arena = Bump::new();
        let mut pairs = sorted_pairs(&arena, 64);
        pairs.reverse();
        pairs.push(("k010", DataValue::String("dup")));
        let got = lookup_str(&pairs, "k010");
        assert!(got == Some("v010") || got == Some("dup"), "got {got:?}");
    }

    #[test]
    fn key_cmp_orders_bytewise() {
        use core::cmp::Ordering::{Equal, Greater, Less};
        assert_eq!(key_cmp(b"", b""), Equal);
        assert_eq!(key_cmp(b"a", b"b"), Less);
        assert_eq!(key_cmp(b"ab", b"a"), Greater); // shared prefix: longer sorts after
        assert_eq!(key_cmp(b"k010", b"k010"), Equal);
        assert_eq!(key_cmp(b"k2", b"k10"), Greater); // byte-wise, not numeric
    }

    #[test]
    fn empty_key_on_wide_object() {
        let arena = Bump::new();
        let mut pairs = vec![("", DataValue::Bool(true))];
        pairs.extend(sorted_pairs(&arena, 63));
        assert_eq!(
            object_lookup_field(&pairs, "").and_then(DataValue::as_bool),
            Some(true)
        );
    }
}
