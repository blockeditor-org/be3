use block_client::{BlockHandleAccess, CachedBlock, properties::BlockName};
use uuid::Uuid;

use crate::editors::EditorRegistry;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BlockLabel {
    pub(crate) block_type: Uuid,
    pub(crate) icon: Option<&'static str>,
    pub(crate) name: String,
    pub(crate) automatic: bool,
}

impl BlockLabel {
    pub(crate) fn new(
        registry: &EditorRegistry,
        block_type: Uuid,
        name: Option<&BlockName>,
    ) -> Self {
        let (name, automatic) = match name.filter(|name| !name.value.is_empty()) {
            Some(name) => (name.value.clone(), !name.manual),
            None => (
                registry
                    .display_name(block_type)
                    .map(str::to_owned)
                    .unwrap_or_else(|| "Untitled".to_owned()),
                true,
            ),
        };
        Self {
            block_type,
            icon: registry.icon(block_type),
            name,
            automatic,
        }
    }

    pub(crate) fn for_cached(registry: &EditorRegistry, cached: &CachedBlock) -> Self {
        Self::new(
            registry,
            cached.block_type,
            block_client::properties::read_name(&cached.properties).as_ref(),
        )
    }

    #[allow(dead_code)]
    pub(crate) fn for_handle(registry: &EditorRegistry, handle: &dyn BlockHandleAccess) -> Self {
        Self::new(registry, handle.block_type(), handle.block_name().as_ref())
    }
}
