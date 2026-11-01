use super::{
    Bar, DockHome, DockSpec, DockSpecBar, DockSpecEntry, DockSpecHome, DockSpecNode, DockSpecPane,
    DockSpecPlace, DockSpecWindow, DockState, Entry, Group, GroupId, Home, MIN_FRACTION,
    MIN_SIDEBAR_WIDTH, Node, Split, SplitId, Surface, SurfaceId,
};

fn named(key: &str) -> Option<String> {
    (!key.is_empty()).then(|| key.to_owned())
}

impl DockState {
    pub fn spec(&self) -> DockSpec {
        let mut spec = DockSpec::default();
        for (index, surface) in self.surfaces.iter().enumerate() {
            let root = self.spec_node(&surface.root);
            match (index, surface.window) {
                (0, _) => spec.main = Some(root),
                (_, Some(rect)) => spec.windows.push(DockSpecWindow {
                    key: surface.key.clone().unwrap_or_default(),
                    rect,
                    root: Some(root),
                }),
                (_, None) => {}
            }
        }
        spec.focus = self.focused_tab();
        spec.see_through.clone_from(&self.see_through);
        spec.homes = self
            .homes
            .iter()
            .filter_map(|home| {
                Some(DockSpecHome {
                    tab: home.tab,
                    place: self.spec_place(&home.place)?,
                })
            })
            .collect();
        spec.bars = self
            .bars
            .iter()
            .map(|bar| DockSpecBar {
                bar: bar.bar,
                owner: bar.owner,
                placement: bar.placement,
                hidden: bar.hidden,
            })
            .collect();
        spec.outside = self.outside.clone();
        spec.unlocked = self
            .unlocked
            .iter()
            .filter_map(|place| self.spec_place(place))
            .chain(self.unlocked_keys.iter().cloned().map(DockSpecPlace::Group))
            .collect();
        spec
    }

    fn spec_place(&self, place: &DockHome) -> Option<DockSpecPlace> {
        match place {
            DockHome::Area(area) => Some(DockSpecPlace::Area(area.clone())),
            DockHome::Group(group) => Some(DockSpecPlace::Group(self.group(*group)?.key.clone()?)),
        }
    }

    fn place_of_spec(&self, place: &DockSpecPlace) -> Option<DockHome> {
        match place {
            DockSpecPlace::Area(area) => Some(DockHome::Area(area.clone())),
            DockSpecPlace::Group(key) => self.group_by_key(key).map(DockHome::Group),
        }
    }

    fn spec_node(&self, node: &Node) -> DockSpecNode {
        match node {
            Node::Leaf(leaf) => DockSpecNode::Pane(DockSpecPane {
                key: leaf.key.clone().unwrap_or_default(),
                entries: leaf
                    .entries
                    .iter()
                    .filter_map(|entry| match entry {
                        Entry::Tab(tab) => Some(DockSpecEntry::Tab(*tab)),
                        Entry::Group(group) => {
                            let group = self.group(*group)?;
                            Some(DockSpecEntry::Group {
                                key: group.key.clone().unwrap_or_default(),
                                pinned: group.pinned,
                                root: Box::new(self.spec_node(&group.root)),
                            })
                        }
                    })
                    .collect(),
                active: leaf.active,
                vertical: leaf.vertical,
                icons_only: leaf.icons_only,
                sidebar: leaf.sidebar,
                keep: leaf.keep,
                area: leaf.area.clone(),
                bare: leaf.bare,
            }),
            Node::Split(split) => DockSpecNode::Split {
                key: split.key.clone().unwrap_or_default(),
                direction: split.direction,
                fraction: split.fraction,
                first: Box::new(self.spec_node(&split.first)),
                second: Box::new(self.spec_node(&split.second)),
            },
        }
    }

    pub fn from_spec(spec: &DockSpec) -> Self {
        let mut state = DockState::new(Vec::new());
        state.recent.clear();
        state.history.clear();
        if let Some(main) = &spec.main {
            let root = state.restore_node(main);
            state.surfaces[0].root = root;
        }
        for window in &spec.windows {
            let root = match &window.root {
                Some(root) => state.restore_node(root),
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
        state.see_through.clone_from(&spec.see_through);
        for home in &spec.homes {
            let Some(place) = state.place_of_spec(&home.place) else {
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
        state.bars = spec
            .bars
            .iter()
            .map(|bar| Bar {
                bar: bar.bar,
                owner: bar.owner,
                placement: bar.placement,
                hidden: bar.hidden,
            })
            .collect();
        state.outside = spec.outside.clone();
        for place in &spec.unlocked {
            match state.place_of_spec(place) {
                Some(place) => state.unlocked.push(place),
                None => {
                    if let DockSpecPlace::Group(key) = place {
                        state.unlocked_keys.push(key.clone());
                    }
                }
            }
        }
        state.focus = spec
            .focus
            .and_then(|tab| state.find(tab))
            .map(|position| position.leaf)
            .or(state.focus);
        state.settle_focus();
        state.remember_focus();
        state.seeded = true;
        state
    }

    fn restore_node(&mut self, node: &DockSpecNode) -> Node {
        match node {
            DockSpecNode::Pane(pane) => {
                let entries: Vec<Entry> = pane
                    .entries
                    .iter()
                    .map(|entry| match entry {
                        DockSpecEntry::Tab(tab) => Entry::Tab(*tab),
                        DockSpecEntry::Group { key, pinned, root } => {
                            let root = self.restore_node(root);
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
                leaf.active = pane.active.min(leaf.entries.len().saturating_sub(1));
                leaf.vertical = pane.vertical;
                leaf.icons_only = pane.icons_only;
                leaf.sidebar = pane.sidebar.max(MIN_SIDEBAR_WIDTH);
                leaf.key = named(&pane.key);
                leaf.keep = pane.keep && leaf.key.is_some();
                leaf.area.clone_from(&pane.area);
                leaf.bare = pane.bare;
                Node::Leaf(leaf)
            }
            DockSpecNode::Split {
                key,
                direction,
                fraction,
                first,
                second,
            } => {
                let first = self.restore_node(first);
                let second = self.restore_node(second);
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
