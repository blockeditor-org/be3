use beui_view::types::Direction;
use beui_view::types::Rect;

use super::{
    Bar, DockBarPlacement, DockHome, DockOutside, DockState, Entry, Group, GroupId, Home,
    MIN_FRACTION, MIN_SIDEBAR_WIDTH, Node, Split, SplitId, Surface, SurfaceId, TabId,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DockPlain {
    pub main: Option<DockPlainNode>,
    pub windows: Vec<DockPlainWindow>,
    pub focus: Option<TabId>,
    pub homes: Vec<DockPlainHome>,
    pub bars: Vec<DockPlainBar>,
    pub outside: Vec<DockOutside>,
    pub unlocked: Vec<DockPlainPlace>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DockPlainBar {
    pub bar: TabId,
    pub owner: TabId,
    pub placement: DockBarPlacement,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DockPlainWindow {
    pub key: String,
    pub rect: Rect,
    pub root: Option<DockPlainNode>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DockPlainNode {
    Split {
        key: String,
        direction: Direction,
        fraction: f32,
        first: Box<DockPlainNode>,
        second: Box<DockPlainNode>,
    },
    Pane {
        key: String,
        active: usize,
        vertical: bool,
        icons_only: bool,
        sidebar: f32,
        keep: bool,
        area: Option<String>,
        bare: bool,
        entries: Vec<DockPlainEntry>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum DockPlainEntry {
    Tab(TabId),
    Group {
        key: String,
        pinned: bool,
        root: Box<DockPlainNode>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct DockPlainHome {
    pub tab: TabId,
    pub place: DockPlainPlace,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DockPlainPlace {
    Area(String),
    Group(String),
}

fn named(key: &str) -> Option<String> {
    (!key.is_empty()).then(|| key.to_owned())
}

impl DockState {
    pub fn plain(&self) -> DockPlain {
        let mut plain = DockPlain::default();
        for (index, surface) in self.surfaces.iter().enumerate() {
            let root = self.plain_node(&surface.root);
            match (index, surface.window) {
                (0, _) => plain.main = Some(root),
                (_, Some(rect)) => plain.windows.push(DockPlainWindow {
                    key: surface.key.clone().unwrap_or_default(),
                    rect,
                    root: Some(root),
                }),
                (_, None) => {}
            }
        }
        plain.focus = self.focused_tab();
        plain.homes = self
            .homes
            .iter()
            .filter_map(|home| {
                Some(DockPlainHome {
                    tab: home.tab,
                    place: self.plain_place(&home.place)?,
                })
            })
            .collect();
        plain.bars = self
            .bars
            .iter()
            .map(|bar| DockPlainBar {
                bar: bar.bar,
                owner: bar.owner,
                placement: bar.placement,
            })
            .collect();
        plain.outside = self.outside.clone();
        plain.unlocked = self
            .unlocked
            .iter()
            .filter_map(|place| self.plain_place(place))
            .chain(
                self.unlocked_keys
                    .iter()
                    .cloned()
                    .map(DockPlainPlace::Group),
            )
            .collect();
        plain
    }

    fn plain_place(&self, place: &DockHome) -> Option<DockPlainPlace> {
        match place {
            DockHome::Area(area) => Some(DockPlainPlace::Area(area.clone())),
            DockHome::Group(group) => Some(DockPlainPlace::Group(self.group(*group)?.key.clone()?)),
        }
    }

    fn place_of_plain(&self, place: &DockPlainPlace) -> Option<DockHome> {
        match place {
            DockPlainPlace::Area(area) => Some(DockHome::Area(area.clone())),
            DockPlainPlace::Group(key) => self.group_by_key(key).map(DockHome::Group),
        }
    }

    fn plain_node(&self, node: &Node) -> DockPlainNode {
        match node {
            Node::Leaf(leaf) => DockPlainNode::Pane {
                key: leaf.key.clone().unwrap_or_default(),
                active: leaf.active,
                vertical: leaf.vertical,
                icons_only: leaf.icons_only,
                sidebar: leaf.sidebar,
                keep: leaf.keep,
                area: leaf.area.clone(),
                bare: leaf.bare,
                entries: leaf
                    .entries
                    .iter()
                    .filter_map(|entry| match entry {
                        Entry::Tab(tab) => Some(DockPlainEntry::Tab(*tab)),
                        Entry::Group(group) => {
                            let group = self.group(*group)?;
                            Some(DockPlainEntry::Group {
                                key: group.key.clone().unwrap_or_default(),
                                pinned: group.pinned,
                                root: Box::new(self.plain_node(&group.root)),
                            })
                        }
                    })
                    .collect(),
            },
            Node::Split(split) => DockPlainNode::Split {
                key: split.key.clone().unwrap_or_default(),
                direction: split.direction,
                fraction: split.fraction,
                first: Box::new(self.plain_node(&split.first)),
                second: Box::new(self.plain_node(&split.second)),
            },
        }
    }

    pub fn from_plain(plain: &DockPlain) -> Self {
        let mut state = DockState::new(Vec::new());
        state.recent.clear();
        state.history.clear();
        if let Some(main) = &plain.main {
            let root = state.build_plain(main);
            state.surfaces[0].root = root;
        }
        for window in &plain.windows {
            let root = match &window.root {
                Some(root) => state.build_plain(root),
                None => Node::Leaf(state.new_leaf(Vec::new())),
            };
            let id = SurfaceId(state.mint());
            state.surfaces.push(Surface {
                id,
                root,
                window: Some(window.rect),
                key: named(&window.key),
            });
        }
        state.invalidate();
        for home in &plain.homes {
            let Some(place) = state.place_of_plain(&home.place) else {
                continue;
            };
            if state.contains(home.tab) {
                state.homes.push(Home {
                    tab: home.tab,
                    place,
                    within: None,
                });
            }
        }
        state.bars = plain
            .bars
            .iter()
            .map(|bar| Bar {
                bar: bar.bar,
                owner: bar.owner,
                placement: bar.placement,
            })
            .collect();
        state.outside = plain.outside.clone();
        for place in &plain.unlocked {
            match state.place_of_plain(place) {
                Some(place) => state.unlocked.push(place),
                None => {
                    if let DockPlainPlace::Group(key) = place {
                        state.unlocked_keys.push(key.clone());
                    }
                }
            }
        }
        state.focus = plain
            .focus
            .and_then(|tab| state.find(tab))
            .map(|position| position.leaf)
            .or(state.focus);
        state.settle_focus();
        state.remember_focus();
        state.seeded = true;
        state
    }

    fn build_plain(&mut self, node: &DockPlainNode) -> Node {
        match node {
            DockPlainNode::Pane {
                key,
                active,
                vertical,
                icons_only,
                sidebar,
                keep,
                area,
                bare,
                entries,
            } => {
                let entries: Vec<Entry> = entries
                    .iter()
                    .map(|entry| match entry {
                        DockPlainEntry::Tab(tab) => Entry::Tab(*tab),
                        DockPlainEntry::Group { key, pinned, root } => {
                            let root = self.build_plain(root);
                            let group = GroupId(self.mint());
                            self.groups.push(Group {
                                id: group,
                                root,
                                pinned: *pinned,
                                key: named(key),
                            });
                            Entry::Group(group)
                        }
                    })
                    .collect();
                let mut leaf = self.new_leaf(entries);
                leaf.active = (*active).min(leaf.entries.len().saturating_sub(1));
                leaf.vertical = *vertical;
                leaf.icons_only = *icons_only;
                leaf.sidebar = sidebar.max(MIN_SIDEBAR_WIDTH);
                leaf.key = named(key);
                leaf.keep = *keep && leaf.key.is_some();
                leaf.area.clone_from(area);
                leaf.bare = *bare;
                Node::Leaf(leaf)
            }
            DockPlainNode::Split {
                key,
                direction,
                fraction,
                first,
                second,
            } => {
                let first = self.build_plain(first);
                let second = self.build_plain(second);
                Node::Split(Split {
                    id: SplitId(self.mint()),
                    direction: *direction,
                    fraction: fraction.clamp(MIN_FRACTION, 1.0 - MIN_FRACTION),
                    first: Box::new(first),
                    second: Box::new(second),
                    key: named(key),
                    adjusted: false,
                })
            }
        }
    }
}
