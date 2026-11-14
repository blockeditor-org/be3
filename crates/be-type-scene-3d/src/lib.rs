use be_block_content::Root;
use be_block_model::{Document, Model};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct Scene3d {}

impl Root for Scene3d {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x3364_2d73_6365_6e65_2d62_6c6f_636b_3031);
}

pub type Scene3dContent = Document<Scene3d>;

be_sdk_type_guest::be_block_types![Scene3dContent];

#[cfg(feature = "module")]
be_sdk_type_guest::type_module!(BlockTypes);
