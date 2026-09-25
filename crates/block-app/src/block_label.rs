use uuid::Uuid;

use crate::be::Node;
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
        name: Option<&str>,
        named_by_hand: bool,
    ) -> Self {
        let (name, automatic) = match name.filter(|name| !name.is_empty()) {
            Some(name) => (name.to_owned(), !named_by_hand),
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

    pub(crate) fn for_node(registry: &EditorRegistry, node: &Node) -> Self {
        Self::new(
            registry,
            node.content_type,
            node.metadata.name.as_deref(),
            node.metadata.named_by_hand,
        )
    }
}
