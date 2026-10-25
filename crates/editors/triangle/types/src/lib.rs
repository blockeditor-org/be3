use be_block::Root;
use be_model::{Document, Model};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct Triangle {}

impl Root for Triangle {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7767_7075_2d74_7269_616e_676c_6530_3031);
}

pub type TriangleContent = Document<Triangle>;

block_type_guest::be_block_types![TriangleContent];

#[cfg(feature = "module")]
block_type_guest::type_module!(BlockTypes);
