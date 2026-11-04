use super::{DockState, Entry, LeafId, Node, SplitSize, TabId, Tree};

/// A gathered tree's sizes, fold and shown tab, kept per stage tab and saved apart from the tiled
/// splits so switching sizes mixes neither.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DockCompact {
    pub stage: TabId,
    pub shown: Option<TabId>,
    pub portrait: Option<f32>,
    pub landscape: Option<f32>,
    pub folded: bool,
}

impl DockCompact {
    pub fn new(stage: TabId) -> Self {
        Self {
            stage,
            shown: None,
            portrait: None,
            landscape: None,
            folded: false,
        }
    }
}

pub const COMPACT_SHARE: f32 = 0.4;
pub const COMPACT_SIDE: f32 = 280.0;

impl DockState {
    /// A bare pane among others: an editor's stage, which a compact dock keeps while gathering the rest.
    pub fn stage_of(&self, tree: Tree) -> Option<LeafId> {
        let leaves = self.tree_leaves(tree);
        if leaves.len() < 2 {
            return None;
        }
        leaves.into_iter().find(|leaf| self.is_bare(*leaf))
    }

    pub fn stage_tab(&self, tree: Tree) -> Option<TabId> {
        let stage = self.stage_of(tree)?;
        let entry = self.active_entry(stage)?;
        self.standing_tabs(entry).first().copied()
    }

    pub fn compact_tree(&self, leaf: LeafId) -> Option<Tree> {
        let mut leaf = leaf;
        for _ in 0..64 {
            let tree = self.tree_of(leaf)?;
            if self.stage_of(tree).is_some() {
                return Some(tree);
            }
            match tree {
                Tree::Group(group) => leaf = self.holder(group)?.0,
                Tree::Surface(_) => return None,
            }
        }
        None
    }

    pub fn compact_tree_of(&self, tab: TabId) -> Option<Tree> {
        self.compact_tree(self.find(tab)?.leaf)
    }

    pub fn gathered_tabs(&self, tree: Tree) -> Vec<TabId> {
        let Some(stage) = self.stage_of(tree) else {
            return Vec::new();
        };
        self.tree_leaves(tree)
            .into_iter()
            .filter(|leaf| *leaf != stage)
            .flat_map(|leaf| self.entries(leaf))
            .flat_map(|entry| self.standing_tabs(entry))
            .collect()
    }

    /// Lists a gathered tree by its stage alone.
    pub fn switchable(&self, tabs: Vec<TabId>) -> Vec<TabId> {
        tabs.into_iter()
            .filter(|tab| {
                self.compact_tree_of(*tab)
                    .is_none_or(|tree| !self.gathered_tabs(tree).contains(tab))
            })
            .collect()
    }

    pub fn compact(&self, stage: TabId) -> DockCompact {
        self.compact
            .iter()
            .find(|held| held.stage == stage)
            .copied()
            .unwrap_or(DockCompact::new(stage))
    }

    pub fn compacts(&self) -> &[DockCompact] {
        &self.compact
    }

    pub fn set_compacts(&mut self, compacts: Vec<DockCompact>) {
        self.compact = compacts;
    }

    fn edit_compact(&mut self, stage: TabId, change: impl FnOnce(&mut DockCompact)) {
        let mut held = self.compact(stage);
        change(&mut held);
        self.compact.retain(|other| other.stage != stage);
        self.compact.push(held);
    }

    pub fn gathered_shown(&self, tree: Tree) -> Option<TabId> {
        let tabs = self.gathered_tabs(tree);
        let stage = self.stage_tab(tree)?;
        let picked = self.compact(stage).shown.filter(|tab| tabs.contains(tab));
        picked
            .or_else(|| {
                let stage_leaf = self.stage_of(tree)?;
                let leaves: Vec<LeafId> = self
                    .tree_leaves(tree)
                    .into_iter()
                    .filter(|leaf| *leaf != stage_leaf && !self.entries(*leaf).is_empty())
                    .collect();
                let leaf = leaves
                    .iter()
                    .find(|leaf| self.is_bare(**leaf))
                    .or(leaves.first())
                    .copied()?;
                self.standing_tabs(self.active_entry(leaf)?)
                    .first()
                    .copied()
            })
            .or_else(|| tabs.first().copied())
    }

    pub fn is_folded(&self, tree: Tree) -> bool {
        self.stage_tab(tree)
            .is_some_and(|stage| self.compact(stage).folded)
    }

    pub fn pick_gathered(&mut self, tab: TabId) {
        let Some(tree) = self.compact_tree_of(tab) else {
            return;
        };
        let Some(stage) = self.stage_tab(tree) else {
            return;
        };
        let refold = self.gathered_shown(tree) == Some(tab) && !self.compact(stage).folded;
        self.edit_compact(stage, |held| {
            held.shown = Some(tab);
            held.folded = refold;
        });
        if let Some(position) = self.find(tab) {
            if let Some(leaf) = self.leaf_mut(position.leaf)
                && position.index < leaf.entries.len()
            {
                leaf.active = position.index;
            }
        }
    }

    pub(super) fn reveal_gathered(&mut self, tab: TabId) {
        let Some(tree) = self.compact_tree_of(tab) else {
            return;
        };
        if !self.gathered_tabs(tree).contains(&tab) {
            return;
        }
        let Some(stage) = self.stage_tab(tree) else {
            return;
        };
        self.edit_compact(stage, |held| {
            held.shown = Some(tab);
            held.folded = false;
        });
    }

    pub fn set_folded(&mut self, tree: Tree, folded: bool) {
        if let Some(stage) = self.stage_tab(tree) {
            self.edit_compact(stage, |held| held.folded = folded);
        }
    }

    pub fn set_compact_size(&mut self, tree: Tree, portrait: bool, points: f32) {
        let Some(stage) = self.stage_tab(tree) else {
            return;
        };
        let points = points.max(0.0).round();
        self.edit_compact(stage, |held| match portrait {
            true => held.portrait = Some(points),
            false => held.landscape = Some(points),
        });
    }

    pub fn compact_wanted(&self, tree: Tree, portrait: bool) -> Option<f32> {
        let held = self.stage_tab(tree).map(|stage| self.compact(stage));
        match portrait {
            true => held.and_then(|held| held.portrait),
            false => held
                .and_then(|held| held.landscape)
                .or_else(|| self.side_points(tree))
                .or(Some(COMPACT_SIDE)),
        }
    }

    fn side_points(&self, tree: Tree) -> Option<f32> {
        fn find(node: &Node) -> Option<f32> {
            match node {
                Node::Leaf(_) => None,
                Node::Split(split) => match split.size {
                    SplitSize::First(points) | SplitSize::Second(points) => Some(points),
                    SplitSize::Fraction(_) => find(&split.first).or_else(|| find(&split.second)),
                },
            }
        }
        find(self.root(tree)?)
    }

    pub(super) fn compact_shows(&self, tab: TabId, top: Option<Tree>) -> bool {
        let Some(position) = self.find(tab) else {
            return false;
        };
        let mut leaf = position.leaf;
        let mut entry = Entry::Tab(tab);
        for _ in 0..64 {
            let Some(tree) = self.tree_of(leaf) else {
                return false;
            };
            let shown = match self.stage_of(tree) {
                Some(stage) if stage == leaf => self.active_entry(leaf) == Some(entry),
                Some(_) => {
                    let gathered = self.gathered_shown(tree);
                    !self.is_folded(tree)
                        && gathered.is_some_and(|gathered| {
                            self.standing_tabs(entry).contains(&gathered)
                                || self.entry_tabs(entry).contains(&gathered)
                        })
                }
                None => self.active_entry(leaf) == Some(entry),
            };
            if !shown {
                return false;
            }
            if Some(tree) == top {
                return true;
            }
            match tree {
                Tree::Surface(_) => return top.is_none(),
                Tree::Group(group) => {
                    let Some((holder, _)) = self.holder(group) else {
                        return false;
                    };
                    leaf = holder;
                    entry = Entry::Group(group);
                }
            }
        }
        false
    }
}

pub fn compact_length(wanted: Option<f32>, length: f32) -> f32 {
    let wanted = wanted.unwrap_or_else(|| (length * COMPACT_SHARE).round());
    let most = (length - super::MIN_PANE_LENGTH).max(length / 2.0);
    wanted.min(most).max(0.0).round()
}
