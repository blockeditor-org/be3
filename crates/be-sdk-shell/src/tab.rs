use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TabItem {
    pub id: Uuid,
    pub block_type: Uuid,
    pub editor: Option<Uuid>,
}

impl TabItem {
    pub fn new(id: Uuid, block_type: Uuid) -> Self {
        Self {
            id,
            block_type,
            editor: None,
        }
    }

    pub fn edited_with(mut self, editor: Option<Uuid>) -> Self {
        self.editor = editor;
        self
    }
}
