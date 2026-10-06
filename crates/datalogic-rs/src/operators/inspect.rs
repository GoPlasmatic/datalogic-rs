//! Type inspection operator for runtime type checking.
//!
//! The `type` operator returns a string indicating the type of a value,
//! useful for conditional logic based on data types.
//!
//! # Return Values
//!
//! | Value | Returns |
//! |-------|---------|
//! | `null` | `"null"` |
//! | `true`/`false` | `"boolean"` |
//! | `123`, `1.5` | `"number"` |
//! | `"hello"` | `"string"` |
//! | `[1, 2, 3]` | `"array"` |
//! | `{"key": "val"}` | `"object"` |
//! | ISO datetime string | `"datetime"` |
//! | Duration string | `"duration"` |
//!
//! # Special Type Detection
//!
//! The operator performs heuristic detection for datetime and duration strings:
//! - Datetime: Contains `T`, `:`, and either `Z` or `+` (ISO 8601 format)
//! - Duration: Contains time unit letters (`d`, `h`, `m`, `s`) with digits
//!
//! # Examples
//!
//! ```json
//! {"type": 42}                          // Returns: "number"
//! {"type": "hello"}                     // Returns: "string"
//! {"type": [[1, 2, 3]]}                 // Returns: "array"
//! {"type": "2024-01-15T10:30:00Z"}      // Returns: "datetime"
//! {"type": "2h30m"}                     // Returns: "duration"
//! ```
//!
//! Note the wrapping in the array example: as with every operator, a
//! bare array is the argument LIST (`{"type": [1, 2, 3]}` inspects `1`),
//! so a literal array operand must be wrapped. A `var` that resolves to
//! an array needs no wrapping.

use crate::Result;
use crate::arena::DataValue;
use crate::operators::eager::Cx;

/// `type` with no argument: `"null"`, as if it had inspected `null`.
pub(crate) fn type_of_nothing() -> &'static DataValue<'static> {
    crate::arena::singletons::singleton_type_name("null")
}

/// `type(value)`: the name of the value's type, as a static singleton.
#[inline]
pub(crate) fn type_<'a>(_cx: &mut Cx<'_, 'a>, av: &'a DataValue<'a>) -> Result<&'a DataValue<'a>> {
    // Datetime/duration object detection (e.g. {"datetime": "..."}).
    #[cfg(feature = "datetime")]
    {
        use crate::operators::datetime::sentinel_str;
        if sentinel_str(av, "datetime").is_some() {
            return Ok(crate::arena::singletons::singleton_type_name("datetime"));
        }
        if sentinel_str(av, "timestamp").is_some() {
            return Ok(crate::arena::singletons::singleton_type_name("duration"));
        }
    }

    let type_str: &'static str = match av {
        DataValue::String(s) => classify_string(s),
        other => super::type_name(other),
    };
    Ok(crate::arena::singletons::singleton_type_name(type_str))
}

/// Classify a string into "datetime" / "duration" / "string" using the
/// `type` operator's string heuristic.
#[inline]
fn classify_string(s: &str) -> &'static str {
    #[cfg(feature = "datetime")]
    {
        if s.contains('T') && s.contains(':') && (s.contains('Z') || s.contains('+')) {
            return "datetime";
        }
        if s.chars().any(|c| matches!(c, 'd' | 'h' | 'm' | 's'))
            && s.chars().any(|c| c.is_ascii_digit())
            && !s.contains(' ')
        {
            return "duration";
        }
        "string"
    }
    #[cfg(not(feature = "datetime"))]
    {
        let _ = s;
        "string"
    }
}
