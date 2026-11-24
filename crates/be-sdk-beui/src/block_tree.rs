//! One walk of blocks as a tree, which every editor that shows the workspace's blocks as nested
//! rows (the files pane, the block picker) builds its rows from: which sections there are is
//! `sections`; this walks a section's blocks, or any list of them, and their children.
//!
//! Each block is found by its path, the blocks from the walk's top down to it, so a block reached
//! two ways is two rows. A block already on its own path is shown but never unfolded. What is
//! listed comes from the editor: live per-folder watches (the files pane) or a snapshot of the
//! graph (the picker); which blocks unfold, which are shown and what a search keeps, too.

use uuid::Uuid;

use crate::BlockInfo;

/// What is known of a block's children.
pub enum Listed {
    /// Its children, in order.
    Blocks(Vec<BlockInfo>),
    /// Not yet listed, and nothing to say for it yet.
    Waiting,
    /// Not listed, and why: it failed, or is still loading after a while.
    Failed(String),
}

/// A block's row.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeBlock {
    /// The blocks from the walk's top down to this one, which keys its row.
    pub path: Vec<Uuid>,
    pub block: BlockInfo,
    pub depth: usize,
    /// The block it is listed in: the one before it on its path, if any.
    pub container: Option<Uuid>,
    /// Whether its children follow it.
    pub expanded: bool,
    /// Whether it has children to show, when they are listed; None when they are not.
    pub has_children: Option<bool>,
    /// Whether a block above it is deleted.
    pub in_deleted: bool,
}

/// What stands for a block's children when there are none to show.
#[derive(Clone, Debug, PartialEq)]
pub enum TreeNote {
    /// It has none.
    Empty,
    /// They could not be listed, and why.
    Failed(String),
}

/// A row of the tree.
#[derive(Clone, Debug, PartialEq)]
pub enum TreeEntry {
    Block(TreeBlock),
    /// In place of the children of the block at `path`.
    Note {
        path: Vec<Uuid>,
        depth: usize,
        note: TreeNote,
    },
}

/// How a tree is walked.
pub struct BlockTree<'a> {
    /// The children of a block's row, once whether it unfolds (`expanded`) is known, or None
    /// when they are not listed: an editor that lists only what is open answers None for the
    /// rest.
    pub children: &'a mut dyn FnMut(&TreeBlock) -> Option<Listed>,
    /// Whether a block's row unfolds, asked of every block not on its own path before its
    /// children are (`has_children` is still None).
    pub open: &'a dyn Fn(&TreeBlock) -> bool,
    /// Whether a block gets a row at all.
    pub shown: &'a dyn Fn(&BlockInfo) -> bool,
    /// A search: every block unfolds, and only those it matches and the blocks above them are
    /// kept. Notes are left out while it searches.
    pub matches: Option<&'a dyn Fn(&BlockInfo) -> bool>,
}

impl BlockTree<'_> {
    /// Walks `blocks`, listed in the block at the end of `under` (if any) at `depth`, onto
    /// `rows`, and answers whether it kept any.
    pub fn walk(
        &mut self,
        blocks: Vec<BlockInfo>,
        under: &[Uuid],
        depth: usize,
        in_deleted: bool,
        rows: &mut Vec<TreeEntry>,
    ) -> bool {
        let mut kept = false;
        for block in blocks {
            kept |= self.block(block, under, depth, in_deleted, rows);
        }
        kept
    }

    /// Walks `listed`, the children of the block at `under`, putting a note in their place when
    /// there are none or they failed.
    pub fn walk_listed(
        &mut self,
        listed: Listed,
        under: &[Uuid],
        depth: usize,
        in_deleted: bool,
        rows: &mut Vec<TreeEntry>,
    ) -> bool {
        let note = match listed {
            Listed::Blocks(blocks) if blocks.iter().any(|block| (self.shown)(block)) => {
                return self.walk(blocks, under, depth, in_deleted, rows);
            }
            Listed::Blocks(_) => TreeNote::Empty,
            Listed::Failed(why) => TreeNote::Failed(why),
            Listed::Waiting => return false,
        };
        if self.matches.is_none() {
            rows.push(TreeEntry::Note {
                path: under.to_vec(),
                depth,
                note,
            });
        }
        false
    }

    fn block(
        &mut self,
        block: BlockInfo,
        under: &[Uuid],
        depth: usize,
        in_deleted: bool,
        rows: &mut Vec<TreeEntry>,
    ) -> bool {
        if !(self.shown)(&block) {
            return false;
        }
        let cycle = under.contains(&block.id);
        let mut path = under.to_vec();
        path.push(block.id);
        let mut row = TreeBlock {
            path,
            container: under.last().copied(),
            depth,
            expanded: false,
            has_children: None,
            in_deleted,
            block,
        };
        // Asked even while searching: an editor may note what it opens.
        row.expanded = !cycle && ((self.open)(&row) | self.matches.is_some());
        let children = match cycle {
            true => None,
            false => (self.children)(&row),
        };
        row.has_children = match &children {
            Some(Listed::Blocks(children)) => {
                Some(children.iter().any(|child| (self.shown)(child)))
            }
            Some(Listed::Waiting | Listed::Failed(_)) | None => None,
        };
        let at = rows.len();
        let below = in_deleted || row.block.deleted;
        let expanded = row.expanded;
        let path = row.path.clone();
        let matched = self.matches.is_none_or(|matches| matches(&row.block));
        rows.push(TreeEntry::Block(row));
        let kept_child = match (expanded, children) {
            (true, Some(children)) => self.walk_listed(children, &path, depth + 1, below, rows),
            _ => false,
        };
        if !matched && !kept_child {
            rows.truncate(at);
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests;
