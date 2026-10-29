use super::{DockEntry, DockFloating, DockNode, DockNodes};
use crate::{
    ChildId, DecodeError, DockBar, DockCover, DockDeclaration, DockGroup, DockHome, DockHosted, DockLayout,
    DockOutside, DockPane, DockPlace, DockPlacedBar, DockRequest, DockTab, DockTabPath, DockTree,
};

pub fn dock_prefix(host_key: &str, block: [u8; 16]) -> String {
    format!("{host_key}/{}", uuid::Uuid::from_bytes(block).simple())
}

pub fn dock_prefix_block(prefix: &str) -> Option<[u8; 16]> {
    let (_, block) = prefix.rsplit_once('/')?;
    uuid::Uuid::try_parse(block)
        .ok()
        .map(uuid::Uuid::into_bytes)
}

fn prefixed(prefix: &str, key: &str) -> String {
    match key.is_empty() {
        true => String::new(),
        false => format!("{prefix}/{key}"),
    }
}

fn unprefixed(prefix: &str, key: &str) -> String {
    key.strip_prefix(prefix)
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or_default()
        .to_owned()
}

type Replace<'a> = &'a dyn Fn(&DockEntry) -> Option<Option<DockEntry>>;

struct Mapping<'a> {
    key: &'a dyn Fn(&str) -> String,
    tab: &'a dyn Fn(&DockTabPath) -> Option<DockTabPath>,
    entry: Replace<'a>,
}

impl Mapping<'_> {
    fn node(&self, node: &DockNode) -> DockNode {
        match node {
            DockNode::Split {
                split,
                first,
                second,
            } => DockNode::Split {
                split: crate::DockSplit {
                    key: (self.key)(&split.key),
                    ..split.clone()
                },
                first: Box::new(self.node(first)),
                second: Box::new(self.node(second)),
            },
            DockNode::Pane { pane, entries } => {
                let mut kept = Vec::new();
                let mut active = pane.active;
                for (index, entry) in entries.iter().enumerate() {
                    match self.entry(entry) {
                        Some(entry) => kept.push(entry),
                        None => {
                            if active.is_some_and(|active| active as usize > index) {
                                active = active.map(|active| active - 1);
                            }
                        }
                    }
                }
                let active = active.map(|active| active.min(kept.len().saturating_sub(1) as u32));
                DockNode::Pane {
                    pane: DockPane {
                        key: (self.key)(&pane.key),
                        area: pane
                            .area
                            .as_deref()
                            .map(|area| (self.key)(area))
                            .filter(|area| !area.is_empty()),
                        empty: pane.empty.as_ref().and_then(|empty| (self.tab)(empty)),
                        active,
                        ..pane.clone()
                    },
                    entries: kept,
                }
            }
        }
    }

    fn entry(&self, entry: &DockEntry) -> Option<DockEntry> {
        if let Some(replaced) = (self.entry)(entry) {
            return replaced;
        }
        match entry {
            DockEntry::Tab(tab) => (self.tab)(tab).map(DockEntry::Tab),
            DockEntry::Group { group, root } => Some(DockEntry::Group {
                group: DockGroup {
                    key: (self.key)(&group.key),
                    ..group.clone()
                },
                root: Box::new(self.node(root)),
            }),
        }
    }

    fn window(&self, window: &DockFloating) -> DockFloating {
        DockFloating {
            window: crate::DockWindow {
                key: (self.key)(&window.window.key),
                ..window.window.clone()
            },
            root: window.root.as_ref().map(|root| self.node(root)),
        }
    }

    fn nodes(&self, nodes: &DockNodes) -> DockNodes {
        DockNodes {
            main: nodes.main.as_ref().map(|main| self.node(main)),
            windows: nodes
                .windows
                .iter()
                .map(|window| self.window(window))
                .collect(),
        }
    }

    fn home(&self, home: &DockHome) -> Option<DockHome> {
        let place = match &home.place {
            DockPlace::Area(key) => DockPlace::Area((self.key)(key)),
            DockPlace::Group(key) => DockPlace::Group((self.key)(key)),
        };
        Some(DockHome {
            tab: (self.tab)(&home.tab)?,
            place,
            pinned: home.pinned,
        })
    }

    fn layout(&self, nodes: &DockNodes, layout: &DockLayout) -> DockLayout {
        DockLayout {
            tree: DockTree::flatten(
                &self.nodes(nodes),
                layout
                    .tree
                    .focus
                    .as_ref()
                    .and_then(|focus| (self.tab)(focus)),
            ),
            homes: layout
                .homes
                .iter()
                .filter_map(|home| self.home(home))
                .collect(),
            bars: layout
                .bars
                .iter()
                .filter_map(|bar| {
                    Some(DockPlacedBar {
                        bar: (self.tab)(&bar.bar)?,
                        owner: (self.tab)(&bar.owner)?,
                        placement: bar.placement.clone(),
                    })
                })
                .collect(),
            outside: layout
                .outside
                .iter()
                .filter_map(|outside| {
                    Some(DockOutside {
                        tab: (self.tab)(&outside.tab)?,
                        place: outside.place.clone(),
                    })
                })
                .collect(),
        }
    }
}

fn keep(_: &DockEntry) -> Option<Option<DockEntry>> {
    None
}

fn empty_pane(key: String) -> DockNode {
    DockNode::Pane {
        pane: DockPane {
            key,
            active: None,
            vertical: false,
            icons_only: false,
            sidebar: 180.0,
            keep: false,
            area: None,
            empty: None,
            bare: false,
        },
        entries: Vec::new(),
    }
}

impl DockNode {
    fn spliced_entry(&self, child: ChildId) -> Option<(&DockGroup, &DockNode)> {
        match self {
            Self::Split { first, second, .. } => first
                .spliced_entry(child)
                .or_else(|| second.spliced_entry(child)),
            Self::Pane { entries, .. } => entries.iter().find_map(|entry| match entry {
                DockEntry::Tab(_) => None,
                DockEntry::Group { group, root } if group.child == Some(child) => {
                    Some((group, &**root))
                }
                DockEntry::Group { root, .. } => root.spliced_entry(child),
            }),
        }
    }
}

impl DockNodes {
    fn spliced_entry(&self, child: ChildId) -> Option<(&DockGroup, &DockNode)> {
        self.roots().find_map(|root| root.spliced_entry(child))
    }

    fn spliced_group(&self, child: ChildId) -> Option<&DockNode> {
        self.spliced_entry(child).map(|(_, root)| root)
    }
}

impl DockDeclaration {
    pub fn owner_block(&self, via: &[ChildId]) -> Option<[u8; 16]> {
        let nodes = self.tree.nest().ok()?;
        let (first, rest) = via.split_first()?;
        let mut group = nodes.spliced_entry(*first)?;
        for child in rest {
            group = group.1.spliced_entry(*child)?;
        }
        dock_prefix_block(&group.0.key)
    }

    pub fn splice(
        &mut self,
        host: u64,
        child: ChildId,
        prefix: &str,
        inner: &DockDeclaration,
    ) -> Result<bool, DecodeError> {
        let nodes = self.tree.nest()?;
        let inner_nodes = inner.tree.nest()?;
        let host = DockTabPath::own(host);
        if !nodes.tabs().contains(&&host) {
            return Ok(false);
        }
        let inward = Mapping {
            key: &|key| prefixed(prefix, key),
            tab: &|tab| Some(tab.clone().through(child)),
            entry: &keep,
        };
        let root = inner_nodes
            .main
            .as_ref()
            .map(|main| inward.node(main))
            .unwrap_or_else(|| empty_pane(format!("{prefix}/")));
        let group = DockEntry::Group {
            group: DockGroup {
                key: prefix.to_owned(),
                pinned: true,
                child: Some(child),
            },
            root: Box::new(root),
        };
        let replace = |entry: &DockEntry| {
            matches!(entry, DockEntry::Tab(tab) if *tab == host).then(|| Some(group.clone()))
        };
        let outward = Mapping {
            key: &str::to_owned,
            tab: &|tab| Some(tab.clone()),
            entry: &replace,
        };
        let mut spliced = outward.nodes(&nodes);
        spliced.windows.extend(
            inner_nodes
                .windows
                .iter()
                .map(|window| inward.window(window)),
        );
        let focus = match self.tree.focus.as_ref() == Some(&host) {
            true => inner.tree.focus.clone().map(|focus| focus.through(child)),
            false => self.tree.focus.clone(),
        };
        self.tree = DockTree::flatten(&spliced, focus);
        self.tabs.retain(|tab| tab.tab != host);
        self.tabs.extend(inner.tabs.iter().map(|tab| DockTab {
            tab: tab.tab.clone().through(child),
            key: prefixed(prefix, &tab.key),
            ..tab.clone()
        }));
        self.dialogs
            .extend(inner.dialogs.iter().map(|dialog| crate::DockDialog {
                tab: dialog.tab.clone().through(child),
                parent: dialog.parent.clone().through(child),
                ..dialog.clone()
            }));
        self.bars.retain(|bar| bar.owner != host);
        self.bars.extend(inner.bars.iter().map(|bar| DockBar {
            tab: bar.tab.clone().through(child),
            key: prefixed(prefix, &bar.key),
            owner: bar.owner.clone().through(child),
            ..bar.clone()
        }));
        Ok(true)
    }
}

impl DockLayout {
    pub fn through(&self, child: ChildId, prefix: &str) -> Result<DockLayout, DecodeError> {
        let nodes = self.tree.nest()?;
        let inward = Mapping {
            key: &|key| prefixed(prefix, key),
            tab: &|tab| Some(tab.clone().through(child)),
            entry: &keep,
        };
        Ok(inward.layout(&nodes, self))
    }
}

impl DockRequest {
    pub fn through(&self, child: ChildId, prefix: &str) -> Result<DockRequest, DecodeError> {
        Ok(match self {
            Self::Show(tab) => Self::Show(tab.clone().through(child)),
            Self::Restore(layout) => Self::Restore(layout.through(child, prefix)?),
            Self::Fullscreen(fullscreen) => {
                Self::Fullscreen(fullscreen.clone().map(|fullscreen| crate::DockFullscreen {
                    tab: fullscreen.tab.through(child),
                    area: fullscreen.area,
                }))
            }
            Self::PlaceBar(place) => Self::PlaceBar(crate::DockBarPlace {
                bar: place.bar.clone().through(child),
                placement: place.placement.clone(),
            }),
            Self::MoveBar(moved) => Self::MoveBar(crate::DockBarMove {
                bar: moved.bar.clone().through(child),
                ..moved.clone()
            }),
        })
    }
}

impl DockHosted {
    pub fn own(&self, hosts: &[(ChildId, u64)]) -> Result<DockHosted, DecodeError> {
        let nodes = self.layout.tree.nest()?;
        let host_of = |child: ChildId| {
            hosts
                .iter()
                .find(|(hosted, _)| *hosted == child)
                .map(|(_, tab)| *tab)
        };
        let path = |tab: &DockTabPath| match tab.via.first() {
            None => Some(tab.clone()),
            Some(child) => host_of(*child).map(DockTabPath::own),
        };
        let replace = |entry: &DockEntry| match entry {
            DockEntry::Group {
                group: DockGroup {
                    child: Some(child), ..
                },
                ..
            } => Some(host_of(*child).map(|tab| DockEntry::Tab(DockTabPath::own(tab)))),
            _ => None,
        };
        let mapping = Mapping {
            key: &str::to_owned,
            tab: &|tab| tab.is_own().then(|| tab.clone()),
            entry: &replace,
        };
        let mut tabs: Vec<DockTabPath> = self
            .tabs
            .iter()
            .filter(|tab| tab.is_own())
            .cloned()
            .collect();
        tabs.extend(
            hosts
                .iter()
                .map(|(_, tab)| DockTabPath::own(*tab))
                .filter(|tab| !self.tabs.contains(tab)),
        );
        Ok(DockHosted {
            layout: mapping.layout(&nodes, &self.layout),
            tabs,
            focused: self.focused.as_ref().and_then(path),
            shown: self.shown.as_ref().and_then(path),
            visible: self
                .visible
                .iter()
                .filter(|tab| tab.is_own())
                .cloned()
                .collect(),
            fullscreen: self.fullscreen.as_ref().filter(|tab| tab.is_own()).cloned(),
            stacked: self.stacked,
            covers: self
                .covers
                .iter()
                .filter(|cover| cover.tab.is_own())
                .cloned()
                .collect(),
        })
    }

    pub fn child(&self, child: ChildId, prefix: &str) -> Result<Option<DockHosted>, DecodeError> {
        let nodes = self.layout.tree.nest()?;
        let within = |tab: &DockTabPath| tab.within(child);
        let tabs: Vec<DockTabPath> = self.tabs.iter().filter_map(within).collect();
        let group = nodes.spliced_group(child);
        if group.is_none() && tabs.is_empty() {
            return Ok(None);
        }
        let inward = Mapping {
            key: &|key| unprefixed(prefix, key),
            tab: &within,
            entry: &keep,
        };
        let windows = nodes
            .windows
            .iter()
            .filter(|window| window.window.key.starts_with(&format!("{prefix}/")))
            .cloned()
            .collect();
        let held = DockNodes {
            main: group.cloned(),
            windows,
        };
        Ok(Some(DockHosted {
            layout: inward.layout(&held, &self.layout),
            tabs,
            focused: self.focused.as_ref().and_then(within),
            shown: self.shown.as_ref().and_then(within),
            visible: self.visible.iter().filter_map(within).collect(),
            fullscreen: self.fullscreen.as_ref().and_then(within),
            stacked: self.stacked,
            covers: self
                .covers
                .iter()
                .filter_map(|cover| {
                    Some(DockCover {
                        tab: within(&cover.tab)?,
                        rect: cover.rect,
                    })
                })
                .collect(),
        }))
    }
}
