use be_block::be_model::BlockRef;
use be_block::{
    BlockContent, BlockMetadata, Edit, PREFIXES, PrefixRegistry, PrefixRegistryContent,
    WORKSPACE_TYPE,
};
use be_graph::{Access, BlockParent};
use be_store::ObjectStore;
use uuid::Uuid;

use super::{Peer, Saved};
use crate::ClientError;

const ATTEMPTS: usize = 8;
const MAX_DEPTH: usize = 256;

pub struct Registry {
    pub owner: Uuid,
    pub kind: Option<Uuid>,
}

impl<S: ObjectStore> Peer<S> {
    pub async fn registry_for(&self, block: Uuid) -> Result<Option<Registry>, ClientError> {
        let parent = self.summary(block).await?.parent;
        self.registry_above(parent).await
    }

    async fn registry_above(
        &self,
        mut current: Option<BlockParent>,
    ) -> Result<Option<Registry>, ClientError> {
        let mut workspace = None;
        let mut outermost = None;
        for _ in 0..MAX_DEPTH {
            let Some(BlockParent::Block(block)) = current else {
                break;
            };
            let Ok(summary) = self.summary(block).await else {
                break;
            };
            if !summary.access.can_view() {
                break;
            }
            if let Some(kind) = self.metadata(&summary)?.get(&PREFIXES) {
                return Ok(Some(Registry {
                    owner: block,
                    kind: Some(kind.0),
                }));
            }
            if summary.content_type == WORKSPACE_TYPE && workspace.is_none() {
                workspace = Some(block);
            }
            outermost = Some(block);
            current = summary.parent;
        }
        Ok(workspace
            .or(outermost)
            .map(|owner| Registry { owner, kind: None }))
    }

    pub async fn register_prefix(
        &self,
        database: Uuid,
        prefix: Option<&str>,
    ) -> Result<(), ClientError> {
        let Some(registry) = self.registry_for(database).await? else {
            return Ok(());
        };
        let edit = PrefixRegistry::register(database, prefix);
        self.change_registry(registry, &[edit]).await
    }

    async fn change_registry(&self, registry: Registry, edits: &[Edit]) -> Result<(), ClientError> {
        let kind = match registry.kind {
            Some(kind) => kind,
            None => {
                let kind = Uuid::now_v7();
                self.create_block(
                    kind,
                    PrefixRegistryContent::CONTENT_TYPE,
                    BlockParent::Block(registry.owner),
                    &BlockMetadata::default(),
                )
                .await?;
                self.open_inside(kind, Access::View).await?;
                self.define(registry.owner, |metadata| {
                    metadata.insert(&PREFIXES, &BlockRef(kind));
                })
                .await?;
                kind
            }
        };
        for _ in 0..ATTEMPTS {
            let head = self.newest_head(kind).await?;
            let mut content = match head {
                Some(_) => self
                    .open::<PrefixRegistryContent>(kind)
                    .await?
                    .unwrap_or_default(),
                None => PrefixRegistryContent::default(),
            };
            let before = content.root();
            for edit in edits {
                content.apply(edit);
            }
            if content.root() == before {
                return Ok(());
            }
            match self.save(kind, &content, head).await? {
                Saved::Rejected { .. } => continue,
                Saved::Published(_) | Saved::Unchanged(_) => return Ok(()),
            }
        }
        Err(ClientError::Contended)
    }

    pub(super) async fn follow_prefixes(
        &self,
        block: Uuid,
        from: Option<BlockParent>,
        to: BlockParent,
    ) -> Result<(), ClientError> {
        if !matches!(to, BlockParent::Block(_)) || from == Some(to) {
            return Ok(());
        }
        let Some(Registry {
            owner,
            kind: Some(old),
        }) = self.registry_above(from).await?
        else {
            return Ok(());
        };
        let Some(registered) = self.open::<PrefixRegistryContent>(old).await? else {
            return Ok(());
        };
        let registered = registered.root();
        let mut moved = Vec::new();
        for (database, prefix) in registered.databases.iter() {
            if self.lies_in(*database, block).await {
                moved.push((*database, prefix.clone()));
            }
        }
        if moved.is_empty() {
            return Ok(());
        }
        let new = self.registry_above(Some(to)).await?;
        if new.as_ref().is_some_and(|new| new.kind == Some(old)) {
            return Ok(());
        }
        let removed: Vec<Edit> = moved
            .iter()
            .map(|(database, _)| PrefixRegistry::register(*database, None))
            .collect();
        if let Some(new) = new {
            let added: Vec<Edit> = moved
                .iter()
                .map(|(database, prefix)| PrefixRegistry::register(*database, Some(prefix)))
                .collect();
            self.change_registry(new, &added).await?;
        }
        self.change_registry(
            Registry {
                owner,
                kind: Some(old),
            },
            &removed,
        )
        .await
    }

    async fn lies_in(&self, block: Uuid, ancestor: Uuid) -> bool {
        let mut current = block;
        for _ in 0..MAX_DEPTH {
            if current == ancestor {
                return true;
            }
            match self.summary(current).await.map(|summary| summary.parent) {
                Ok(Some(BlockParent::Block(parent))) => current = parent,
                _ => return false,
            }
        }
        false
    }
}
