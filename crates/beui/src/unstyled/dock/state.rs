use crate::base::Direction;
use crate::geometry::{Pos2, Rect, Vec2};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TabId(u64);

impl TabId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LeafId(u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SplitId(u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SurfaceId(u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GroupId(u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Entry {
    Tab(TabId),
    Group(GroupId),
}

impl Entry {
    pub fn tab(self) -> Option<TabId> {
        match self {
            Entry::Tab(tab) => Some(tab),
            Entry::Group(_) => None,
        }
    }

    pub fn group(self) -> Option<GroupId> {
        match self {
            Entry::Tab(_) => None,
            Entry::Group(group) => Some(group),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Tree {
    Surface(SurfaceId),
    Group(GroupId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Side {
    Left,
    Right,
    Above,
    Below,
}

impl Side {
    pub fn direction(self) -> Direction {
        match self {
            Side::Left | Side::Right => Direction::Horizontal,
            Side::Above | Side::Below => Direction::Vertical,
        }
    }

    fn leads(self) -> bool {
        matches!(self, Side::Left | Side::Above)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DockDrop {
    Tab { leaf: LeafId, index: usize },
    Group { leaf: LeafId, index: usize },
    Pane { leaf: LeafId },
    Split { leaf: LeafId, side: Side },
    Window { pos: Pos2 },
}

impl DockDrop {
    pub fn leaf(self) -> Option<LeafId> {
        match self {
            DockDrop::Tab { leaf, .. }
            | DockDrop::Group { leaf, .. }
            | DockDrop::Pane { leaf }
            | DockDrop::Split { leaf, .. } => Some(leaf),
            DockDrop::Window { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TabPosition {
    pub surface: SurfaceId,
    pub leaf: LeafId,
    pub index: usize,
}

pub const MIN_FRACTION: f32 = 0.05;
pub const MIN_WINDOW_SIZE: Vec2 = Vec2::new(200.0, 140.0);
pub const FLOATING_SIZE: Vec2 = Vec2::new(420.0, 300.0);
pub const SIDEBAR_WIDTH: f32 = 180.0;
pub const MIN_SIDEBAR_WIDTH: f32 = 80.0;

#[derive(Clone, Debug, PartialEq)]
struct Leaf {
    id: LeafId,
    entries: Vec<Entry>,
    active: usize,
    vertical: bool,
    sidebar: f32,
}

#[derive(Clone, Debug, PartialEq)]
struct Split {
    id: SplitId,
    direction: Direction,
    fraction: f32,
    first: Box<Node>,
    second: Box<Node>,
}

#[derive(Clone, Debug, PartialEq)]
enum Node {
    Leaf(Leaf),
    Split(Split),
}

impl Node {
    fn walk<'a>(&'a self, out: &mut Vec<&'a Leaf>) {
        match self {
            Node::Leaf(leaf) => out.push(leaf),
            Node::Split(split) => {
                split.first.walk(out);
                split.second.walk(out);
            }
        }
    }

    fn leaf(&self, id: LeafId) -> Option<&Leaf> {
        match self {
            Node::Leaf(leaf) => (leaf.id == id).then_some(leaf),
            Node::Split(split) => split.first.leaf(id).or_else(|| split.second.leaf(id)),
        }
    }

    fn leaf_mut(&mut self, id: LeafId) -> Option<&mut Leaf> {
        match self {
            Node::Leaf(leaf) => (leaf.id == id).then_some(leaf),
            Node::Split(split) => match split.first.leaf_mut(id) {
                Some(leaf) => Some(leaf),
                None => split.second.leaf_mut(id),
            },
        }
    }

    fn first_leaf(&self) -> &Leaf {
        match self {
            Node::Leaf(leaf) => leaf,
            Node::Split(split) => split.first.first_leaf(),
        }
    }

    fn split(&self, id: SplitId) -> Option<&Split> {
        let Node::Split(split) = self else {
            return None;
        };
        if split.id == id {
            return Some(split);
        }
        split.first.split(id).or_else(|| split.second.split(id))
    }

    fn split_mut(&mut self, id: SplitId) -> Option<&mut Split> {
        let Node::Split(split) = self else {
            return None;
        };
        if split.id == id {
            return Some(split);
        }
        match split.first.split_mut(id) {
            Some(found) => Some(found),
            None => split.second.split_mut(id),
        }
    }

    fn replace_leaf<F: FnOnce(Node) -> Node>(&mut self, target: LeafId, build: F) -> bool {
        let Some(slot) = self.slot_of(target) else {
            return false;
        };
        let placeholder = Node::Leaf(Leaf {
            id: target,
            entries: Vec::new(),
            active: 0,
            vertical: false,
            sidebar: SIDEBAR_WIDTH,
        });
        let existing = std::mem::replace(slot, placeholder);
        *slot = build(existing);
        true
    }

    fn slot_of(&mut self, target: LeafId) -> Option<&mut Node> {
        match self {
            Node::Leaf(leaf) if leaf.id == target => Some(self),
            Node::Leaf(_) => None,
            Node::Split(split) => match split.first.slot_of(target) {
                Some(slot) => Some(slot),
                None => split.second.slot_of(target),
            },
        }
    }

    fn without_leaf(self, id: LeafId) -> Option<Node> {
        match self {
            Node::Leaf(leaf) => (leaf.id != id).then_some(Node::Leaf(leaf)),
            Node::Split(split) => {
                let Split {
                    id: split_id,
                    direction,
                    fraction,
                    first,
                    second,
                } = split;
                match (first.without_leaf(id), second.without_leaf(id)) {
                    (Some(first), Some(second)) => Some(Node::Split(Split {
                        id: split_id,
                        direction,
                        fraction,
                        first: Box::new(first),
                        second: Box::new(second),
                    })),
                    (Some(node), None) | (None, Some(node)) => Some(node),
                    (None, None) => None,
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Surface {
    id: SurfaceId,
    root: Node,
    window: Option<Rect>,
}

#[derive(Clone, Debug, PartialEq)]
struct Group {
    id: GroupId,
    root: Node,
    pinned: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DockTree {
    Tabs {
        entries: Vec<DockTreeEntry>,
        active: usize,
        vertical: bool,
        sidebar: f32,
    },
    Split {
        direction: Direction,
        fraction: f32,
        first: Box<DockTree>,
        second: Box<DockTree>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum DockTreeEntry {
    Tab(TabId),
    Group(DockTree),
}

impl Default for DockTree {
    fn default() -> Self {
        DockTree::Tabs {
            entries: Vec::new(),
            active: 0,
            vertical: false,
            sidebar: SIDEBAR_WIDTH,
        }
    }
}

impl DockTree {
    pub fn tabs(&self) -> Vec<TabId> {
        let mut tabs = Vec::new();
        self.collect_tabs(&mut tabs);
        tabs
    }

    fn collect_tabs(&self, out: &mut Vec<TabId>) {
        match self {
            DockTree::Tabs { entries, .. } => {
                for entry in entries {
                    match entry {
                        DockTreeEntry::Tab(tab) => out.push(*tab),
                        DockTreeEntry::Group(group) => group.collect_tabs(out),
                    }
                }
            }
            DockTree::Split { first, second, .. } => {
                first.collect_tabs(out);
                second.collect_tabs(out);
            }
        }
    }

    pub fn without(&self, dropped: &[TabId]) -> DockTree {
        match self {
            DockTree::Tabs {
                entries,
                active,
                vertical,
                sidebar,
            } => {
                let shown = entries.get(*active);
                let kept: Vec<DockTreeEntry> = entries
                    .iter()
                    .filter_map(|entry| match entry {
                        DockTreeEntry::Tab(tab) => {
                            (!dropped.contains(tab)).then_some(DockTreeEntry::Tab(*tab))
                        }
                        DockTreeEntry::Group(group) => {
                            let group = group.without(dropped);
                            (!group.tabs().is_empty()).then_some(DockTreeEntry::Group(group))
                        }
                    })
                    .collect();
                let active = shown
                    .and_then(|shown| kept.iter().position(|entry| entry == shown))
                    .unwrap_or_else(|| (*active).min(kept.len().saturating_sub(1)));
                DockTree::Tabs {
                    entries: kept,
                    active,
                    vertical: *vertical,
                    sidebar: *sidebar,
                }
            }
            DockTree::Split {
                direction,
                fraction,
                first,
                second,
            } => {
                let first = first.without(dropped);
                let second = second.without(dropped);
                match (first.tabs().is_empty(), second.tabs().is_empty()) {
                    (false, false) => DockTree::Split {
                        direction: *direction,
                        fraction: *fraction,
                        first: Box::new(first),
                        second: Box::new(second),
                    },
                    (true, false) => second,
                    (false, true) | (true, true) => first,
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Home {
    tab: TabId,
    group: GroupId,
    pinned: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DockState {
    surfaces: Vec<Surface>,
    groups: Vec<Group>,
    homes: Vec<Home>,
    focus: Option<LeafId>,
    next: u64,
}

impl Default for DockState {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl DockState {
    pub fn new(tabs: impl IntoIterator<Item = TabId>) -> Self {
        let mut state = Self {
            surfaces: Vec::new(),
            groups: Vec::new(),
            homes: Vec::new(),
            focus: None,
            next: 0,
        };
        let leaf = state.new_leaf(tabs.into_iter().map(Entry::Tab).collect());
        let focus = leaf.id;
        let id = SurfaceId(state.mint());
        state.surfaces.push(Surface {
            id,
            root: Node::Leaf(leaf),
            window: None,
        });
        state.focus = Some(focus);
        state
    }

    fn mint(&mut self) -> u64 {
        self.next += 1;
        self.next
    }

    fn new_leaf(&mut self, entries: Vec<Entry>) -> Leaf {
        Leaf {
            id: LeafId(self.mint()),
            entries,
            active: 0,
            vertical: false,
            sidebar: SIDEBAR_WIDTH,
        }
    }

    fn surface(&self, id: SurfaceId) -> Option<&Surface> {
        self.surfaces.iter().find(|surface| surface.id == id)
    }

    fn surface_mut(&mut self, id: SurfaceId) -> Option<&mut Surface> {
        self.surfaces.iter_mut().find(|surface| surface.id == id)
    }

    fn group(&self, id: GroupId) -> Option<&Group> {
        self.groups.iter().find(|group| group.id == id)
    }

    fn root(&self, tree: Tree) -> Option<&Node> {
        match tree {
            Tree::Surface(surface) => self.surface(surface).map(|surface| &surface.root),
            Tree::Group(group) => self.group(group).map(|group| &group.root),
        }
    }

    fn root_mut(&mut self, tree: Tree) -> Option<&mut Node> {
        match tree {
            Tree::Surface(surface) => self.surface_mut(surface).map(|surface| &mut surface.root),
            Tree::Group(group) => self
                .groups
                .iter_mut()
                .find(|candidate| candidate.id == group)
                .map(|group| &mut group.root),
        }
    }

    fn roots(&self) -> impl Iterator<Item = &Node> {
        self.surfaces
            .iter()
            .map(|surface| &surface.root)
            .chain(self.groups.iter().map(|group| &group.root))
    }

    fn every_leaf(&self) -> Vec<&Leaf> {
        let mut leaves = Vec::new();
        for root in self.roots() {
            root.walk(&mut leaves);
        }
        leaves
    }

    fn leaf(&self, id: LeafId) -> Option<&Leaf> {
        self.roots().find_map(|root| root.leaf(id))
    }

    fn leaf_mut(&mut self, id: LeafId) -> Option<&mut Leaf> {
        self.surfaces
            .iter_mut()
            .map(|surface| &mut surface.root)
            .chain(self.groups.iter_mut().map(|group| &mut group.root))
            .find_map(|root| root.leaf_mut(id))
    }

    pub fn main(&self) -> SurfaceId {
        self.surfaces[0].id
    }

    pub fn surfaces(&self) -> Vec<SurfaceId> {
        self.surfaces.iter().map(|surface| surface.id).collect()
    }

    pub fn windows(&self) -> Vec<SurfaceId> {
        self.surfaces
            .iter()
            .filter(|surface| surface.window.is_some())
            .map(|surface| surface.id)
            .collect()
    }

    pub fn window_rect(&self, surface: SurfaceId) -> Option<Rect> {
        self.surface(surface).and_then(|surface| surface.window)
    }

    pub fn set_window_rect(&mut self, surface: SurfaceId, rect: Rect) {
        if let Some(surface) = self.surface_mut(surface)
            && surface.window.is_some()
        {
            surface.window = Some(rect);
        }
    }

    pub fn open_window(&mut self, rect: Rect, tabs: Vec<TabId>) -> SurfaceId {
        self.open_window_with(rect, tabs.into_iter().map(Entry::Tab).collect())
    }

    fn open_window_with(&mut self, rect: Rect, entries: Vec<Entry>) -> SurfaceId {
        let leaf = self.new_leaf(entries);
        let focus = leaf.id;
        let id = SurfaceId(self.mint());
        self.surfaces.push(Surface {
            id,
            root: Node::Leaf(leaf),
            window: Some(rect),
        });
        self.focus = Some(focus);
        id
    }

    pub fn close_window(&mut self, surface: SurfaceId) {
        if self
            .surface(surface)
            .is_none_or(|surface| surface.window.is_none())
        {
            return;
        }
        let nested = self.nested_groups(Tree::Surface(surface));
        self.groups.retain(|group| !nested.contains(&group.id));
        self.surfaces.retain(|candidate| candidate.id != surface);
        self.settle_focus();
    }

    pub fn raise(&mut self, surface: SurfaceId) {
        let Some(index) = self
            .surfaces
            .iter()
            .position(|candidate| candidate.id == surface)
        else {
            return;
        };
        if index == 0 || index + 1 == self.surfaces.len() {
            return;
        }
        let raised = self.surfaces.remove(index);
        self.surfaces.push(raised);
    }

    pub fn tree_of(&self, leaf: LeafId) -> Option<Tree> {
        if let Some(surface) = self
            .surfaces
            .iter()
            .find(|surface| surface.root.leaf(leaf).is_some())
        {
            return Some(Tree::Surface(surface.id));
        }
        self.groups
            .iter()
            .find(|group| group.root.leaf(leaf).is_some())
            .map(|group| Tree::Group(group.id))
    }

    pub fn holder(&self, group: GroupId) -> Option<(LeafId, usize)> {
        self.locate(Entry::Group(group))
    }

    pub fn surface_of(&self, leaf: LeafId) -> Option<SurfaceId> {
        let mut leaf = leaf;
        loop {
            match self.tree_of(leaf)? {
                Tree::Surface(surface) => return Some(surface),
                Tree::Group(group) => leaf = self.holder(group)?.0,
            }
        }
    }

    pub fn is_nested(&self, leaf: LeafId) -> bool {
        matches!(self.tree_of(leaf), Some(Tree::Group(_)))
    }

    pub fn leaves(&self, surface: SurfaceId) -> Vec<LeafId> {
        self.tree_leaves(Tree::Surface(surface))
    }

    pub fn tree_leaves(&self, tree: Tree) -> Vec<LeafId> {
        let Some(root) = self.root(tree) else {
            return Vec::new();
        };
        let mut leaves = Vec::new();
        root.walk(&mut leaves);
        leaves.into_iter().map(|leaf| leaf.id).collect()
    }

    pub fn entries(&self, leaf: LeafId) -> Vec<Entry> {
        self.leaf(leaf)
            .map(|leaf| leaf.entries.clone())
            .unwrap_or_default()
    }

    pub fn active_index(&self, leaf: LeafId) -> usize {
        self.leaf(leaf).map_or(0, |leaf| leaf.active)
    }

    pub fn active_entry(&self, leaf: LeafId) -> Option<Entry> {
        let leaf = self.leaf(leaf)?;
        leaf.entries.get(leaf.active).copied()
    }

    pub fn active_tab(&self, leaf: LeafId) -> Option<TabId> {
        self.active_entry(leaf).and_then(Entry::tab)
    }

    pub fn set_active_index(&mut self, leaf: LeafId, index: usize) {
        if let Some(leaf) = self.leaf_mut(leaf)
            && index < leaf.entries.len()
        {
            leaf.active = index;
        }
    }

    pub fn is_vertical(&self, leaf: LeafId) -> bool {
        self.leaf(leaf).is_some_and(|leaf| leaf.vertical)
    }

    pub fn set_vertical(&mut self, leaf: LeafId, vertical: bool) {
        if let Some(leaf) = self.leaf_mut(leaf) {
            leaf.vertical = vertical;
        }
    }

    pub fn sidebar_width(&self, leaf: LeafId) -> f32 {
        self.leaf(leaf).map_or(SIDEBAR_WIDTH, |leaf| leaf.sidebar)
    }

    pub fn set_sidebar_width(&mut self, leaf: LeafId, width: f32) {
        if let Some(leaf) = self.leaf_mut(leaf) {
            leaf.sidebar = width.max(MIN_SIDEBAR_WIDTH);
        }
    }

    pub fn locate(&self, entry: Entry) -> Option<(LeafId, usize)> {
        self.every_leaf().into_iter().find_map(|leaf| {
            let index = leaf
                .entries
                .iter()
                .position(|candidate| *candidate == entry)?;
            Some((leaf.id, index))
        })
    }

    pub fn find(&self, tab: TabId) -> Option<TabPosition> {
        let (leaf, index) = self.locate(Entry::Tab(tab))?;
        Some(TabPosition {
            surface: self.surface_of(leaf)?,
            leaf,
            index,
        })
    }

    pub fn contains(&self, tab: TabId) -> bool {
        self.find(tab).is_some()
    }

    pub fn all_tabs(&self) -> Vec<TabId> {
        self.surfaces
            .iter()
            .flat_map(|surface| self.tree_tabs(Tree::Surface(surface.id)))
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.all_tabs().is_empty()
    }

    pub fn surface_tabs(&self, surface: SurfaceId) -> Vec<TabId> {
        self.tree_tabs(Tree::Surface(surface))
    }

    pub fn group_tabs(&self, group: GroupId) -> Vec<TabId> {
        self.tree_tabs(Tree::Group(group))
    }

    pub fn entry_tabs(&self, entry: Entry) -> Vec<TabId> {
        match entry {
            Entry::Tab(tab) => vec![tab],
            Entry::Group(group) => self.group_tabs(group),
        }
    }

    fn tree_tabs(&self, tree: Tree) -> Vec<TabId> {
        let Some(root) = self.root(tree) else {
            return Vec::new();
        };
        let mut leaves = Vec::new();
        root.walk(&mut leaves);
        leaves
            .into_iter()
            .flat_map(|leaf| leaf.entries.iter().copied())
            .flat_map(|entry| self.entry_tabs(entry))
            .collect()
    }

    fn nested_groups(&self, tree: Tree) -> Vec<GroupId> {
        let Some(root) = self.root(tree) else {
            return Vec::new();
        };
        let mut leaves = Vec::new();
        root.walk(&mut leaves);
        let mut groups = Vec::new();
        for group in leaves
            .into_iter()
            .flat_map(|leaf| leaf.entries.iter().filter_map(|entry| entry.group()))
        {
            groups.push(group);
            groups.extend(self.nested_groups(Tree::Group(group)));
        }
        groups
    }

    pub fn contains_leaf(&self, entry: Entry, leaf: LeafId) -> bool {
        let Entry::Group(group) = entry else {
            return false;
        };
        let mut leaf = leaf;
        loop {
            match self.tree_of(leaf) {
                Some(Tree::Group(inside)) if inside == group => return true,
                Some(Tree::Group(inside)) => match self.holder(inside) {
                    Some((holder, _)) => leaf = holder,
                    None => return false,
                },
                _ => return false,
            }
        }
    }

    pub fn focused_leaf(&self) -> Option<LeafId> {
        self.focus
    }

    pub fn focused_tab(&self) -> Option<TabId> {
        self.focus.and_then(|leaf| self.active_tab(leaf))
    }

    pub fn focus(&mut self, leaf: LeafId) {
        if self.leaf(leaf).is_none() {
            return;
        }
        self.focus = Some(leaf);
        self.reveal(leaf);
        if let Some(surface) = self.surface_of(leaf) {
            self.raise(surface);
        }
    }

    fn reveal(&mut self, leaf: LeafId) {
        let mut leaf = leaf;
        while let Some(Tree::Group(group)) = self.tree_of(leaf) {
            let Some((holder, index)) = self.holder(group) else {
                return;
            };
            self.set_active_index(holder, index);
            leaf = holder;
        }
    }

    pub fn show(&mut self, tab: TabId) {
        let Some(position) = self.find(tab) else {
            return;
        };
        self.set_active_index(position.leaf, position.index);
        self.focus(position.leaf);
    }

    pub fn push(&mut self, leaf: LeafId, tab: TabId) {
        self.push_entry(leaf, Entry::Tab(tab));
    }

    fn push_entry(&mut self, leaf: LeafId, entry: Entry) {
        let index = self.entries(leaf).len();
        self.insert_entry(leaf, index, entry);
    }

    pub fn insert(&mut self, leaf: LeafId, index: usize, tab: TabId) {
        self.insert_entry(leaf, index, Entry::Tab(tab));
    }

    fn insert_entry(&mut self, leaf: LeafId, index: usize, entry: Entry) {
        let Some(target) = self.leaf_mut(leaf) else {
            return;
        };
        let index = index.min(target.entries.len());
        target.entries.insert(index, entry);
        target.active = index;
        self.focus(leaf);
    }

    pub fn push_to_focused(&mut self, tab: TabId) {
        let leaf = self
            .focus
            .or_else(|| self.leaves(self.main()).first().copied());
        if let Some(leaf) = leaf {
            self.push(leaf, tab);
        }
    }

    pub fn split(
        &mut self,
        leaf: LeafId,
        side: Side,
        share: f32,
        tabs: Vec<TabId>,
    ) -> Option<LeafId> {
        self.split_with(
            leaf,
            side,
            share,
            tabs.into_iter().map(Entry::Tab).collect(),
        )
    }

    fn split_with(
        &mut self,
        leaf: LeafId,
        side: Side,
        share: f32,
        entries: Vec<Entry>,
    ) -> Option<LeafId> {
        let tree = self.tree_of(leaf)?;
        let added = self.new_leaf(entries);
        let id = added.id;
        let split = SplitId(self.mint());
        let share = share.clamp(MIN_FRACTION, 1.0 - MIN_FRACTION);
        let fraction = match side.leads() {
            true => share,
            false => 1.0 - share,
        };
        let direction = side.direction();
        let leads = side.leads();
        let root = self.root_mut(tree)?;
        root.replace_leaf(leaf, move |existing| {
            let added = Node::Leaf(added);
            let (first, second) = match leads {
                true => (added, existing),
                false => (existing, added),
            };
            Node::Split(Split {
                id: split,
                direction,
                fraction,
                first: Box::new(first),
                second: Box::new(second),
            })
        });
        self.focus(id);
        Some(id)
    }

    pub fn remove(&mut self, tab: TabId) -> bool {
        self.homes.retain(|home| home.tab != tab);
        if !self.take(Entry::Tab(tab)) {
            return false;
        }
        self.normalize();
        self.settle_focus();
        true
    }

    pub fn replace(&mut self, tab: TabId, with: TabId) -> bool {
        let Some((leaf, index)) = self.locate(Entry::Tab(tab)) else {
            return false;
        };
        let Some(leaf) = self.leaf_mut(leaf) else {
            return false;
        };
        leaf.entries[index] = Entry::Tab(with);
        true
    }

    pub fn drop_tab(&mut self, tab: TabId, target: DockDrop) {
        self.drop_entry(Entry::Tab(tab), target);
    }

    pub fn drop_entry(&mut self, entry: Entry, target: DockDrop) {
        let Some((source, from)) = self.locate(entry) else {
            return;
        };
        if target
            .leaf()
            .is_some_and(|leaf| self.contains_leaf(entry, leaf))
        {
            return;
        }
        if !self.admits(entry, target) {
            return;
        }
        let alone = self.entries(source).len() == 1;
        match target {
            DockDrop::Tab { leaf, index } if leaf == source => {
                let Some(target) = self.leaf_mut(leaf) else {
                    return;
                };
                let index = index.min(target.entries.len());
                let index = index - usize::from(index > from);
                target.entries.remove(from);
                target.entries.insert(index, entry);
                target.active = index;
                self.focus(leaf);
            }
            DockDrop::Tab { leaf, index } => {
                self.take(entry);
                self.place(leaf, entry, |state, leaf| {
                    state.insert_entry(leaf, index, entry);
                });
            }
            DockDrop::Group { leaf, index } => {
                let Some(onto) = self.entries(leaf).get(index).copied() else {
                    return;
                };
                if onto == entry {
                    return;
                }
                self.take(entry);
                self.join(onto, entry);
            }
            DockDrop::Pane { leaf } => {
                if leaf == source && alone {
                    return;
                }
                self.take(entry);
                self.place(leaf, entry, |state, leaf| state.push_entry(leaf, entry));
            }
            DockDrop::Split { leaf, side } => {
                if leaf == source && alone {
                    return;
                }
                self.take(entry);
                self.place(leaf, entry, |state, leaf| {
                    state.split_with(leaf, side, 0.5, vec![entry]);
                });
            }
            DockDrop::Window { pos } => {
                let window = self
                    .surface_of(source)
                    .filter(|_| !self.is_nested(source))
                    .and_then(|surface| Some((surface, self.window_rect(surface)?)));
                if let Some((surface, rect)) = window
                    && alone
                {
                    self.set_window_rect(surface, Rect::from_min_size(pos, rect.size()));
                    return;
                }
                self.take(entry);
                self.open_window_with(Rect::from_min_size(pos, FLOATING_SIZE), vec![entry]);
            }
        }
        self.normalize();
        self.settle_focus();
    }

    pub fn drop_leaf(&mut self, leaf: LeafId, target: DockDrop) {
        if !self.admits_leaf(leaf, target) {
            return;
        }
        let Some(moved) = self.leaf(leaf).cloned() else {
            return;
        };
        if let [only] = moved.entries.as_slice() {
            let only = *only;
            self.drop_entry(only, target);
            if let Some((landed, _)) = self.locate(only)
                && landed != leaf
                && self.entries(landed).len() == 1
            {
                self.dress(landed, moved.vertical, moved.sidebar);
            }
            return;
        }
        if let Some(onto) = target.leaf()
            && (onto == leaf
                || moved
                    .entries
                    .iter()
                    .any(|entry| self.contains_leaf(*entry, onto)))
        {
            return;
        }
        let shown = moved.entries.get(moved.active).copied();
        let landed = match target {
            DockDrop::Tab { leaf: onto, index } | DockDrop::Group { leaf: onto, index } => {
                self.prune(leaf);
                self.insert_entries(onto, index, moved.entries, shown)
            }
            DockDrop::Pane { leaf: onto } => {
                self.prune(leaf);
                self.insert_entries(onto, usize::MAX, moved.entries, shown)
            }
            DockDrop::Split { leaf: onto, side } => {
                self.prune(leaf);
                let landed = match self.leaf(onto).is_some() {
                    true => self.split_with(onto, side, 0.5, moved.entries),
                    false => self.insert_entries(onto, usize::MAX, moved.entries, shown),
                };
                if let Some(landed) = landed {
                    self.dress(landed, moved.vertical, moved.sidebar);
                }
                landed
            }
            DockDrop::Window { pos } => {
                let window = self
                    .surface_of(leaf)
                    .filter(|_| !self.is_nested(leaf))
                    .filter(|surface| self.leaves(*surface).len() == 1)
                    .and_then(|surface| Some((surface, self.window_rect(surface)?)));
                if let Some((surface, rect)) = window {
                    self.set_window_rect(surface, Rect::from_min_size(pos, rect.size()));
                    return;
                }
                self.prune(leaf);
                let surface =
                    self.open_window_with(Rect::from_min_size(pos, FLOATING_SIZE), moved.entries);
                let landed = self.leaves(surface).first().copied();
                if let Some(landed) = landed {
                    self.dress(landed, moved.vertical, moved.sidebar);
                }
                landed
            }
        };
        if let (Some(landed), Some(shown)) = (landed, shown)
            && let Some(index) = self
                .entries(landed)
                .iter()
                .position(|entry| *entry == shown)
        {
            self.set_active_index(landed, index);
            self.focus(landed);
        }
        self.normalize();
        self.settle_focus();
    }

    fn dress(&mut self, leaf: LeafId, vertical: bool, sidebar: f32) {
        if let Some(leaf) = self.leaf_mut(leaf) {
            leaf.vertical = vertical;
            leaf.sidebar = sidebar;
        }
    }

    fn insert_entries(
        &mut self,
        leaf: LeafId,
        index: usize,
        entries: Vec<Entry>,
        shown: Option<Entry>,
    ) -> Option<LeafId> {
        let leaf = match self.leaf(leaf).is_some() {
            true => leaf,
            false => self.leaves(self.main()).first().copied()?,
        };
        let target = self.leaf_mut(leaf)?;
        let index = index.min(target.entries.len());
        target.entries.splice(index..index, entries);
        if let Some(shown) = shown
            && let Some(active) = target.entries.iter().position(|entry| *entry == shown)
        {
            target.active = active;
        }
        Some(leaf)
    }

    pub fn group_with_next(&mut self, leaf: LeafId, index: usize) {
        let Some(next) = self.entries(leaf).get(index + 1).copied() else {
            return;
        };
        self.drop_entry(next, DockDrop::Group { leaf, index });
    }

    pub fn split_with_next(&mut self, leaf: LeafId, index: usize) {
        let Some(next) = self.entries(leaf).get(index + 1).copied() else {
            return;
        };
        self.drop_entry(next, DockDrop::Group { leaf, index });
        let Some((inner, _)) = self.locate(next) else {
            return;
        };
        if !self.is_nested(inner) {
            return;
        }
        self.drop_entry(
            next,
            DockDrop::Split {
                leaf: inner,
                side: Side::Right,
            },
        );
    }

    pub fn ungroup(&mut self, group: GroupId) {
        if self.is_pinned(group) {
            return;
        }
        let Some((holder, index)) = self.holder(group) else {
            return;
        };
        let entries: Vec<Entry> = self
            .tree_leaves(Tree::Group(group))
            .into_iter()
            .flat_map(|leaf| self.entries(leaf))
            .collect();
        let shown = self
            .focus
            .filter(|focus| self.tree_of(*focus) == Some(Tree::Group(group)))
            .and_then(|focus| self.active_entry(focus))
            .or_else(|| entries.first().copied());
        self.groups.retain(|candidate| candidate.id != group);
        let Some(leaf) = self.leaf_mut(holder) else {
            return;
        };
        leaf.entries.splice(index..=index, entries.iter().copied());
        leaf.active = shown
            .and_then(|shown| entries.iter().position(|entry| *entry == shown))
            .map_or(index, |offset| index + offset);
        self.focus(holder);
        self.normalize();
        self.settle_focus();
    }

    fn place(&mut self, leaf: LeafId, entry: Entry, put: impl FnOnce(&mut Self, LeafId)) {
        if self.leaf(leaf).is_some() {
            put(self, leaf);
            return;
        }
        let fallback = self.leaves(self.main()).first().copied();
        if let Some(fallback) = fallback {
            self.push_entry(fallback, entry);
        }
    }

    fn join(&mut self, onto: Entry, entry: Entry) {
        match onto {
            Entry::Group(group) => {
                let inner = self
                    .focus
                    .filter(|focus| self.tree_of(*focus) == Some(Tree::Group(group)))
                    .or_else(|| self.group(group).map(|group| group.root.first_leaf().id));
                if let Some(inner) = inner {
                    self.push_entry(inner, entry);
                }
            }
            Entry::Tab(_) => {
                let Some((leaf, index)) = self.locate(onto) else {
                    return;
                };
                let mut inner = self.new_leaf(vec![onto, entry]);
                inner.active = 1;
                let inner_id = inner.id;
                let group = GroupId(self.mint());
                self.groups.push(Group {
                    id: group,
                    root: Node::Leaf(inner),
                    pinned: false,
                });
                if let Some(leaf) = self.leaf_mut(leaf) {
                    leaf.entries[index] = Entry::Group(group);
                }
                self.focus(inner_id);
            }
        }
    }

    fn take(&mut self, entry: Entry) -> bool {
        let Some((leaf_id, index)) = self.locate(entry) else {
            return false;
        };
        let Some(leaf) = self.leaf_mut(leaf_id) else {
            return false;
        };
        leaf.entries.remove(index);
        if leaf.active > index || leaf.active >= leaf.entries.len() {
            leaf.active = leaf.active.saturating_sub(1);
        }
        if leaf.entries.is_empty() {
            self.prune(leaf_id);
        }
        true
    }

    fn prune(&mut self, leaf: LeafId) {
        let Some(tree) = self.tree_of(leaf) else {
            return;
        };
        let replacement = LeafId(self.mint());
        let main = self.main();
        let Some(root) = self.root_mut(tree) else {
            return;
        };
        let placeholder = Node::Leaf(Leaf {
            id: replacement,
            entries: Vec::new(),
            active: 0,
            vertical: false,
            sidebar: SIDEBAR_WIDTH,
        });
        let taken = std::mem::replace(root, placeholder);
        match taken.without_leaf(leaf) {
            Some(rest) => *root = rest,
            None => match tree {
                Tree::Surface(surface) if surface == main => {}
                Tree::Surface(surface) => {
                    self.surfaces.retain(|candidate| candidate.id != surface);
                }
                Tree::Group(group) if self.is_pinned(group) => {}
                Tree::Group(group) => {
                    self.take(Entry::Group(group));
                    self.groups.retain(|candidate| candidate.id != group);
                }
            },
        }
    }

    fn normalize(&mut self) {
        while self.normalize_once() {}
    }

    fn normalize_once(&mut self) -> bool {
        let groups: Vec<GroupId> = self
            .groups
            .iter()
            .filter(|group| !group.pinned)
            .map(|group| group.id)
            .collect();
        for group in groups {
            let Some((holder, index)) = self.holder(group) else {
                self.groups.retain(|candidate| candidate.id != group);
                return true;
            };
            let Some(root) = self.group(group).map(|group| group.root.clone()) else {
                continue;
            };
            if let Node::Leaf(inner) = &root
                && inner.entries.len() <= 1
            {
                self.groups.retain(|candidate| candidate.id != group);
                let Some(leaf) = self.leaf_mut(holder) else {
                    return true;
                };
                match inner.entries.first() {
                    Some(only) => leaf.entries[index] = *only,
                    None => {
                        leaf.entries.remove(index);
                        leaf.active = leaf.active.min(leaf.entries.len().saturating_sub(1));
                        if leaf.entries.is_empty() {
                            self.prune(holder);
                        }
                    }
                }
                self.redirect_focus(inner.id, holder);
                return true;
            }
            if self.entries(holder).len() != 1 {
                continue;
            }
            match root {
                Node::Leaf(inner) => {
                    self.groups.retain(|candidate| candidate.id != group);
                    if let Some(leaf) = self.leaf_mut(holder) {
                        leaf.entries = inner.entries;
                        leaf.active = inner.active;
                    }
                    self.redirect_focus(inner.id, holder);
                    return true;
                }
                Node::Split(_) => {
                    let windowed = self
                        .tree_of(holder)
                        .and_then(|tree| match tree {
                            Tree::Surface(surface) => self.window_rect(surface),
                            Tree::Group(_) => None,
                        })
                        .is_some();
                    if windowed {
                        continue;
                    }
                    let Some(tree) = self.tree_of(holder) else {
                        continue;
                    };
                    let first = root.first_leaf().id;
                    self.groups.retain(|candidate| candidate.id != group);
                    if let Some(parent) = self.root_mut(tree) {
                        parent.replace_leaf(holder, move |_| root);
                    }
                    self.redirect_focus(holder, first);
                    return true;
                }
            }
        }
        false
    }

    fn redirect_focus(&mut self, from: LeafId, to: LeafId) {
        if self.focus == Some(from) {
            self.focus = Some(to);
        }
    }

    pub fn is_pinned(&self, group: GroupId) -> bool {
        self.group(group).is_some_and(|group| group.pinned)
    }

    pub fn home(&self, tab: TabId) -> Option<GroupId> {
        self.homes
            .iter()
            .find(|home| home.tab == tab)
            .map(|home| home.group)
    }

    pub fn is_tab_pinned(&self, tab: TabId) -> bool {
        self.homes.iter().any(|home| home.tab == tab && home.pinned)
    }

    pub fn at_home(&self, tab: TabId) -> bool {
        self.home(tab)
            .zip(self.locate(Entry::Tab(tab)))
            .is_some_and(|(group, (leaf, _))| self.contains_leaf(Entry::Group(group), leaf))
    }

    pub fn set_tab_pinned(&mut self, tab: TabId, pinned: bool) {
        if pinned && !self.at_home(tab) {
            return;
        }
        if let Some(home) = self.homes.iter_mut().find(|home| home.tab == tab) {
            home.pinned = pinned;
        }
    }

    pub fn pinned_group_of(&self, leaf: LeafId) -> Option<GroupId> {
        let mut leaf = leaf;
        loop {
            match self.tree_of(leaf)? {
                Tree::Surface(_) => return None,
                Tree::Group(group) if self.is_pinned(group) => return Some(group),
                Tree::Group(group) => leaf = self.holder(group)?.0,
            }
        }
    }

    pub fn admits(&self, entry: Entry, target: DockDrop) -> bool {
        let destination = match target {
            DockDrop::Window { .. } => None,
            DockDrop::Group { leaf, index } => match self.entries(leaf).get(index) {
                Some(Entry::Group(group)) if self.is_pinned(*group) => Some(*group),
                _ => self.pinned_group_of(leaf),
            },
            DockDrop::Tab { leaf, .. } | DockDrop::Pane { leaf } | DockDrop::Split { leaf, .. } => {
                self.pinned_group_of(leaf)
            }
        };
        let tabs = self.entry_tabs(entry);
        let held = tabs.iter().any(|tab| {
            let Some(home) = self.homes.iter().find(|home| home.tab == *tab && home.pinned) else {
                return false;
            };
            entry != Entry::Group(home.group) && !self.lands_in(target, home.group)
        });
        if held {
            return false;
        }
        let Some(destination) = destination else {
            return true;
        };
        if matches!(entry, Entry::Group(group) if self.is_pinned(group)) {
            return false;
        }
        tabs.into_iter()
            .all(|tab| self.home(tab) == Some(destination))
    }

    pub fn admits_leaf(&self, leaf: LeafId, target: DockDrop) -> bool {
        self.entries(leaf)
            .into_iter()
            .all(|entry| self.admits(entry, target))
    }

    fn lands_in(&self, target: DockDrop, group: GroupId) -> bool {
        if let DockDrop::Group { leaf, index } = target
            && self.entries(leaf).get(index) == Some(&Entry::Group(group))
        {
            return true;
        }
        target
            .leaf()
            .is_some_and(|leaf| self.contains_leaf(Entry::Group(group), leaf))
    }

    pub fn insert_pinned_group(
        &mut self,
        leaf: LeafId,
        index: usize,
        layout: &DockTree,
    ) -> GroupId {
        let group = GroupId(self.mint());
        let root = Node::Leaf(self.new_leaf(Vec::new()));
        self.groups.push(Group {
            id: group,
            root,
            pinned: true,
        });
        if let Some(target) = self.leaf_mut(leaf) {
            let index = index.min(target.entries.len());
            target.entries.insert(index, Entry::Group(group));
            target.active = index;
        }
        self.set_tree(Tree::Group(group), layout);
        group
    }

    pub fn unpin(&mut self, group: GroupId) {
        let Some(pinned) = self
            .groups
            .iter_mut()
            .find(|candidate| candidate.id == group)
        else {
            return;
        };
        pinned.pinned = false;
        self.homes.retain(|home| home.group != group);
        self.normalize();
        self.settle_focus();
    }

    pub fn from_tree(layout: &DockTree) -> Self {
        let mut state = Self::new(Vec::new());
        let main = Tree::Surface(state.main());
        state.set_tree(main, layout);
        state
    }

    pub fn tree(&self, tree: Tree) -> Option<DockTree> {
        self.root(tree).map(|root| self.export(root))
    }

    fn export(&self, node: &Node) -> DockTree {
        match node {
            Node::Leaf(leaf) => DockTree::Tabs {
                entries: leaf
                    .entries
                    .iter()
                    .map(|entry| match entry {
                        Entry::Tab(tab) => DockTreeEntry::Tab(*tab),
                        Entry::Group(group) => {
                            DockTreeEntry::Group(self.tree(Tree::Group(*group)).unwrap_or_default())
                        }
                    })
                    .collect(),
                active: leaf.active,
                vertical: leaf.vertical,
                sidebar: leaf.sidebar,
            },
            Node::Split(split) => DockTree::Split {
                direction: split.direction,
                fraction: split.fraction,
                first: Box::new(self.export(&split.first)),
                second: Box::new(self.export(&split.second)),
            },
        }
    }

    pub fn set_tree(&mut self, tree: Tree, layout: &DockTree) {
        if self.tree(tree).as_ref() == Some(layout) || self.root(tree).is_none() {
            return;
        }
        let shown = self
            .focus
            .filter(|focus| self.within(*focus, tree))
            .and_then(|focus| self.active_entry(focus))
            .and_then(Entry::tab);
        let nested = self.nested_groups(tree);
        self.groups.retain(|group| !nested.contains(&group.id));
        let emptied = Node::Leaf(self.new_leaf(Vec::new()));
        if let Some(root) = self.root_mut(tree) {
            *root = emptied;
        }
        let incoming = layout.tabs();
        for tab in &incoming {
            self.take(Entry::Tab(*tab));
        }
        let built = self.build(layout);
        if let Some(root) = self.root_mut(tree) {
            *root = built;
        }
        if let Tree::Group(group) = tree
            && self.is_pinned(group)
        {
            let kept = self.homes.clone();
            self.homes.retain(|home| !incoming.contains(&home.tab));
            self.homes.extend(incoming.iter().map(|tab| Home {
                tab: *tab,
                group,
                pinned: !kept
                    .iter()
                    .any(|home| home.tab == *tab && home.group == group && !home.pinned),
            }));
        }
        self.normalize();
        let refocus = shown
            .and_then(|tab| self.locate(Entry::Tab(tab)))
            .map(|(leaf, _)| leaf);
        match refocus {
            Some(leaf) => self.focus = Some(leaf),
            None => self.settle_focus(),
        }
    }

    fn within(&self, leaf: LeafId, tree: Tree) -> bool {
        let mut leaf = leaf;
        loop {
            let Some(found) = self.tree_of(leaf) else {
                return false;
            };
            if found == tree {
                return true;
            }
            match found {
                Tree::Surface(_) => return false,
                Tree::Group(group) => match self.holder(group) {
                    Some((holder, _)) => leaf = holder,
                    None => return false,
                },
            }
        }
    }

    fn build(&mut self, layout: &DockTree) -> Node {
        match layout {
            DockTree::Tabs {
                entries,
                active,
                vertical,
                sidebar,
            } => {
                let entries: Vec<Entry> = entries
                    .iter()
                    .map(|entry| match entry {
                        DockTreeEntry::Tab(tab) => Entry::Tab(*tab),
                        DockTreeEntry::Group(inner) => {
                            let root = self.build(inner);
                            let group = GroupId(self.mint());
                            self.groups.push(Group {
                                id: group,
                                root,
                                pinned: false,
                            });
                            Entry::Group(group)
                        }
                    })
                    .collect();
                let mut leaf = self.new_leaf(entries);
                leaf.active = (*active).min(leaf.entries.len().saturating_sub(1));
                leaf.vertical = *vertical;
                leaf.sidebar = sidebar.max(MIN_SIDEBAR_WIDTH);
                Node::Leaf(leaf)
            }
            DockTree::Split {
                direction,
                fraction,
                first,
                second,
            } => {
                let first = self.build(first);
                let second = self.build(second);
                Node::Split(Split {
                    id: SplitId(self.mint()),
                    direction: *direction,
                    fraction: fraction.clamp(MIN_FRACTION, 1.0 - MIN_FRACTION),
                    first: Box::new(first),
                    second: Box::new(second),
                })
            }
        }
    }

    pub fn split_direction(&self, split: SplitId) -> Option<Direction> {
        self.roots()
            .find_map(|root| root.split(split))
            .map(|split| split.direction)
    }

    pub fn split_fraction(&self, split: SplitId) -> f32 {
        self.roots()
            .find_map(|root| root.split(split))
            .map_or(0.5, |split| split.fraction)
    }

    pub fn set_split_fraction(&mut self, split: SplitId, fraction: f32) {
        let fraction = fraction.clamp(MIN_FRACTION, 1.0 - MIN_FRACTION);
        for root in self
            .surfaces
            .iter_mut()
            .map(|surface| &mut surface.root)
            .chain(self.groups.iter_mut().map(|group| &mut group.root))
        {
            if let Some(found) = root.split_mut(split) {
                found.fraction = fraction;
                return;
            }
        }
    }

    fn settle_focus(&mut self) {
        if self.focus.is_some_and(|leaf| self.leaf(leaf).is_some()) {
            return;
        }
        self.focus = self
            .surfaces
            .last()
            .map(|surface| surface.root.first_leaf().id);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DockSplitter {
    pub id: SplitId,
    pub direction: Direction,
    pub handle: Rect,
    pub area: Rect,
    pub fraction: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DockLayout {
    pub leaves: Vec<(LeafId, Rect)>,
    pub splitters: Vec<DockSplitter>,
}

impl DockLayout {
    pub fn leaf_rect(&self, leaf: LeafId) -> Option<Rect> {
        self.leaves
            .iter()
            .find_map(|(id, rect)| (*id == leaf).then_some(*rect))
    }

    pub fn leaf_at(&self, pos: Pos2) -> Option<LeafId> {
        self.leaves
            .iter()
            .find_map(|(id, rect)| rect.contains(pos).then_some(*id))
    }

    pub fn splitter(&self, split: SplitId) -> Option<DockSplitter> {
        self.splitters
            .iter()
            .find(|splitter| splitter.id == split)
            .copied()
    }
}

pub fn layout_surface(
    state: &DockState,
    surface: SurfaceId,
    area: Rect,
    thickness: f32,
) -> DockLayout {
    layout_tree(state, Tree::Surface(surface), area, thickness)
}

pub fn layout_tree(state: &DockState, tree: Tree, area: Rect, thickness: f32) -> DockLayout {
    let mut layout = DockLayout::default();
    if let Some(root) = state.root(tree) {
        place(root, area, thickness, &mut layout);
    }
    layout
}

fn place(node: &Node, area: Rect, thickness: f32, out: &mut DockLayout) {
    match node {
        Node::Leaf(leaf) => out.leaves.push((leaf.id, area)),
        Node::Split(split) => {
            let (first, handle, second) = divide(area, split.direction, split.fraction, thickness);
            out.splitters.push(DockSplitter {
                id: split.id,
                direction: split.direction,
                handle,
                area,
                fraction: split.fraction,
            });
            place(&split.first, first, thickness, out);
            place(&split.second, second, thickness, out);
        }
    }
}

fn divide(area: Rect, direction: Direction, fraction: f32, thickness: f32) -> (Rect, Rect, Rect) {
    let available = (direction.main(area.size()) - thickness).max(0.0);
    let first = (available * fraction).round();
    match direction {
        Direction::Horizontal => {
            let split = area.left() + first;
            (
                Rect::from_min_max(area.min, Pos2::new(split, area.bottom())),
                Rect::from_min_max(
                    Pos2::new(split, area.top()),
                    Pos2::new(split + thickness, area.bottom()),
                ),
                Rect::from_min_max(Pos2::new(split + thickness, area.top()), area.max),
            )
        }
        Direction::Vertical => {
            let split = area.top() + first;
            (
                Rect::from_min_max(area.min, Pos2::new(area.right(), split)),
                Rect::from_min_max(
                    Pos2::new(area.left(), split),
                    Pos2::new(area.right(), split + thickness),
                ),
                Rect::from_min_max(Pos2::new(area.left(), split + thickness), area.max),
            )
        }
    }
}

pub fn fraction_moved(
    area: Rect,
    direction: Direction,
    thickness: f32,
    fraction: f32,
    moved: f32,
) -> f32 {
    let length = direction.main(area.size()) - thickness;
    if length <= 0.0 {
        return fraction;
    }
    (fraction + moved / length).clamp(MIN_FRACTION, 1.0 - MIN_FRACTION)
}
