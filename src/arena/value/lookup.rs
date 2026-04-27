//! Object key equality and field lookup. Both are micro-optimised: the hot
//! path here is variable resolution, so `key_eq` sidesteps the libc
//! `memcmp` call that `<&str as PartialEq>` lowers to (size-class dispatch
//! covers up to 16-byte keys), and `arena_object_lookup_field` adds a
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
    unsafe {
        match n {
            0 => true,
            1 => *ab.get_unchecked(0) == *bb.get_unchecked(0),
            2 => {
                let x = (ab.as_ptr() as *const u16).read_unaligned();
                let y = (bb.as_ptr() as *const u16).read_unaligned();
                x == y
            }
            3 => {
                let x = (ab.as_ptr() as *const u16).read_unaligned();
                let y = (bb.as_ptr() as *const u16).read_unaligned();
                x == y && *ab.get_unchecked(2) == *bb.get_unchecked(2)
            }
            4 => {
                let x = (ab.as_ptr() as *const u32).read_unaligned();
                let y = (bb.as_ptr() as *const u32).read_unaligned();
                x == y
            }
            5..=7 => {
                let x = (ab.as_ptr() as *const u32).read_unaligned();
                let y = (bb.as_ptr() as *const u32).read_unaligned();
                if x != y {
                    return false;
                }
                // Tail: read trailing u32 from the last 4 bytes (overlaps).
                let tail_off = n - 4;
                let xt = (ab.as_ptr().add(tail_off) as *const u32).read_unaligned();
                let yt = (bb.as_ptr().add(tail_off) as *const u32).read_unaligned();
                xt == yt
            }
            8 => {
                let x = (ab.as_ptr() as *const u64).read_unaligned();
                let y = (bb.as_ptr() as *const u64).read_unaligned();
                x == y
            }
            9..=16 => {
                let x = (ab.as_ptr() as *const u64).read_unaligned();
                let y = (bb.as_ptr() as *const u64).read_unaligned();
                if x != y {
                    return false;
                }
                let tail_off = n - 8;
                let xt = (ab.as_ptr().add(tail_off) as *const u64).read_unaligned();
                let yt = (bb.as_ptr().add(tail_off) as *const u64).read_unaligned();
                xt == yt
            }
            _ => ab == bb,
        }
    }
}

/// Object field lookup with length + first-byte prefilter. Hoisting
/// `target.as_bytes()` and `target_first` out of the loop lets each non-match
/// reject in a single byte compare before reaching the (already inlined)
/// `key_eq` byte loop. Empty targets fall through to `key_eq` directly,
/// which length-checks correctly.
#[inline(always)]
pub(crate) fn arena_object_lookup_field<'a>(
    pairs: &'a [(&'a str, DataValue<'a>)],
    target: &str,
) -> Option<&'a DataValue<'a>> {
    let tb = target.as_bytes();
    let tlen = tb.len();
    if tlen == 0 {
        for (k, v) in pairs {
            if k.is_empty() {
                let av_ref: &'a DataValue<'a> = unsafe { &*(v as *const DataValue<'a>) };
                return Some(av_ref);
            }
        }
        return None;
    }
    // SAFETY: tlen > 0, tb has at least 1 byte.
    let tfirst = unsafe { *tb.get_unchecked(0) };
    for (k, v) in pairs {
        let kb = k.as_bytes();
        if kb.len() != tlen {
            continue;
        }
        // SAFETY: kb.len() == tlen > 0, so kb has at least 1 byte.
        if unsafe { *kb.get_unchecked(0) } != tfirst {
            continue;
        }
        if key_eq(k, target) {
            // SAFETY: pairs is `&'a [(&'a str, DataValue<'a>)]`.
            // The cast restores the 'a lifetime that `pairs.iter()` would
            // otherwise tie to the iterator's shorter borrow.
            let av_ref: &'a DataValue<'a> = unsafe { &*(v as *const DataValue<'a>) };
            return Some(av_ref);
        }
    }
    None
}
