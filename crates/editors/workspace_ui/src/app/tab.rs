use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TabItem {
    pub(crate) id: Uuid,
    pub(crate) block_type: Uuid,
}
