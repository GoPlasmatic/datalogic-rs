//! Re-export of the arena-allocated value type plus the helpers built
//! around it.
//!
//! `DataValue` is just `datavalue::DataValue` — the two crates share a
//! single bump-allocated value type. Helpers (truthiness, coercion,
//! traversal, lookup, conversion at the serde_json boundary) stay in
//! datalogic-rs because they're tied to engine config / op semantics.

mod coercion;
mod conversion;
mod lookup;
mod strings;
mod traversal;

#[cfg(feature = "datetime")]
pub(crate) use coercion::coerce_to_number;
pub(crate) use coercion::{coerce_to_number_cfg, parse_finite, try_coerce_to_integer_cfg};
#[cfg(feature = "serde_json")]
pub(crate) use conversion::data_to_value;
#[cfg(feature = "serde_json")]
pub(crate) use conversion::value_to_data;
pub(crate) use lookup::{ORDERED_PROBE_MIN_PAIRS, object_lookup_field, object_lookup_field_hinted};
pub(crate) use strings::{data_to_str, truthy_arena};
pub(crate) use traversal::apply_path_element;
pub(crate) use traversal::{
    access_path_str_ref, path_exists_segments, path_exists_str, traverse_segments,
};

pub use datavalue::DataValue;
