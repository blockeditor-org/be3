use uuid::Uuid;

use crate::BlockClient;

pub use be_block::BlockRef;

pub trait WorktreeMembership: Send + Sync {
    fn worktree_type_id(&self) -> Uuid;

    fn repo_id(&self, client: &BlockClient, worktree_id: Uuid) -> Option<Uuid>;

    fn eternal_id_for_member(
        &self,
        client: &BlockClient,
        worktree_id: Uuid,
        live_id: Uuid,
    ) -> Option<Uuid>;

    fn mint_eternal_id(&self, client: &BlockClient, worktree_id: Uuid, live_id: Uuid) -> Uuid;

    fn resolve_eternal_id(
        &self,
        client: &BlockClient,
        worktree_id: Uuid,
        eternal_id: Uuid,
    ) -> Option<Uuid>;
}

#[cfg(test)]
mod tests;
