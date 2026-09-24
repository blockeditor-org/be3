use block::BlockParent;
use block_client::{
    BlockClient, BlockHandleAccess, BlockHistoryHandle, BlockRelationships, properties::BlockName,
};
use uuid::Uuid;

pub(super) struct UnsupportedBlock {
    id: Uuid,
    block_type: Uuid,
}

impl UnsupportedBlock {
    pub(super) fn new(id: Uuid, block_type: Uuid) -> Self {
        Self { id, block_type }
    }
}

impl BlockHandleAccess for UnsupportedBlock {
    fn id(&self) -> Uuid {
        self.id
    }

    fn block_type(&self) -> Uuid {
        self.block_type
    }

    fn name(&self) -> Option<String> {
        Some(self.id.to_string())
    }

    fn block_name(&self) -> Option<BlockName> {
        Some(BlockName {
            manual: true,
            value: self.id.to_string(),
        })
    }

    fn relationships(&self) -> Option<BlockRelationships> {
        None
    }

    fn set_parent(&self, _parent: BlockParent) {}

    fn set_name(&self, _name: Option<String>) {}

    fn history(&self) -> Option<&dyn BlockHistoryHandle> {
        None
    }

    fn duplicate(&self, _client: &BlockClient) -> Option<Uuid> {
        None
    }
}
