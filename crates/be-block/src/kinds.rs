use be_model::{Document, Model};
use uuid::Uuid;

use crate::Root;

macro_rules! kind {
    ($name:ident, $content:ident, $id:literal) => {
        #[derive(Clone, Debug, Default, Model, PartialEq)]
        pub struct $name {}

        impl Root for $name {
            const CONTENT_TYPE: Uuid = Uuid::from_u128($id);
        }

        pub type $content = Document<$name>;
    };
}

kind!(
    FileTree,
    FileTreeContent,
    0x6669_6c65_2d74_7265_652d_626c_6f63_6b01
);
kind!(
    PanZoom,
    PanZoomContent,
    0x7061_6e5f_7a6f_6f6d_2d62_6c6f_636b_0001
);
kind!(
    Scene3d,
    Scene3dContent,
    0x3364_2d73_6365_6e65_2d62_6c6f_636b_3031
);
kind!(
    WorkspaceUi,
    WorkspaceUiContent,
    0x776f_726b_7370_6163_652d_7569_2d30_3031
);
