//! Object key equality and field lookup. Both are micro-optimised: the hot
//! path here is variable resolution, so `key_eq` sidesteps the libc
//! `memcmp` call that `<&str as PartialEq>` lowers to (size-class dispatch
//! covers up to 16-byte keys), and `object_lookup_field` adds a
//! length + first-byte prefilter so non-matching pairs reject in a single
//! byte compare before reaching the byte loop.

use super::DataValue;

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

/// Object field lookup with length + first-byte prefilter. Hoisting
/// `target.as_bytes()` and `target_first` out of the loop lets each non-match
/// reject in a single byte compare before reaching the (already inlined)
/// `key_eq` byte loop. Empty targets fall through to `key_eq` directly,
/// which length-checks correctly.
#[inline(always)]
pub(crate) fn object_lookup_field<'a>(
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
