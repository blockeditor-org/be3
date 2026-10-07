use std::collections::HashSet;

use beui_core::base::Direction;
use beui_core::geometry::{Pos2, Rect};

use super::{
    DockState, Entry, FLOATING_SIZE, Group, GroupId, Home, Leaf, LeafId, MIN_FRACTION,
    MIN_SIDEBAR_WIDTH, Node, Side, Split, SplitId, Surface, SurfaceId, TabId, Tree,
};

const FLOAT_ORIGIN: Pos2 = Pos2::new(64.0, 48.0);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DockSpec {
    pub main: Option<DockSpecNode>,
    pub windows: Vec<DockSpecWindow>,
    pub focus: Option<TabId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DockSpecWindow {
    pub key: String,
    pub rect: Rect,
    pub root: Option<DockSpecNode>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DockSpecNode {
    Split {
        key: String,
        direction: Direction,
        fraction: f32,
        first: Box<DockSpecNode>,
        second: Box<DockSpecNode>,
    },
    Pane(DockSpecPane),
}

#[derive(Clone, Debug, PartialEq)]
pub struct DockSpecPane {
    pub key: String,
    pub entries: Vec<DockSpecEntry>,
    pub active: Option<TabId>,
    pub vertical: bool,
    pub sidebar: f32,
    pub keep: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DockSpecEntry {
    Tab(TabId),
    Group {
        key: String,
        pinned: bool,
        root: Box<DockSpecNode>,
    },
}

#[derive(Clone, Copy)]
enum Holder<'a> {
    Main,
    Window(&'a DockSpecWindow),
    Group(&'a str),
}

#[derive(Clone, Copy)]
struct Step<'a> {
    split: &'a DockSpecNode,
    first: bool,
}

struct Site<'a> {
    pane: &'a DockSpecPane,
    path: Vec<usize>,
    holder: Holder<'a>,
    steps: Vec<Step<'a>>,
}

#[derive(Default)]
struct Keys {
    panes: HashSet<String>,
    splits: HashSet<String>,
    windows: HashSet<String>,
    groups: HashSet<String>,
}

impl DockSpec {
    pub fn tabs(&self) -> Vec<TabId> {
        let mut tabs = Vec::new();
        for site in self.sites() {
            for entry in &site.pane.entries {
                if let DockSpecEntry::Tab(tab) = entry {
                    tabs.push(*tab);
                }
            }
        }
        tabs
    }

    fn sites(&self) -> Vec<Site<'_>> {
        let mut sites = Vec::new();
        if let Some(main) = &self.main {
            collect(main, vec![0], Holder::Main, Vec::new(), &mut sites);
        }
        for (index, window) in self.windows.iter().enumerate() {
            if let Some(root) = &window.root {
                collect(
                    root,
                    vec![index + 1],
                    Holder::Window(window),
                    Vec::new(),
                    &mut sites,
                );
            }
        }
        sites
    }

    fn keys(&self) -> Keys {
        let mut keys = Keys::default();
        if let Some(main) = &self.main {
            gather(main, &mut keys);
        }
        for window in &self.windows {
            keys.windows.insert(window.key.clone());
            if let Some(root) = &window.root {
                gather(root, &mut keys);
            }
        }
        keys
    }
}

fn collect<'a>(
    node: &'a DockSpecNode,
    path: Vec<usize>,
    holder: Holder<'a>,
    steps: Vec<Step<'a>>,
    sites: &mut Vec<Site<'a>>,
) {
    match node {
        DockSpecNode::Split { first, second, .. } => {
            for (index, child) in [first, second].into_iter().enumerate() {
                let mut path = path.clone();
                path.push(index);
                let mut steps = steps.clone();
                steps.push(Step {
                    split: node,
                    first: index == 0,
                });
                collect(child, path, holder, steps, sites);
            }
        }
        DockSpecNode::Pane(pane) => {
            sites.push(Site {
                pane,
                path: path.clone(),
                holder,
                steps,
            });
            for (index, entry) in pane.entries.iter().enumerate() {
                if let DockSpecEntry::Group { key, root, .. } = entry {
                    let mut path = path.clone();
                    path.push(index);
                    collect(root, path, Holder::Group(key), Vec::new(), sites);
                }
            }
        }
    }
}

fn gather(node: &DockSpecNode, keys: &mut Keys) {
    match node {
        DockSpecNode::Split {
            key, first, second, ..
        } => {
            keys.splits.insert(key.clone());
            gather(first, keys);
            gather(second, keys);
        }
        DockSpecNode::Pane(pane) => {
            keys.panes.insert(pane.key.clone());
            for entry in &pane.entries {
                if let DockSpecEntry::Group { key, root, .. } = entry {
                    keys.groups.insert(key.clone());
                    gather(root, keys);
                }
            }
        }
    }
}

fn distance(first: &[usize], second: &[usize]) -> usize {
    let shared = first
        .iter()
        .zip(second)
        .take_while(|(first, second)| first == second)
        .count();
    first.len() + second.len() - 2 * shared
}

fn visit_leaves(node: &mut Node, visit: &mut impl FnMut(&mut Leaf)) {
    match node {
        Node::Leaf(leaf) => visit(leaf),
        Node::Split(split) => {
            visit_leaves(&mut split.first, visit);
            visit_leaves(&mut split.second, visit);
        }
    }
}

fn visit_splits(node: &mut Node, visit: &mut impl FnMut(&mut Split)) {
    if let Node::Split(split) = node {
        visit(split);
        visit_splits(&mut split.first, visit);
        visit_splits(&mut split.second, visit);
    }
}

fn find_leaf(node: &Node, key: &str) -> Option<LeafId> {
    match node {
        Node::Leaf(leaf) => (leaf.key.as_deref() == Some(key)).then_some(leaf.id),
        Node::Split(split) => {
            find_leaf(&split.first, key).or_else(|| find_leaf(&split.second, key))
        }
    }
}

fn has_split(node: &Node, key: &str) -> bool {
    match node {
        Node::Leaf(_) => false,
        Node::Split(split) => {
            split.key.as_deref() == Some(key)
                || has_split(&split.first, key)
                || has_split(&split.second, key)
        }
    }
}

impl DockState {
    pub fn is_seeded(&self) -> bool {
        self.seeded
    }

    pub fn mark_seeded(&mut self) {
        self.seeded = true;
    }

    pub fn reconcile(&mut self, spec: &DockSpec) {
        if !self.seeded {
            self.seed(spec);
            return;
        }
        let declared: HashSet<TabId> = spec.tabs().into_iter().collect();
        for tab in self.all_tabs() {
            if !declared.contains(&tab) {
                self.close(tab);
            }
        }
        let sites = spec.sites();
        self.forget_keys(&spec.keys());
        self.sync(spec, &sites);
        self.restore_kept(&sites);
        self.place_missing(&sites);
        self.normalize();
        self.settle_focus();
    }

    fn seed(&mut self, spec: &DockSpec) {
        let main = self.main();
        self.invalidate();
        self.groups.clear();
        self.homes.clear();
        self.recent.clear();
        self.surfaces.retain(|surface| surface.id == main);
        let root = match &spec.main {
            Some(node) => self.build_spec(node, &|_| true),
            None => None,
        };
        let root = root.unwrap_or_else(|| Node::Leaf(self.new_leaf(Vec::new())));
        if let Some(surface) = self.surface_mut(main) {
            surface.root = root;
        }
        for window in &spec.windows {
            let Some(root) = window
                .root
                .as_ref()
                .and_then(|root| self.build_spec(root, &|_| true))
            else {
                continue;
            };
            self.add_window(window.rect, Some(window.key.clone()), root);
        }
        self.seeded = true;
        self.focus = None;
        match spec.focus.filter(|tab| self.contains(*tab)) {
            Some(tab) => self.show(tab),
            None => {
                let leaves = self.leaves(main);
                let leaf = leaves
                    .iter()
                    .copied()
                    .find(|leaf| !self.entries(*leaf).is_empty())
                    .or_else(|| leaves.first().copied());
                self.focus = leaf;
                self.remember_focus();
            }
        }
    }

    fn add_window(&mut self, rect: Rect, key: Option<String>, root: Node) -> SurfaceId {
        let id = SurfaceId(self.mint());
        self.invalidate();
        self.surfaces.push(Surface {
            id,
            root,
            window: Some(rect),
            key,
        });
        id
    }

    fn build_spec(&mut self, node: &DockSpecNode, wanted: &dyn Fn(TabId) -> bool) -> Option<Node> {
        match node {
            DockSpecNode::Split {
                key,
                direction,
                fraction,
                first,
                second,
            } => {
                let first = self.build_spec(first, wanted);
                let second = self.build_spec(second, wanted);
                match (first, second) {
                    (Some(first), Some(second)) => Some(Node::Split(Split {
                        id: SplitId(self.mint()),
                        direction: *direction,
                        fraction: fraction.clamp(MIN_FRACTION, 1.0 - MIN_FRACTION),
                        first: Box::new(first),
                        second: Box::new(second),
                        key: Some(key.clone()),
                        adjusted: false,
                    })),
                    (Some(only), None) | (None, Some(only)) => Some(only),
                    (None, None) => None,
                }
            }
            DockSpecNode::Pane(pane) => {
                let mut entries = Vec::new();
                for entry in &pane.entries {
                    match entry {
                        DockSpecEntry::Tab(tab) if wanted(*tab) => entries.push(Entry::Tab(*tab)),
                        DockSpecEntry::Tab(_) => {}
                        DockSpecEntry::Group { key, pinned, root } => {
                            if let Some(group) = self.build_group(key, *pinned, root, wanted) {
                                entries.push(Entry::Group(group));
                            }
                        }
                    }
                }
                if entries.is_empty() && !pane.keep {
                    return None;
                }
                Some(Node::Leaf(self.spec_leaf(pane, entries)))
            }
        }
    }

    fn spec_leaf(&mut self, pane: &DockSpecPane, entries: Vec<Entry>) -> Leaf {
        let mut leaf = self.new_leaf(entries);
        leaf.active = pane
            .active
            .and_then(|tab| {
                leaf.entries
                    .iter()
                    .position(|entry| *entry == Entry::Tab(tab))
            })
            .unwrap_or(0);
        leaf.vertical = pane.vertical;
        leaf.sidebar = pane.sidebar.max(MIN_SIDEBAR_WIDTH);
        leaf.key = Some(pane.key.clone());
        leaf.keep = pane.keep;
        leaf
    }

    fn build_group(
        &mut self,
        key: &str,
        pinned: bool,
        root: &DockSpecNode,
        wanted: &dyn Fn(TabId) -> bool,
    ) -> Option<GroupId> {
        let root = self.build_spec(root, wanted)?;
        let group = GroupId(self.mint());
        self.invalidate();
        self.groups.push(Group {
            id: group,
            root,
            pinned,
            key: Some(key.to_owned()),
        });
        if pinned {
            for tab in self.group_tabs(group) {
                self.homes.retain(|home| home.tab != tab);
                self.homes.push(Home {
                    tab,
                    group,
                    pinned: true,
                });
            }
        }
        Some(group)
    }

    fn roots_mut(&mut self) -> impl Iterator<Item = &mut Node> {
        self.invalidate();
        self.surfaces
            .iter_mut()
            .map(|surface| &mut surface.root)
            .chain(self.groups.iter_mut().map(|group| &mut group.root))
    }

    fn leaf_by_key(&self, key: &str) -> Option<LeafId> {
        self.roots().find_map(|root| find_leaf(root, key))
    }

    fn split_exists(&self, key: &str) -> bool {
        self.roots().any(|root| has_split(root, key))
    }

    fn group_by_key(&self, key: &str) -> Option<GroupId> {
        self.groups
            .iter()
            .find(|group| group.key.as_deref() == Some(key))
            .map(|group| group.id)
    }

    fn surface_by_key(&self, key: &str) -> Option<SurfaceId> {
        self.surfaces
            .iter()
            .find(|surface| surface.key.as_deref() == Some(key))
            .map(|surface| surface.id)
    }

    fn forget_keys(&mut self, keys: &Keys) {
        let mut loosened = Vec::new();
        for root in self.roots_mut() {
            visit_leaves(root, &mut |leaf| {
                if leaf
                    .key
                    .as_ref()
                    .is_some_and(|key| !keys.panes.contains(key))
                {
                    leaf.key = None;
                    if leaf.keep && leaf.entries.is_empty() {
                        loosened.push(leaf.id);
                    }
                    leaf.keep = false;
                }
            });
            visit_splits(root, &mut |split| {
                if split
                    .key
                    .as_ref()
                    .is_some_and(|key| !keys.splits.contains(key))
                {
                    split.key = None;
                }
            });
        }
        for surface in &mut self.surfaces {
            if surface
                .key
                .as_ref()
                .is_some_and(|key| !keys.windows.contains(key))
            {
                surface.key = None;
            }
        }
        for group in &mut self.groups {
            if group
                .key
                .as_ref()
                .is_some_and(|key| !keys.groups.contains(key))
            {
                group.key = None;
            }
        }
        self.prune_loose(loosened);
    }

    fn prune_loose(&mut self, leaves: Vec<LeafId>) {
        let main = self.main();
        for leaf in leaves {
            if self.leaves(main) == [leaf] || self.is_kept(leaf) {
                continue;
            }
            if self.entries(leaf).is_empty() {
                self.prune(leaf);
            }
        }
    }

    fn sync(&mut self, spec: &DockSpec, sites: &[Site<'_>]) {
        let mut loosened = Vec::new();
        for site in sites {
            let Some(leaf) = self.leaf_by_key(&site.pane.key) else {
                continue;
            };
            let keep = site.pane.keep;
            if let Some(held) = self.leaf_mut(leaf) {
                if held.keep && !keep && held.entries.is_empty() {
                    loosened.push(leaf);
                }
                held.keep = keep;
            }
        }
        self.prune_loose(loosened);
        let mut fractions = Vec::new();
        if let Some(main) = &spec.main {
            split_fractions(main, &mut fractions);
        }
        for window in &spec.windows {
            if let Some(root) = &window.root {
                split_fractions(root, &mut fractions);
            }
        }
        for (key, direction, fraction) in fractions {
            let fraction = fraction.clamp(MIN_FRACTION, 1.0 - MIN_FRACTION);
            for root in self.roots_mut() {
                visit_splits(root, &mut |split| {
                    if split.key.as_deref() == Some(key.as_str())
                        && !split.adjusted
                        && split.direction == direction
                    {
                        split.fraction = fraction;
                    }
                });
            }
        }
    }

    fn restore_kept(&mut self, sites: &[Site<'_>]) {
        for site in sites {
            if !site.pane.keep || self.leaf_by_key(&site.pane.key).is_some() {
                continue;
            }
            let focus = self.focus;
            self.recreate(site);
            self.focus = focus;
            self.settle_focus();
        }
    }

    fn recreate(&mut self, site: &Site<'_>) -> Option<LeafId> {
        let leaf = self.spec_leaf(site.pane, Vec::new());
        for (depth, step) in site.steps.iter().enumerate().rev() {
            let DockSpecNode::Split {
                key,
                direction,
                fraction,
                first,
                second,
            } = step.split
            else {
                continue;
            };
            let sibling = match step.first {
                true => second,
                false => first,
            };
            let Some(anchor) = self.first_present(sibling) else {
                continue;
            };
            let side = side_of(*direction, step.first);
            let share = match step.first {
                true => *fraction,
                false => 1.0 - fraction,
            };
            let named = depth + 1 == site.steps.len()
                && matches!(**sibling, DockSpecNode::Pane(_))
                && !self.split_exists(key);
            let key = named.then(|| key.clone());
            return self.split_off(anchor, side, share, leaf, key);
        }
        let tree = match site.holder {
            Holder::Main => Tree::Surface(self.main()),
            Holder::Window(window) => match self.surface_by_key(&window.key) {
                Some(surface) => Tree::Surface(surface),
                None => {
                    let id = leaf.id;
                    self.add_window(window.rect, Some(window.key.clone()), Node::Leaf(leaf));
                    return Some(id);
                }
            },
            Holder::Group(key) => Tree::Group(self.group_by_key(key)?),
        };
        self.attach(tree, site, leaf)
    }

    fn attach(&mut self, tree: Tree, site: &Site<'_>, leaf: Leaf) -> Option<LeafId> {
        let id = leaf.id;
        let vacant = match self.root(tree)? {
            Node::Leaf(held) => held.entries.is_empty() && !held.keep && held.key.is_none(),
            Node::Split(_) => false,
        };
        if vacant {
            let root = self.root_mut(tree)?;
            *root = Node::Leaf(leaf);
            return Some(id);
        }
        let (side, share) = match site.steps.first().map(|step| (step.split, step.first)) {
            Some((
                DockSpecNode::Split {
                    direction,
                    fraction,
                    ..
                },
                first,
            )) => (
                side_of(*direction, first),
                match first {
                    true => *fraction,
                    false => 1.0 - fraction,
                },
            ),
            _ => (Side::Right, 0.5),
        };
        let split = SplitId(self.mint());
        let share = share.clamp(MIN_FRACTION, 1.0 - MIN_FRACTION);
        let fraction = match side.leads() {
            true => share,
            false => 1.0 - share,
        };
        let root = self.root_mut(tree)?;
        let existing = std::mem::replace(root, Node::Leaf(Leaf::new(id, Vec::new())));
        let added = Node::Leaf(leaf);
        let (first, second) = match side.leads() {
            true => (added, existing),
            false => (existing, added),
        };
        *root = Node::Split(Split {
            id: split,
            direction: side.direction(),
            fraction,
            first: Box::new(first),
            second: Box::new(second),
            key: None,
            adjusted: false,
        });
        Some(id)
    }

    fn first_present(&self, node: &DockSpecNode) -> Option<LeafId> {
        match node {
            DockSpecNode::Split { first, second, .. } => self
                .first_present(first)
                .or_else(|| self.first_present(second)),
            DockSpecNode::Pane(pane) => self.leaf_by_key(&pane.key),
        }
    }

    fn place_missing(&mut self, sites: &[Site<'_>]) {
        for (index, site) in sites.iter().enumerate() {
            for (position, entry) in site.pane.entries.iter().enumerate() {
                match entry {
                    DockSpecEntry::Tab(tab) => {
                        if self.contains(*tab) {
                            continue;
                        }
                        self.settle_entry(sites, index, position, Entry::Tab(*tab));
                        let landed = self.locate(Entry::Tab(*tab)).map(|(leaf, _)| leaf);
                        if let Some(group) = landed.and_then(|leaf| self.pinned_group_of(leaf))
                            && self.home(*tab).is_none()
                        {
                            self.homes.push(Home {
                                tab: *tab,
                                group,
                                pinned: true,
                            });
                        }
                    }
                    DockSpecEntry::Group { key, pinned, root } => {
                        if self.group_by_key(key).is_some() {
                            continue;
                        }
                        let missing: HashSet<TabId> = {
                            let mut tabs = Vec::new();
                            spec_tabs(root, &mut tabs);
                            tabs.into_iter()
                                .filter(|tab| !self.contains(*tab))
                                .collect()
                        };
                        if missing.is_empty() {
                            continue;
                        }
                        let Some(group) =
                            self.build_group(key, *pinned, root, &|tab| missing.contains(&tab))
                        else {
                            continue;
                        };
                        self.settle_entry(sites, index, position, Entry::Group(group));
                    }
                }
            }
        }
    }

    fn settle_entry(&mut self, sites: &[Site<'_>], index: usize, position: usize, entry: Entry) {
        let site = &sites[index];
        if let Some(leaf) = self.leaf_by_key(&site.pane.key) {
            let at = self.neighbour_index(leaf, site.pane, position);
            self.insert_entry(leaf, at, entry);
            return;
        }
        if let Holder::Window(window) = site.holder
            && self.surface_by_key(&window.key).is_none()
        {
            let leaf = self.spec_leaf(site.pane, vec![entry]);
            let id = leaf.id;
            self.add_window(window.rect, Some(window.key.clone()), Node::Leaf(leaf));
            self.focus(id);
            return;
        }
        if let Some((leaf, at)) = self.nearest(sites, index, position, entry) {
            self.insert_entry(leaf, at + 1, entry);
            return;
        }
        let leaf = self.spec_leaf(site.pane, vec![entry]);
        let id = leaf.id;
        let rect = Rect::from_min_size(FLOAT_ORIGIN, FLOATING_SIZE);
        self.add_window(rect, None, Node::Leaf(leaf));
        self.focus(id);
    }

    fn neighbour_index(&self, leaf: LeafId, pane: &DockSpecPane, position: usize) -> usize {
        let held = self.entries(leaf);
        let index_of = |entry: &DockSpecEntry| {
            let entry = match entry {
                DockSpecEntry::Tab(tab) => Entry::Tab(*tab),
                DockSpecEntry::Group { key, .. } => Entry::Group(self.group_by_key(key)?),
            };
            held.iter().position(|candidate| *candidate == entry)
        };
        if let Some(before) = pane.entries[..position].iter().rev().find_map(index_of) {
            return before + 1;
        }
        if let Some(after) = pane.entries[position + 1..].iter().find_map(index_of) {
            return after;
        }
        held.len()
    }

    fn nearest(
        &self,
        sites: &[Site<'_>],
        index: usize,
        position: usize,
        entry: Entry,
    ) -> Option<(LeafId, usize)> {
        let origin = &sites[index].path;
        let mut seen = 0;
        let mut here = 0;
        let mut candidates: Vec<(usize, usize, bool, TabId)> = Vec::new();
        for (other, site) in sites.iter().enumerate() {
            let apart = distance(origin, &site.path);
            for (at, candidate) in site.pane.entries.iter().enumerate() {
                if other == index && at == position {
                    here = seen;
                }
                if let DockSpecEntry::Tab(tab) = candidate {
                    candidates.push((apart, seen, false, *tab));
                    seen += 1;
                }
            }
        }
        for candidate in &mut candidates {
            candidate.2 = candidate.1 >= here;
            candidate.1 = candidate.1.abs_diff(here);
        }
        candidates.sort_by_key(|(apart, gap, after, _)| (*apart, *gap, *after));
        let own = self.entry_tabs(entry);
        candidates.into_iter().find_map(|(_, _, _, tab)| {
            if own.contains(&tab) {
                return None;
            }
            let (leaf, at) = self.locate(Entry::Tab(tab))?;
            self.pinned_group_of(leaf).is_none().then_some((leaf, at))
        })
    }
}

fn spec_tabs(node: &DockSpecNode, out: &mut Vec<TabId>) {
    match node {
        DockSpecNode::Split { first, second, .. } => {
            spec_tabs(first, out);
            spec_tabs(second, out);
        }
        DockSpecNode::Pane(pane) => {
            for entry in &pane.entries {
                match entry {
                    DockSpecEntry::Tab(tab) => out.push(*tab),
                    DockSpecEntry::Group { root, .. } => spec_tabs(root, out),
                }
            }
        }
    }
}

fn split_fractions(node: &DockSpecNode, out: &mut Vec<(String, Direction, f32)>) {
    match node {
        DockSpecNode::Split {
            key,
            direction,
            fraction,
            first,
            second,
        } => {
            out.push((key.clone(), *direction, *fraction));
            split_fractions(first, out);
            split_fractions(second, out);
        }
        DockSpecNode::Pane(pane) => {
            for entry in &pane.entries {
                if let DockSpecEntry::Group { root, .. } = entry {
                    split_fractions(root, out);
                }
            }
        }
    }
}

fn side_of(direction: Direction, first: bool) -> Side {
    match (direction, first) {
        (Direction::Horizontal, true) => Side::Left,
        (Direction::Horizontal, false) => Side::Right,
        (Direction::Vertical, true) => Side::Above,
        (Direction::Vertical, false) => Side::Below,
    }
}
