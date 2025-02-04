use serde_json::Value;

pub mod error;
pub use error::*;

pub mod optype;
pub use optype::*;

pub mod coercion;
pub use coercion::*;

pub mod arithmetic;
pub use arithmetic::*;

pub mod comparison;
pub use comparison::*;

pub mod control;
pub use control::*;

pub mod string;
pub use string::*;

pub mod array;
pub use array::*;

pub mod var;
pub use var::*;

pub mod logic;
pub use logic::*;

pub mod missing;
pub use missing::*;

pub type EvalResult = Result<Value, Error>;
