//! `data_to_str` (arena-resident `&str` rendering) + truthiness on
//! `DataValue`. Both produce arena-resident strings so chained
//! string-building operators (`cat`, `substr`, …) avoid heap traffic.
//!
//! `DataValue → JSON String` is *not* implemented here — `datavalue`'s
//! native `Display` impl emits JSON via its SWAR-driven emitter, and
//! `value.to_string()` is the canonical entry point.

use bumpalo::Bump;
use datavalue::NumberValue;

use super::DataValue;

/// Render an `DataValue` as a `&'a str` allocated in the arena (or borrowed
/// when already a string). Produces arena-resident strings so string-building
/// operators (cat, substr) can chain without heap traffic.
pub(crate) fn data_to_str<'a>(v: &DataValue<'a>, arena: &'a Bump) -> &'a str {
    match v {
        DataValue::String(s) => s,
        DataValue::Null => "",
        DataValue::Bool(true) => "true",
        DataValue::Bool(false) => "false",
        // Integers: itoa renders every i64 byte-identically to `Display`
        // without the `core::fmt` machinery (stack buffer, single memcpy
        // into the arena).
        DataValue::Number(NumberValue::Integer(i)) => {
            arena.alloc_str(itoa::Buffer::new().format(*i))
        }
        DataValue::Number(NumberValue::Float(f)) => float_to_str(*f, arena),
        // Composite types: serialize as JSON via `datavalue`'s native
        // `Display` emitter. Rare path of unbounded length, so keep the
        // amortized heap `String` build + single exact-size arena copy.
        other => arena.alloc_str(&other.to_string()),
    }
}

/// Render a float into the arena, byte-identical to `NumberValue`'s
/// `Display` (`datavalue` >= 0.2.3):
///
/// - NaN / infinities print `"null"`.
/// - Whole values exactly representable as i64 print `"{f as i64}.0"`,
///   rendered here via itoa, which matches i64 `Display` exactly. The
///   range guard mirrors `datavalue`'s `f64_as_i64_exact`: strict `<` on
///   the high side because `i64::MAX as f64` rounds up to 2^63, which the
///   cast would saturate one off (`>=` is exact on the low side, i64::MIN
///   being -2^63). Whole values outside that range print via `{:?}`
///   (scientific for these magnitudes, e.g. `1e300`), matching the
///   Display fix that landed in 0.2.3 — 0.2.2 saturated them.
/// - Fractional values print via `f64`'s `Display` (shortest round-trip,
///   always positional notation). ryu also emits shortest round-trip
///   digits, so when its output is positional and needs at most 15
///   significant digits the representation is provably unique and the two
///   must agree byte-for-byte (interval width < grid spacing below 16
///   digits). At 16-17 digits multiple shortest candidates can exist and
///   ryu/std are known to disagree on the last digit (e.g.
///   `1436244512748976.3` vs `...976.2`), and scientific-range values
///   (`|f| < 1e-5` after the integral filter) render positionally in std
///   but scientifically in ryu; both cases fall back to the `fmt` path.
///
/// Parity is enforced by `float_display_parity` below across a spicy
/// corpus plus randomized sweeps, and by the differential property test.
fn float_to_str(f: f64, arena: &Bump) -> &str {
    if f.is_nan() || f.is_infinite() {
        return "null";
    }
    if f.fract() == 0.0 {
        if f >= i64::MIN as f64 && f < i64::MAX as f64 {
            let digits_buf = &mut itoa::Buffer::new();
            let digits = digits_buf.format(f as i64);
            let mut buf = bumpalo::collections::String::with_capacity_in(digits.len() + 2, arena);
            buf.push_str(digits);
            buf.push_str(".0");
            return buf.into_bump_str();
        }
        use std::fmt::Write as _;
        let mut buf = bumpalo::collections::String::with_capacity_in(24, arena);
        let _ = write!(&mut buf, "{f:?}");
        return buf.into_bump_str();
    }
    let ryu_buf = &mut ryu::Buffer::new();
    let s = ryu_buf.format_finite(f);
    if ryu_matches_display(s) {
        arena.alloc_str(s)
    } else {
        // Rare: scientific notation or 16+ significant digits. Keep the
        // `fmt` path so the output stays byte-identical to `Display`.
        use std::fmt::Write as _;
        let mut buf = bumpalo::collections::String::with_capacity_in(24, arena);
        let _ = write!(&mut buf, "{f}");
        buf.into_bump_str()
    }
}

/// True iff a ryu-rendered float string is provably identical to what
/// `f64`'s `Display` would produce: positional notation (no exponent) and
/// at most 15 significant digits, where the shortest round-trip
/// representation is unique. See [`float_to_str`] for the argument.
#[inline]
fn ryu_matches_display(s: &str) -> bool {
    let mut sig = 0usize;
    let mut seen_nonzero = false;
    for &b in s.as_bytes() {
        match b {
            b'e' | b'E' => return false,
            b'1'..=b'9' => {
                seen_nonzero = true;
                sig += 1;
            }
            b'0' if seen_nonzero => sig += 1,
            _ => {}
        }
    }
    sig <= 15
}

/// Config-aware truthiness for `DataValue`.
///
/// `#[inline(always)]` because the function ends up inside the per-iteration
/// general path of every quantifier/filter — outlining was paying a real call
/// per item even though the hot branch is just the JS/Python default.
#[inline(always)]
pub(crate) fn truthy_arena(v: &DataValue<'_>, engine: &crate::Engine) -> bool {
    use crate::config::TruthyEvaluator;
    match &engine.config().truthy_evaluator {
        TruthyEvaluator::JavaScript | TruthyEvaluator::Python => super::truthy_js_arena(v),
        TruthyEvaluator::StrictBoolean => match v {
            DataValue::Null => false,
            DataValue::Bool(b) => *b,
            _ => true,
        },
        TruthyEvaluator::Custom(f) => f(&v.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference rendering: `NumberValue`'s `Display`, which is what the
    /// pre-itoa/ryu code paid `core::fmt` for on every conversion.
    fn reference(n: NumberValue) -> String {
        format!("{n}")
    }

    fn assert_parity_i64(i: i64) {
        let arena = Bump::new();
        let v = DataValue::Number(NumberValue::Integer(i));
        assert_eq!(
            data_to_str(&v, &arena),
            reference(NumberValue::Integer(i)),
            "integer parity broke for {i}"
        );
    }

    fn assert_parity_f64(f: f64) {
        let arena = Bump::new();
        let v = DataValue::Number(NumberValue::Float(f));
        assert_eq!(
            data_to_str(&v, &arena),
            reference(NumberValue::Float(f)),
            "float parity broke for {f:?} (bits {:#x})",
            f.to_bits()
        );
    }

    /// Minimal xorshift so the randomized sweeps need no dev-dependency.
    struct XorShift(u64);
    impl XorShift {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
    }

    #[test]
    fn integer_display_parity() {
        for i in [
            i64::MIN,
            i64::MIN + 1,
            -10,
            -1,
            0,
            1,
            42,
            i64::MAX - 1,
            i64::MAX,
        ] {
            assert_parity_i64(i);
        }
        let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
        for _ in 0..100_000 {
            assert_parity_i64(rng.next() as i64);
        }
    }

    #[test]
    fn float_display_parity() {
        // Spicy corpus: signed zero, integral floats (".0" arm and its
        // exact-i64-range guard), notation boundaries, subnormals,
        // non-finite, and the two known std-vs-ryu last-digit divergences.
        let corpus: &[f64] = &[
            0.0,
            -0.0,
            1.0,
            -1.0,
            1.5,
            -1.5,
            0.1,
            0.3,
            123.456,
            1e300,
            -1e300,
            // The i64-exactness boundary: 2^63 (first whole float past
            // i64, the old saturating cast printed it one off), the
            // largest float below it (2^63 - 1024, still exact), exactly
            // -2^63 (= i64::MIN, exact), and the first whole float below
            // that.
            9223372036854775808.0,
            9223372036854774784.0,
            -9223372036854775808.0,
            -9223372036854777856.0,
            1e-7,
            -1e-7,
            1e-5,
            1e-6,
            9.999999999999999e-6,
            0.30000000000000004,
            123456789012345680000.0,
            4503599627370495.5,
            -4503599627370495.5,
            4503599627370496.0,
            1e15 + 0.5,
            1e16,
            f64::MAX,
            f64::MIN,
            f64::MIN_POSITIVE,
            5e-324,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            // std prints ...976.3 / ...797.3 while raw ryu prints ...2;
            // the significant-digit gate must route these to `fmt`.
            f64::from_bits(0x4314_6906_eff8_d6c1),
            f64::from_bits(0xc304_0c93_6651_e66a),
        ];
        for &f in corpus {
            assert_parity_f64(f);
        }

        let mut rng = XorShift(0xD1B5_4A32_D192_ED03);
        // Human-scale decimals (mantissa / 10^k): the population `cat`
        // actually coerces; all take the ryu fast path.
        for _ in 0..100_000 {
            let m = (rng.next() % 1_000_000_000_000) as f64;
            let k = (rng.next() % 9 + 1) as i32;
            let sign = if rng.next() & 1 == 0 { 1.0 } else { -1.0 };
            assert_parity_f64(m / 10f64.powi(k) * sign);
        }
        // Raw bit patterns: exercises non-finite, subnormal, scientific
        // range, and the 16-17-digit fallback.
        for _ in 0..100_000 {
            assert_parity_f64(f64::from_bits(rng.next()));
        }
        // Dense walk just below 2^52, where every fractional value needs
        // 16-17 significant digits and must fall back to `fmt`.
        let base = 4503599627370496.0f64.to_bits();
        for i in 0..100_000u64 {
            assert_parity_f64(f64::from_bits(base - 1 - i * 87));
        }
    }
}
