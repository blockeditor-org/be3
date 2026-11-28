use be_block_content::Root;
use be_block_model::{Anchor, Change, Document, Edit, List, Model, ObjectId, Text};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct Graph {
    pub equations: List<Equation>,
}

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct Equation {
    pub expression: Text,
}

impl Graph {
    pub fn add(expression: &str) -> Edit {
        Self::EQUATIONS
            .insert(
                ObjectId::ROOT,
                Anchor::End,
                &Equation { expression: expression.into() },
            )
            .1
            .into()
    }

    pub fn remove(id: ObjectId) -> Edit {
        Change::remove(id).into()
    }

    pub fn move_after(id: ObjectId, after: Option<ObjectId>) -> Edit {
        let anchor = after.map_or(Anchor::Start, Anchor::After);
        Self::EQUATIONS.move_into(ObjectId::ROOT, anchor, id).into()
    }
}

impl Root for Graph {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7095_6c4a_1353_4814_abb7_9f59_63ab_39de);
}

pub type GraphContent = Document<Graph>;

be_sdk_type_guest::be_block_types![GraphContent { undo }];

#[cfg(feature = "module")]
be_sdk_type_guest::type_module!(BlockTypes);
