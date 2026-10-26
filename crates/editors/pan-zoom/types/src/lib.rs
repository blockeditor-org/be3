use be_block::Root;
use be_model::{Document, Model};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct PanZoom {}

impl Root for PanZoom {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7061_6e5f_7a6f_6f6d_2d62_6c6f_636b_0001);
}

pub type PanZoomContent = Document<PanZoom>;

block_type_guest::be_block_types![PanZoomContent];

#[cfg(feature = "module")]
block_type_guest::type_module!(BlockTypes);
