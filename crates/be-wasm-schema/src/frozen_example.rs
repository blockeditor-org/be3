//! A frozen type, for the tests: `schema.lock` numbers its fields out of source order (it was
//! frozen as `y`, `x`, and `label` came after), which the tests check the encoding follows.

be_wasm_schema::frozen!();

use crate::Schema;

#[derive(Debug, PartialEq, Schema)]
pub struct FrozenPoint {
    pub x: i32,
    pub y: i32,
    pub label: Option<String>,
}
