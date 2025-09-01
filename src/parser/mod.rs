pub mod jsonlogic;
#[cfg(test)]
mod tests;

// Re-export the JSONLogic parsing functions for convenient access
pub use jsonlogic::{
    parse_jsonlogic, parse_jsonlogic_json, parse_jsonlogic_json_with_preserve,
    parse_jsonlogic_with_preserve,
};
