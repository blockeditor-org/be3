use block_plugin_api::{AccessLevel, BlockInfo, BlockLocation, BlockQuery, MetadataEntries};
use uuid::Uuid;

use crate::be::{Node, Query};

pub(crate) fn query_of(query: BlockQuery) -> Query {
    match query {
        BlockQuery::Roots => Query::Roots,
        BlockQuery::Detached => Query::Detached,
        BlockQuery::Shared => Query::Shared,
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

pub(crate) fn access_of(level: AccessLevel) -> be_graph::Access {
    match level {
        AccessLevel::None => be_graph::Access::None,
        AccessLevel::KnowExists => be_graph::Access::KnowExists,
        AccessLevel::View => be_graph::Access::View,
        AccessLevel::Edit => be_graph::Access::Edit,
    }
}

pub(crate) fn grants_of(
    listed: Result<Vec<be_protocol::AccessEntry>, String>,
) -> Result<Vec<block_plugin_api::AccessGrant>, String> {
    listed.map(|entries| {
        entries
            .into_iter()
            .map(|entry| block_plugin_api::AccessGrant {
                account: entry.account.into_bytes(),
                email: entry.email,
                display_name: entry.display_name,
                administrator: matches!(entry.role, be_protocol::OrganizationRole::Administrator),
                author: entry.author,
                granted: entry.granted.map(level_of),
                effective: level_of(entry.effective),
                waiting: entry.waiting,
            })
            .collect()
    })
}

pub(crate) fn level_of(access: be_graph::Access) -> AccessLevel {
    match access {
        be_graph::Access::None => AccessLevel::None,
        be_graph::Access::KnowExists => AccessLevel::KnowExists,
        be_graph::Access::View => AccessLevel::View,
        be_graph::Access::Edit => AccessLevel::Edit,
    }
}

pub(crate) fn info_of(node: &Node, deleted: bool) -> BlockInfo {
    BlockInfo {
        block_id: node.id.into_bytes(),
        block_type: node.content_type.into_bytes(),
        author: node.author.map(Uuid::into_bytes),
        parent: node.parent.map(location_of),
        references: node.references.iter().map(|id| id.into_bytes()).collect(),
        access: level_of(node.access),
        metadata: block_plugin_api::BlockMetadata {
            defined: wire_entries(&node.metadata.defined),
            derived: wire_entries(&node.metadata.derived),
        },
        locked: node.locked,
        deleted,
    }
}

pub(crate) fn answer(query: BlockQuery) -> Vec<BlockInfo> {
    let nodes = crate::be::query(query_of(query));
    crate::be::with_graph(|graph| {
        nodes
            .iter()
            .filter(|node| node.access.can_know_exists())
            .map(|node| info_of(node, graph.deleted_root(node.id).is_some()))
            .collect()
    })
    .unwrap_or_default()
}

pub(crate) fn wire_entries(entries: &be_block::Entries) -> MetadataEntries {
    entries
        .iter()
        .map(|(key, value)| {
            (key.into_bytes(), value.clone())
        })
        .collect()
}

pub(crate) fn entries_of(entries: MetadataEntries) -> be_block::Entries {
    entries
        .into_iter()
        .map(|(key, value)| (Uuid::from_bytes(key), value))
        .collect()
}
