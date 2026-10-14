use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TabItem {
    pub(crate) id: Uuid,
    pub(crate) block_type: Uuid,
    pub(crate) editor: Option<Uuid>,
}

impl TabItem {
    pub(crate) fn new(id: Uuid, block_type: Uuid) -> Self {
        Self {
            id,
            block_type,
            editor: None,
        }
    }

    pub(crate) fn edited_with(mut self, editor: Option<Uuid>) -> Self {
        self.editor = editor;
        self
    }
}
