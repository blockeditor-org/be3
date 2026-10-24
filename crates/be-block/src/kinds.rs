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
    BeuiDemo,
    BeuiDemoContent,
    0x6265_7569_2d64_656d_6f2d_626c_6f63_6b31
);
