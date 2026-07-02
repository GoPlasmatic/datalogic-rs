//! Size regression guards for the types every evaluation moves around.
//!
//! These are the measured all-features sizes on 64-bit as of 5.1.0; the
//! bounds are ceilings, not exact matches, so unrelated refactors don't
//! trip them. If one fails, a variant or field grew a hot type — box the
//! payload instead of raising the bound, unless the growth is deliberate.

#![cfg(target_pointer_width = "64")]

use datalogic_rs::datavalue::{DataValue, OwnedDataValue};
use std::mem::size_of;

#[test]
fn data_value_stays_small() {
    // Arena value: 24 bytes (tag + inline payload/slice).
    assert!(
        size_of::<DataValue>() <= 24,
        "DataValue grew: {}",
        size_of::<DataValue>()
    );
}

#[test]
fn owned_data_value_stays_small() {
    assert!(
        size_of::<OwnedDataValue>() <= 32,
        "OwnedDataValue grew: {}",
        size_of::<OwnedDataValue>()
    );
}

#[test]
fn error_stays_thin() {
    // Every operator returns Result<&DataValue, Error>; Error's size sets
    // the width of that return slot (R3 item 8 tracks shrinking it).
    assert!(
        size_of::<datalogic_rs::Error>() <= 80,
        "Error grew: {}",
        size_of::<datalogic_rs::Error>()
    );
    assert!(
        size_of::<datalogic_rs::Result<()>>() <= 80,
        "Result<()> grew: {}",
        size_of::<datalogic_rs::Result<()>>()
    );
}
