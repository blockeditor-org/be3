use be_block::Root;
use be_block_model::{Document, Model};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct BeuiDemo {}

impl Root for BeuiDemo {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6265_7569_2d64_656d_6f2d_626c_6f63_6b31);
}

pub type BeuiDemoContent = Document<BeuiDemo>;

be_sdk_type_guest::be_block_types![BeuiDemoContent];

#[cfg(feature = "module")]
be_sdk_type_guest::type_module!(BlockTypes);
