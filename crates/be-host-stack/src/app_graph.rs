use be_sdk_api::{AccessLevel, BlockInfo, BlockLocation, BlockQuery, MetadataEntries};
use uuid::Uuid;

use crate::{Node, Query};

pub fn query_of(query: BlockQuery) -> Query {
    match query {
        BlockQuery::Roots => Query::Roots,
        BlockQuery::Detached => Query::Detached,
        BlockQuery::Shared => Query::Shared,
        // With nobody signed in there is no Unfiled: the nil block's children, which are none.
        BlockQuery::Unfiled => Query::Children(crate::unfiled().unwrap_or_else(Uuid::nil)),
        BlockQuery::Children(id) => Query::Children(Uuid::from_bytes(id)),
        BlockQuery::References(id) => Query::References(Uuid::from_bytes(id)),
        BlockQuery::Backrefs(id) => Query::Backrefs(Uuid::from_bytes(id)),
        BlockQuery::Parents(id) => Query::Parents(Uuid::from_bytes(id)),
        BlockQuery::Block(id) => Query::Block(Uuid::from_bytes(id)),
        BlockQuery::OpenSettings(id) => Query::OpenSettings(Uuid::from_bytes(id)),
    }
}

/// The parent a location names, `Unfiled` resolved to the signed-in account's own. None for
/// `Unfiled` until the graph has it, so a call naming it is refused rather than trashing or
/// losing the block.
pub fn parent_of(location: BlockLocation) -> Option<be_block_graph::BlockParent> {
    Some(match location {
        BlockLocation::Root => be_block_graph::BlockParent::Root,
        BlockLocation::Detached => be_block_graph::BlockParent::Detached,
        BlockLocation::Block(id) => be_block_graph::BlockParent::Block(Uuid::from_bytes(id)),
        BlockLocation::Unfiled => be_block_graph::BlockParent::Block(
            crate::unfiled().filter(|unfiled| crate::node(*unfiled).is_some())?,
        ),
    })
}

/// How an editor reads `parent`: the account's Unfiled (`crate::unfiled()`, passed in so this
/// can run under the graph's lock) as `Unfiled`, never by its id.
pub fn location_of(parent: be_block_graph::BlockParent, unfiled: Option<Uuid>) -> BlockLocation {
    match parent {
        be_block_graph::BlockParent::Root => BlockLocation::Root,
        be_block_graph::BlockParent::Detached => BlockLocation::Detached,
        be_block_graph::BlockParent::Block(id) if unfiled == Some(id) => BlockLocation::Unfiled,
        be_block_graph::BlockParent::Block(id) => BlockLocation::Block(id.into_bytes()),
    }
}

pub fn access_of(level: AccessLevel) -> be_block_graph::Access {
    match level {
        AccessLevel::None => be_block_graph::Access::None,
        AccessLevel::KnowExists => be_block_graph::Access::KnowExists,
        AccessLevel::View => be_block_graph::Access::View,
        AccessLevel::Edit => be_block_graph::Access::Edit,
        AccessLevel::Manage => be_block_graph::Access::Manage,
    }
}

pub fn notify_mode_of(mode: be_sdk_api::NotifyMode) -> be_block_protocol::NotifyMode {
    match mode {
        be_sdk_api::NotifyMode::All => be_block_protocol::NotifyMode::All,
        be_sdk_api::NotifyMode::Mentions => be_block_protocol::NotifyMode::Mentions,
        be_sdk_api::NotifyMode::Off => be_block_protocol::NotifyMode::Off,
    }
}

pub fn notify_mode_to(mode: be_block_protocol::NotifyMode) -> be_sdk_api::NotifyMode {
    match mode {
        be_block_protocol::NotifyMode::All => be_sdk_api::NotifyMode::All,
        be_block_protocol::NotifyMode::Mentions => be_sdk_api::NotifyMode::Mentions,
        be_block_protocol::NotifyMode::Off => be_sdk_api::NotifyMode::Off,
    }
}

pub fn grants_of(
    listed: Result<be_block_protocol::AccessList, String>,
) -> Result<be_sdk_api::AccessListing, String> {
    listed.map(|list| be_sdk_api::AccessListing {
        grants: list
            .entries
            .into_iter()
            .map(|entry| be_sdk_api::AccessGrant {
                account: entry.account.into_bytes(),
                email: entry.email,
                display_name: entry.display_name,
                administrator: matches!(
                    entry.role,
                    be_block_protocol::OrganizationRole::Administrator
                ),
                author: entry.author,
                granted: entry.granted.map(level_of),
                inherited: level_of(entry.inherited),
                effective: level_of(entry.effective),
                source: entry.source.map(Uuid::into_bytes),
                waiting: entry.waiting,
            })
            .collect(),
        company: list.company,
        set_inside: list.set_inside,
    })
}

pub fn level_of(access: be_block_graph::Access) -> AccessLevel {
    match access {
        be_block_graph::Access::None => AccessLevel::None,
        be_block_graph::Access::KnowExists => AccessLevel::KnowExists,
        be_block_graph::Access::View => AccessLevel::View,
        be_block_graph::Access::Edit => AccessLevel::Edit,
        be_block_graph::Access::Manage => AccessLevel::Manage,
    }
}

pub fn info_of(node: &Node, deleted: bool, unfiled: Option<Uuid>) -> BlockInfo {
    BlockInfo {
        block_id: node.id.into_bytes(),
        block_type: node.content_type.into_bytes(),
        author: node.author.map(Uuid::into_bytes),
        parent: node.parent.map(|parent| location_of(parent, unfiled)),
        references: node.contents().into_iter().map(Uuid::into_bytes).collect(),
        access: level_of(node.access),
        metadata: be_sdk_api::BlockMetadata {
            defined: wire_entries(&node.metadata.defined),
            derived: wire_entries(&node.metadata.derived),
        },
        locked: node.locked,
        deleted,
        sets_access: node.sets_access,
        size: node.size,
        tree_size: node.tree_size,
    }
}

pub fn answer(query: BlockQuery) -> Vec<BlockInfo> {
    let nodes = crate::query(query_of(query));
    let unfiled = crate::unfiled();
    crate::with_graph(|graph| {
        nodes
            .iter()
            .filter(|node| node.access.can_know_exists() && Some(node.id) != unfiled)
            .map(|node| info_of(node, graph.deleted_root(node.id).is_some(), unfiled))
            .collect()
    })
    .unwrap_or_default()
}

pub fn wire_entries(entries: &be_block::Entries) -> MetadataEntries {
    entries
        .iter()
        .map(|(key, value)| (key.into_bytes(), value.clone()))
        .collect()
}

pub fn entries_of(entries: MetadataEntries) -> be_block::Entries {
    entries
        .into_iter()
        .map(|(key, value)| (Uuid::from_bytes(key), value))
        .collect()
}
