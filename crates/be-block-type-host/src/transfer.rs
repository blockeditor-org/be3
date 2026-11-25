//! A block and everything inside it, collected for a block file (`be_block_client::transfer`):
//! the one export the host's and `be download` share. Import is `Peer::queue_import`.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;

use be_block_client::transfer::SavedBlock;
use be_block_client::{ClientError, Peer};
use be_block_content::Entries;
use be_block_graph::BlockParent;
use be_block_protocol::{BlockSummary, ErrorCode};
use be_block_store::ObjectStore;
use uuid::Uuid;

use crate::Modules;

/// What the exporter holds beside what is stored: the type modules, the replicas it keeps open,
/// whose content is newer than what they saved, and the metadata it derived from content.
/// `Modules` alone holds nothing open and derives nothing.
pub trait Open {
    fn modules(&mut self) -> &mut Modules;

    /// `block`'s content as a replica held open has it; none when no replica of it is open
    /// (or it did not answer), and the block is read as stored. A boxed future, not an
    /// `impl Future`: with the opaque type in `collect`'s future, rustc 1.98 fails to lay that
    /// future out on be-host's wasi build (an internal error in its drop glue), depending on
    /// what else be-host holds.
    fn content(&mut self, block: Uuid) -> Pin<Box<dyn Future<Output = Option<Vec<u8>>> + '_>>;

    /// Whether `block`'s replica waits for its head's objects (`Replicas::not_yet`): its export
    /// fails at once rather than read what is stored.
    fn not_yet(&self, block: Uuid) -> bool;

    /// The metadata the exporter derived from `block`'s content, in place of what was saved.
    fn derived(&self, block: Uuid) -> Option<Entries>;
}

impl Open for Modules {
    fn modules(&mut self) -> &mut Modules {
        self
    }

    fn content(&mut self, _: Uuid) -> Pin<Box<dyn Future<Output = Option<Vec<u8>>> + '_>> {
        Box::pin(std::future::ready(None))
    }

    fn not_yet(&self, _: Uuid) -> bool {
        false
    }

    fn derived(&self, _: Uuid) -> Option<Entries> {
        None
    }
}

/// `top` and every block below it this account may view, breadth first, the first the outermost
/// (a `parent` is a position in the list). A block only its author can read, or one inside
/// such a block, is refused, since a block file opens for anyone; one below that only its
/// author reads is left out. When `top`'s parent is a type `exported_whole` says goes only whole
/// (a database view with its database), the export starts from that parent, if this account
/// may view it; otherwise, or when this account may not know of the parent, from `top` alone.
/// A block above that cannot be read for any other reason, or a block listed inside itself,
/// fails the export. Each block carries its content as `open` holds it, or else as readers outside its session see it,
/// and its signed log when its type keeps one. A type that keeps parts after its first, which
/// bytes do not carry, is refused until a block file carries parts.
pub async fn collect<S: ObjectStore>(
    peer: &Peer<S>,
    top: Uuid,
    exported_whole: impl Fn(Uuid) -> bool,
    open: &mut impl Open,
) -> Result<Vec<SavedBlock>, ClientError> {
    let picked = peer.summary(top).await?;
    // The server marks only the block made readable by its author alone, not what lies in it,
    // so each block above is asked too, up to the first this account may not know of. Any other
    // failure to read one fails the export: what cannot be checked is not exported.
    let mut parent = None;
    let mut seen = HashSet::from([picked.id]);
    let mut above = Some(picked.clone());
    while let Some(summary) = above {
        if summary.owner_only {
            return Err(ClientError::Refused(
                ErrorCode::PermissionDenied,
                "a block only you can read is not exported, since a block file opens for anyone"
                    .into(),
            ));
        }
        above = match summary.parent {
            Some(BlockParent::Block(up)) if !seen.insert(up) => {
                return Err(ClientError::Refused(
                    ErrorCode::InvalidRequest,
                    format!("block {up} is listed inside itself"),
                ));
            }
            Some(BlockParent::Block(up)) => match peer.summary(up).await {
                Ok(summary) => Some(summary),
                Err(ClientError::Refused(
                    ErrorCode::BlockNotFound | ErrorCode::PermissionDenied,
                    _,
                )) => None,
                Err(error) => return Err(error),
            },
            _ => None,
        };
        if summary.id == picked.id {
            parent = above.clone();
        }
    }
    let top = match parent {
        Some(whole) if whole.access.can_view() && exported_whole(whole.content_type) => whole,
        _ => picked,
    };
    let mut found: Vec<(BlockSummary, Option<usize>)> = vec![(top.clone(), None)];
    let mut level = vec![(top.id, 0)];
    while !level.is_empty() {
        let below = futures_util::future::try_join_all(
            level
                .iter()
                .map(|(parent, _)| peer.children(BlockParent::Block(*parent))),
        )
        .await?;
        let mut next = Vec::new();
        for ((_, position), children) in level.into_iter().zip(below) {
            for child in children {
                if child.access.can_view()
                    && !child.owner_only
                    && found.iter().all(|(held, _)| held.id != child.id)
                {
                    next.push((child.id, found.len()));
                    found.push((child, Some(position)));
                }
            }
        }
        level = next;
    }
    let summaries: Vec<BlockSummary> = found.iter().map(|(summary, _)| summary.clone()).collect();
    peer.learn(&summaries).await?;
    // Per type, once: whether it keeps a signed log.
    let mut kinds: HashMap<Uuid, bool> = HashMap::new();
    let mut blocks = Vec::with_capacity(found.len());
    for (summary, parent) in found {
        let logged = match kinds.get(&summary.content_type) {
            Some(kind) => *kind,
            None => {
                let kind = open
                    .modules()
                    .kind(summary.content_type)
                    .await
                    .is_some_and(|kind| kind.log);
                kinds.insert(summary.content_type, kind);
                kind
            }
        };
        // A type with a signed log carries it, opened, each entry with its author's signature
        // in the block it was written in (`Peer::verify_exported`).
        let log = match logged {
            true => peer.export_log(summary.id).await?,
            false => Vec::new(),
        };
        // Not asked at all when not to be had, and checked again after, as it may find out only
        // as it is asked.
        let content = match open.not_yet(summary.id) {
            true => None,
            false => open.content(summary.id).await,
        };
        if content.is_none() && open.not_yet(summary.id) {
            return Err(ClientError::NotYet(summary.id));
        }
        blocks.push((
            summary.head,
            SavedBlock {
                id: summary.id,
                block_type: summary.content_type,
                parent,
                metadata: peer.metadata(&summary).unwrap_or_default(),
                references: peer.content_references(&summary).await?,
                content,
                log,
            },
        ));
    }
    let wanted: Vec<_> = blocks
        .iter()
        .filter(|(_, block)| block.content.is_none())
        .map(|(head, block)| (block.id, *head))
        .collect();
    let mut read = peer.read_contents(&wanted).await?.into_iter();
    let mut saved = Vec::with_capacity(blocks.len());
    for (_, mut block) in blocks {
        if block.content.is_none()
            && let Some(read) = read.next()
        {
            block.content = open.modules().readable(read).await;
        }
        if let Some(derived) = open.derived(block.id) {
            block.metadata.derived = derived;
        }
        saved.push(block);
    }
    Ok(saved)
}
