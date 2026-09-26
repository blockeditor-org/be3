use block_plugin_api::{AccessLevel, ArtifactSource, BlockInfo, BlockLocation, BlockQuery};
use uuid::Uuid;

use crate::be::{Node, Query};

pub(crate) fn query_of(query: BlockQuery) -> Query {
    match query {
        BlockQuery::Roots => Query::Roots,
        BlockQuery::Detached => Query::Detached,
        BlockQuery::Children(id) => Query::Children(Uuid::from_bytes(id)),
        BlockQuery::References(id) => Query::References(Uuid::from_bytes(id)),
        BlockQuery::Backrefs(id) => Query::Backrefs(Uuid::from_bytes(id)),
        BlockQuery::Parents(id) => Query::Parents(Uuid::from_bytes(id)),
        BlockQuery::Block(id) => Query::Block(Uuid::from_bytes(id)),
    }
}

pub(crate) fn parent_of(location: BlockLocation) -> be_graph::BlockParent {
    match location {
        BlockLocation::Root => be_graph::BlockParent::Root,
        BlockLocation::Detached => be_graph::BlockParent::Detached,
        BlockLocation::Block(id) => be_graph::BlockParent::Block(Uuid::from_bytes(id)),
    }
}

pub(crate) fn location_of(parent: be_graph::BlockParent) -> BlockLocation {
    match parent {
        be_graph::BlockParent::Root => BlockLocation::Root,
        be_graph::BlockParent::Detached => BlockLocation::Detached,
        be_graph::BlockParent::Block(id) => BlockLocation::Block(id.into_bytes()),
    }
}

pub(crate) fn level_of(access: be_graph::Access) -> AccessLevel {
    match access {
        be_graph::Access::None => AccessLevel::None,
        be_graph::Access::KnowExists => AccessLevel::KnowExists,
        be_graph::Access::View => AccessLevel::View,
        be_graph::Access::Edit => AccessLevel::Edit,
    }
}

pub(crate) fn info_of(node: &Node) -> BlockInfo {
    BlockInfo {
        block_id: node.id.into_bytes(),
        block_type: node.content_type.into_bytes(),
        author: node.author.into_bytes(),
        parent: location_of(node.parent),
        name: node.metadata.name.clone(),
        named_by_hand: node.metadata.named_by_hand,
        references: node.references.iter().map(|id| id.into_bytes()).collect(),
        access: level_of(node.access),
        artifact: node
            .metadata
            .artifact
            .as_ref()
            .map(|artifact| ArtifactSource {
                source_type: artifact.source_type.into_bytes(),
                data: artifact.data.clone(),
            }),
        thumbhash: node.metadata.derived.thumbhash.as_ref().map(|thumbhash| {
            block_plugin_api::Thumbhash {
                hash: thumbhash.hash.clone(),
                width: thumbhash.width,
                height: thumbhash.height,
            }
        }),
    }
}

pub(crate) fn answer(query: BlockQuery) -> Vec<BlockInfo> {
    crate::be::query(query_of(query))
        .iter()
        .filter(|node| node.access.can_know_exists())
        .map(info_of)
        .collect()
}
