use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Pdf {}

impl Pdf {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum PdfOperation {}

impl Block for Pdf {
    type Operation = PdfOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7064_662d_626c_6f63_6b2d_7479_7065_2d01);

    fn apply_operation(_pdf: &mut Self, operation: &Self::Operation) {
        match *operation {}
    }
}

#[cfg(test)]
mod tests;
