use stable_hash::{HashMap, HashSet};

use beui_view::types::Rect;

use super::{
    Bar, DockHome, DockSpecNode, DockState, DockVisit, Entry, Group, GroupId, Home, LeafId, Node,
    Split, SplitId, TabId, Tree,
};

const JOINED: &str = "joined:";

pub fn joined_key(host: TabId) -> String {
    format!("{JOINED}{}", host.value())
}

pub fn joined_host(key: &str) -> Option<TabId> {
    key.strip_prefix(JOINED)?.parse().ok().map(TabId::new)
}

pub fn joined_prefix(host: TabId) -> String {
    format!("{}/", joined_key(host))
}

#[derive(Clone, Debug, PartialEq)]
pub struct DockOutside {
    pub tab: TabId,
    pub place: DockOutsidePlace,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DockOutsidePlace {
    Pane { key: String, index: usize },
    Window { rect: Rect },
}

pub struct DockMapping<'a> {
    pub tab: &'a dyn Fn(TabId) -> Option<TabId>,
    pub key: &'a dyn Fn(&str) -> Option<String>,
    pub group: &'a dyn Fn(&str) -> Option<TabId>,
}

#[derive(Default)]
struct Copied {
    leaves: HashMap<LeafId, LeafId>,
    groups: HashMap<GroupId, GroupId>,
}

impl DockState {
    pub fn outside(&self) -> &[DockOutside] {
        &self.outside
    }

    pub fn set_outside(&mut self, outside: Vec<DockOutside>) {
        self.outside = outside;
    }

    pub fn outside_of(
        &self,
        group: GroupId,
        inner: &dyn Fn(TabId) -> Option<TabId>,
    ) -> Vec<DockOutside> {
        let inside = self.group_tabs(group);
        self.all_tabs()
            .into_iter()
            .filter(|tab| !inside.contains(tab))
            .filter_map(|tab| {
                let held = inner(tab)?;
                let position = self.find(tab)?;
                let place = match self.pane_key(position.leaf) {
                    Some(key) => DockOutsidePlace::Pane {
                        key: key.to_owned(),
                        index: position.index,
                    },
                    None if !self.is_nested(position.leaf) => DockOutsidePlace::Window {
                        rect: self.window_rect(position.surface)?,
                    },
                    None => return None,
                };
                Some(DockOutside { tab: held, place })
            })
            .collect()
    }

    fn put_outside(&mut self, group: GroupId, from: &DockState, map: &DockMapping<'_>) {
        let focus = self.focus;
        for outside in &from.outside {
            let Some(tab) = (map.tab)(outside.tab) else {
                continue;
            };
            match &outside.place {
                DockOutsidePlace::Pane { key, index } => {
                    let Some(leaf) = self
                        .leaf_by_key(key)
                        .filter(|leaf| !self.contains_leaf(Entry::Group(group), *leaf))
                    else {
                        continue;
                    };
                    self.take(Entry::Tab(tab));
                    if self.leaf(leaf).is_some() {
                        self.insert_entry(leaf, *index, Entry::Tab(tab));
                    }
                }
                DockOutsidePlace::Window { rect } => {
                    self.take(Entry::Tab(tab));
                    self.open_window_with(*rect, vec![Entry::Tab(tab)]);
                }
            }
            if self.contains(tab) {
                self.homes.retain(|home| home.tab != tab);
                self.homes.push(Home {
                    tab,
                    place: DockHome::Group(group),
                    within: None,
                });
            }
        }
        self.focus = focus.filter(|leaf| self.leaf(*leaf).is_some());
    }

    pub fn find_group(&self, key: &str) -> Option<GroupId> {
        self.group_by_key(key)
    }

    pub fn joined_groups(&self) -> Vec<(GroupId, TabId)> {
        self.groups
            .iter()
            .filter_map(|group| Some((group.id, joined_host(group.key.as_deref()?)?)))
            .collect()
    }

    pub fn host_group(
        &mut self,
        tab: TabId,
        key: &str,
        root: &DockSpecNode,
        focus: Option<TabId>,
    ) -> Option<GroupId> {
        let (leaf, index) = self.locate(Entry::Tab(tab))?;
        let focused = self.focused_tab() == Some(tab);
        let held: HashSet<TabId> = self.all_tabs().into_iter().collect();
        let group = self.build_group(key, true, root, &|inner| !held.contains(&inner))?;
        self.forget_hosting_tab(tab);
        if let Some(holder) = self.leaf_mut(leaf) {
            holder.entries[index] = Entry::Group(group);
        }
        let shown = focus
            .filter(|shown| self.group_tabs(group).contains(shown))
            .and_then(|shown| self.locate(Entry::Tab(shown)));
        if let Some((inner, at)) = shown {
            if let Some(holder) = self.leaf_mut(inner) {
                holder.active = at;
            }
            if focused {
                self.focus(inner);
                self.normalize();
                self.settle_focus();
                return Some(group);
            }
        }
        if focused {
            let inner = self
                .tree_leaves(Tree::Group(group))
                .into_iter()
                .find(|leaf| !self.entries(*leaf).is_empty());
            if let Some(inner) = inner {
                self.focus(inner);
            }
        }
        self.normalize();
        self.settle_focus();
        Some(group)
    }

    pub fn unhost_group(&mut self, group: GroupId, tab: TabId) {
        let inside = self.group_tabs(group);
        let focused = self
            .focus
            .is_some_and(|focus| self.within(focus, Tree::Group(group)));
        let holder = self.holder(group).filter(|_| !self.contains(tab));
        let nested = self.nested_groups(Tree::Group(group));
        for parked in std::iter::once(group).chain(nested.iter().copied()) {
            self.park_lock(parked);
        }
        self.invalidate();
        self.groups
            .retain(|candidate| candidate.id != group && !nested.contains(&candidate.id));
        for inner in inside {
            self.forget_tab(inner);
        }
        match holder.and_then(|(leaf, index)| Some((self.leaf_mut(leaf)?, leaf, index))) {
            Some((held, leaf, index)) => {
                held.entries[index] = Entry::Tab(tab);
                if focused {
                    self.focus(leaf);
                }
            }
            None => {
                let orphan: Vec<(LeafId, usize)> = self
                    .surfaces
                    .iter()
                    .flat_map(|surface| self.leaves(surface.id))
                    .flat_map(|leaf| {
                        self.entries(leaf)
                            .into_iter()
                            .enumerate()
                            .filter(|(_, entry)| *entry == Entry::Group(group))
                            .map(move |(index, _)| (leaf, index))
                    })
                    .collect();
                for (leaf, index) in orphan.into_iter().rev() {
                    if let Some(held) = self.leaf_mut(leaf) {
                        held.entries.remove(index);
                        held.active = held.active.min(held.entries.len().saturating_sub(1));
                    }
                }
            }
        }
        self.normalize();
        self.settle_focus();
    }

    fn forget_hosting_tab(&mut self, tab: TabId) {
        self.recent.retain(|other| *other != tab);
        self.forget_visits(tab);
        if self.fullscreen.is_some_and(|shown| shown.tab == tab) {
            self.fullscreen = None;
        }
    }

    pub fn hosted_by(&self, tab: TabId) -> Option<GroupId> {
        let key = joined_key(tab);
        self.groups
            .iter()
            .find(|group| group.key.as_deref() == Some(key.as_str()))
            .map(|group| group.id)
    }

    pub fn stands_for(&self, group: GroupId) -> Option<TabId> {
        let host = joined_host(self.group(group)?.key.as_deref()?)?;
        let [leaf] = self.tree_leaves(Tree::Group(group))[..] else {
            return None;
        };
        self.is_bare(leaf).then_some(host)
    }

    fn forget_tab(&mut self, tab: TabId) {
        self.homes.retain(|home| home.tab != tab);
        self.recent.retain(|other| *other != tab);
        self.forget_visits(tab);
        if self.fullscreen.is_some_and(|shown| shown.tab == tab) {
            self.fullscreen = None;
        }
    }

    pub fn subtree(&self, group: GroupId, map: &DockMapping<'_>) -> DockState {
        let mut out = DockState::new(Vec::new());
        let mut copied = Copied::default();
        let root = self
            .group(group)
            .and_then(|held| out.copy_node(self, &held.root, map, &mut copied));
        let main = out.main();
        if let Some(root) = root
            && let Some(surface) = out.surface_mut(main)
        {
            surface.root = root;
        }
        out.focus = self
            .focus
            .and_then(|focus| copied.leaves.get(&focus).copied())
            .or_else(|| out.leaves(main).first().copied());
        out.recent = self
            .recent
            .iter()
            .filter_map(|tab| (map.tab)(*tab))
            .filter(|tab| out.contains(*tab))
            .collect();
        out.history = self
            .history
            .iter()
            .filter_map(|visit| {
                Some(DockVisit {
                    from: (map.tab)(visit.from)?,
                    to: (map.tab)(visit.to)?,
                    opened: visit.opened,
                })
            })
            .collect();
        for home in &self.homes {
            let Some(tab) = (map.tab)(home.tab).filter(|tab| out.contains(*tab)) else {
                continue;
            };
            let place = match &home.place {
                DockHome::Area(area) => (map.key)(area).map(DockHome::Area),
                DockHome::Group(group) => copied.groups.get(group).copied().map(DockHome::Group),
            };
            if let Some(place) = place {
                out.homes.push(Home {
                    tab,
                    place,
                    within: None,
                });
            }
        }
        let host = self
            .group(group)
            .and_then(|held| joined_host(held.key.as_deref()?));
        out.bars = self
            .bars
            .iter()
            .filter_map(|bar| {
                let owner = match bar.owner? {
                    owner if Some(owner) == host => None,
                    owner => Some((map.tab)(owner)?),
                };
                Some(Bar {
                    bar: (map.tab)(bar.bar)?,
                    owner,
                    placement: bar.placement,
                    hidden: bar.hidden,
                })
            })
            .collect();
        out.unlocked = self
            .unlocked
            .iter()
            .filter_map(|place| match place {
                DockHome::Area(area) => (map.key)(area).map(DockHome::Area),
                DockHome::Group(group) => copied.groups.get(group).copied().map(DockHome::Group),
            })
            .collect();
        out.seeded = true;
        out
    }

    pub fn graft(&mut self, group: GroupId, from: &DockState, map: &DockMapping<'_>) {
        if self.group(group).is_none() {
            return;
        }
        let focused = self
            .focus
            .is_some_and(|focus| self.within(focus, Tree::Group(group)));
        let source = from.main();
        let arriving: Vec<(TabId, TabId)> = from
            .surface_tabs(source)
            .into_iter()
            .filter_map(|inner| Some((inner, (map.tab)(inner)?)))
            .collect();
        let nested = self.nested_groups(Tree::Group(group));
        let joined: HashSet<GroupId> = nested
            .iter()
            .copied()
            .filter(|nested| {
                self.group(*nested)
                    .and_then(|held| joined_host(held.key.as_deref()?))
                    .is_some()
            })
            .collect();
        let dropped: HashSet<GroupId> = nested
            .iter()
            .copied()
            .filter(|nested| {
                !joined.contains(nested)
                    && !joined
                        .iter()
                        .any(|kept| self.nested_groups(Tree::Group(*kept)).contains(nested))
            })
            .collect();
        self.invalidate();
        self.groups
            .retain(|candidate| !dropped.contains(&candidate.id));
        let emptied = Node::Leaf(self.new_leaf(Vec::new()));
        if let Some(root) = self.root_mut(Tree::Group(group)) {
            *root = emptied;
        }
        for (_, tab) in &arriving {
            self.take(Entry::Tab(*tab));
        }
        let mut copied = Copied::default();
        let root = from
            .root(Tree::Surface(source))
            .and_then(|root| self.copy_node(from, root, map, &mut copied))
            .unwrap_or_else(|| Node::Leaf(self.new_leaf(Vec::new())));
        if let Some(held) = self.root_mut(Tree::Group(group)) {
            *held = root;
        }
        let orphans: Vec<GroupId> = joined
            .into_iter()
            .filter(|orphan| self.holder(*orphan).is_none())
            .collect();
        for orphan in orphans {
            let inner = self.nested_groups(Tree::Group(orphan));
            self.invalidate();
            self.groups
                .retain(|candidate| candidate.id != orphan && !inner.contains(&candidate.id));
        }
        let tabs: Vec<TabId> = arriving.iter().map(|(_, tab)| *tab).collect();
        self.homes.retain(|home| !tabs.contains(&home.tab));
        for (inner, tab) in arriving {
            if !self.contains(tab) {
                continue;
            }
            let given = from.homes.iter().find(|home| home.tab == inner);
            let place = given.and_then(|home| match &home.place {
                DockHome::Area(area) => (map.key)(area).map(DockHome::Area),
                DockHome::Group(held) => copied.groups.get(held).copied().map(DockHome::Group),
            });
            let home = match place {
                Some(place) => Home {
                    tab,
                    place,
                    within: Some(group),
                },
                None => Home {
                    tab,
                    place: DockHome::Group(group),
                    within: None,
                },
            };
            self.homes.push(home);
        }
        self.put_outside(group, from, map);
        let host = self
            .group(group)
            .and_then(|held| joined_host(held.key.as_deref()?));
        for bar in &from.bars {
            let owner = match bar.owner {
                Some(owner) => (map.tab)(owner),
                None => host,
            };
            let (Some(moved), Some(owner)) = ((map.tab)(bar.bar), owner) else {
                continue;
            };
            self.add_bar(moved, Some(owner), bar.placement);
            self.set_bar_placement(moved, bar.placement);
            self.set_bar_hidden(moved, bar.hidden);
        }
        let unlocked: Vec<DockHome> = from
            .unlocked
            .iter()
            .filter_map(|place| match place {
                DockHome::Area(area) => (map.key)(area).map(DockHome::Area),
                DockHome::Group(held) => copied.groups.get(held).copied().map(DockHome::Group),
            })
            .collect();
        let inner: HashSet<String> = std::iter::once(group)
            .chain(self.nested_groups(Tree::Group(group)))
            .flat_map(|tree| self.tree_leaves(Tree::Group(tree)))
            .filter_map(|leaf| self.leaf(leaf)?.area.clone())
            .collect();
        self.unlocked
            .retain(|place| !matches!(place, DockHome::Area(area) if inner.contains(area)));
        for place in unlocked {
            self.set_locked(place, false);
        }
        self.normalize();
        let refocus = from
            .focus
            .and_then(|focus| copied.leaves.get(&focus).copied())
            .filter(|leaf| focused && self.leaf(*leaf).is_some());
        if let Some(leaf) = refocus {
            self.focus = Some(leaf);
        }
        self.settle_focus();
    }

    fn copy_node(
        &mut self,
        from: &DockState,
        node: &Node,
        map: &DockMapping<'_>,
        copied: &mut Copied,
    ) -> Option<Node> {
        match node {
            Node::Leaf(leaf) => {
                let mut entries = Vec::new();
                let mut active = 0;
                for (index, entry) in leaf.entries.iter().enumerate() {
                    let entry = match entry {
                        Entry::Tab(tab) => (map.tab)(*tab).map(|tab| self.hosted_entry(tab)),
                        Entry::Group(group) => self.copy_group(from, *group, map, copied),
                    };
                    let Some(entry) = entry else {
                        continue;
                    };
                    if index == leaf.active {
                        active = entries.len();
                    }
                    entries.push(entry);
                }
                let key = leaf.key.as_deref().and_then(map.key);
                let keep = leaf.keep && key.is_some();
                if entries.is_empty() && !keep {
                    return None;
                }
                let mut copy = self.new_leaf(entries);
                copy.active = active;
                copy.vertical = leaf.vertical;
                copy.icons_only = leaf.icons_only;
                copy.sidebar = leaf.sidebar;
                copy.key = key;
                copy.keep = keep;
                copy.area = leaf.area.as_deref().and_then(map.key);
                copy.bare = leaf.bare;
                copied.leaves.insert(leaf.id, copy.id);
                Some(Node::Leaf(copy))
            }
            Node::Split(split) => {
                let first = self.copy_node(from, &split.first, map, copied);
                let second = self.copy_node(from, &split.second, map, copied);
                match (first, second) {
                    (Some(first), Some(second)) => Some(Node::Split(Split {
                        id: SplitId(self.mint()),
                        direction: split.direction,
                        fraction: split.fraction,
                        first: Box::new(first),
                        second: Box::new(second),
                        key: split.key.as_deref().and_then(map.key),
                        adjusted: split.adjusted,
                    })),
                    (Some(only), None) | (None, Some(only)) => Some(only),
                    (None, None) => None,
                }
            }
        }
    }

    fn hosted_entry(&mut self, tab: TabId) -> Entry {
        match self.group_by_key(&joined_key(tab)) {
            Some(group) => {
                self.take(Entry::Group(group));
                Entry::Group(group)
            }
            None => Entry::Tab(tab),
        }
    }

    fn copy_group(
        &mut self,
        from: &DockState,
        group: GroupId,
        map: &DockMapping<'_>,
        copied: &mut Copied,
    ) -> Option<Entry> {
        let source = from.group(group)?;
        if let Some(tab) = source.key.as_deref().and_then(map.group) {
            return Some(Entry::Tab(tab));
        }
        let root = self.copy_node(from, &source.root, map, copied)?;
        let id = GroupId(self.mint());
        self.invalidate();
        self.groups.push(Group {
            id,
            root,
            pinned: source.pinned,
            key: source.key.as_deref().and_then(map.key),
        });
        copied.groups.insert(group, id);
        Some(Entry::Group(id))
    }

    pub fn rehome(&mut self, tab: TabId, group: GroupId) {
        let groups: HashSet<GroupId> = self.groups.iter().map(|held| held.id).collect();
        for home in &mut self.homes {
            if home.tab == tab
                && matches!(home.place, DockHome::Group(held) if !groups.contains(&held))
            {
                home.place = DockHome::Group(group);
                home.within = None;
            }
        }
    }

    pub(super) fn home_group(&self, tab: TabId) -> Option<GroupId> {
        let within = self
            .homes
            .iter()
            .find(|home| home.tab == tab)
            .and_then(|home| home.within)
            .filter(|group| self.group(*group).is_some());
        if within.is_some() {
            return within;
        }
        match self.home(tab)? {
            DockHome::Group(group) => Some(group),
            DockHome::Area(area) => {
                let trees = self
                    .surfaces
                    .iter()
                    .map(|surface| Tree::Surface(surface.id))
                    .chain(self.groups.iter().map(|group| Tree::Group(group.id)));
                let leaf = trees
                    .flat_map(|tree| self.tree_leaves(tree))
                    .find(|leaf| self.area_of(*leaf) == Some(area.as_str()))?;
                self.pinned_group_of(leaf)
            }
        }
    }
}
