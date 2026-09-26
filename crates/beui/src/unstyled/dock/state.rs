use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

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

    fn at(&self, path: &[bool]) -> Option<&Leaf> {
        match (self, path.split_first()) {
            (Node::Leaf(leaf), None) => Some(leaf),
            (Node::Split(split), Some((second, rest))) => match second {
                false => split.first.at(rest),
                true => split.second.at(rest),
            },
            _ => None,
        }
    }

    fn at_mut(&mut self, path: &[bool]) -> Option<&mut Leaf> {
        match (self, path.split_first()) {
            (Node::Leaf(leaf), None) => Some(leaf),
            (Node::Split(split), Some((second, rest))) => match second {
                false => split.first.at_mut(rest),
                true => split.second.at_mut(rest),
            },
            _ => None,
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
}

#[derive(Debug, Default, PartialEq)]
struct Index {
    leaves: HashMap<LeafId, (Tree, Vec<bool>)>,
    entries: HashMap<Entry, (LeafId, usize)>,
}

impl Index {
    fn build(state: &DockState) -> Self {
        let mut index = Self::default();
        let mut path = Vec::new();
        for surface in &state.surfaces {
            index.add(Tree::Surface(surface.id), &surface.root, &mut path);
        }
        for group in &state.groups {
            index.add(Tree::Group(group.id), &group.root, &mut path);
        }
        index
    }

    fn add(&mut self, tree: Tree, node: &Node, path: &mut Vec<bool>) {
        match node {
            Node::Leaf(leaf) => {
                self.leaves.entry(leaf.id).or_insert((tree, path.clone()));
                for (position, entry) in leaf.entries.iter().enumerate() {
                    self.entries.entry(*entry).or_insert((leaf.id, position));
                }
            }
            Node::Split(split) => {
                path.push(false);
                self.add(tree, &split.first, path);
                path.pop();
                path.push(true);
                self.add(tree, &split.second, path);
                path.pop();
            }
        }
    }
}

#[derive(Default)]
struct Lookup(RefCell<Option<Rc<Index>>>);

impl Clone for Lookup {
    fn clone(&self) -> Self {
        Self(RefCell::new(self.0.borrow().clone()))
    }
}

impl PartialEq for Lookup {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl fmt::Debug for Lookup {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Lookup")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DockState {
    surfaces: Vec<Surface>,
    groups: Vec<Group>,
    focus: Option<LeafId>,
    next: u64,
    lookup: Lookup,
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
            focus: None,
            next: 0,
            lookup: Lookup::default(),
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

    fn index(&self) -> Rc<Index> {
        let cached = self.lookup.0.borrow().clone();
        let index = match cached {
            Some(index) => index,
            None => {
                let index = Rc::new(Index::build(self));
                *self.lookup.0.borrow_mut() = Some(Rc::clone(&index));
                index
            }
        };
        #[cfg(test)]
        assert_eq!(
            *index,
            Index::build(self),
            "the dock lookup went stale after a change"
        );
        index
    }

    fn invalidate(&mut self) {
        *self.lookup.0.get_mut() = None;
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
        self.invalidate();
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
        self.invalidate();
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

    fn leaf(&self, id: LeafId) -> Option<&Leaf> {
        let index = self.index();
        let (tree, path) = index.leaves.get(&id)?;
        self.root(*tree)?.at(path)
    }

    fn leaf_mut(&mut self, id: LeafId) -> Option<&mut Leaf> {
        let (tree, path) = self.index().leaves.get(&id)?.clone();
        self.root_mut(tree)?.at_mut(&path)
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
        self.invalidate();
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
        self.invalidate();
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
        self.invalidate();
        self.surfaces.push(raised);
    }

    pub fn tree_of(&self, leaf: LeafId) -> Option<Tree> {
        self.index().leaves.get(&leaf).map(|(tree, _)| *tree)
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
        self.index().entries.get(&entry).copied()
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
        self.invalidate();
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
                self.invalidate();
                self.groups.push(Group {
                    id: group,
                    root: Node::Leaf(inner),
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
                    self.invalidate();
                    self.surfaces.retain(|candidate| candidate.id != surface);
                }
                Tree::Group(group) => {
                    self.take(Entry::Group(group));
                    self.invalidate();
                    self.groups.retain(|candidate| candidate.id != group);
                }
            },
        }
    }

    fn normalize(&mut self) {
        while self.normalize_once() {}
    }

    fn normalize_once(&mut self) -> bool {
        let groups: Vec<GroupId> = self.groups.iter().map(|group| group.id).collect();
        for group in groups {
            let Some((holder, index)) = self.holder(group) else {
                self.invalidate();
                self.groups.retain(|candidate| candidate.id != group);
                return true;
            };
            let Some(root) = self.group(group).map(|group| group.root.clone()) else {
                continue;
            };
            if let Node::Leaf(inner) = &root
                && inner.entries.len() <= 1
            {
                self.invalidate();
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
                    self.invalidate();
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
                    self.invalidate();
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
