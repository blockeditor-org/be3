use std::collections::HashSet;

use crate::{
    ChildId, DecodeError, DockDeclaration, DockGroup, DockHome, DockHosted, DockItem,
    DockItemKind, DockLayout, DockMessage, DockPane, DockPlace, DockRequest, DockSplit, DockTab,
    DockTabPath, DockTree, DockUpdate, DockWindow, Rect, collection, nesting, string,
};

pub const MAX_DOCK_DEPTH: usize = 32;
pub const MAX_DOCK_NESTING: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub enum DockNode {
    Split {
        split: DockSplit,
        first: Box<DockNode>,
        second: Box<DockNode>,
    },
    Pane {
        pane: DockPane,
        entries: Vec<DockEntry>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum DockEntry {
    Tab(DockTabPath),
    Group { group: DockGroup, root: Box<DockNode> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct DockFloating {
    pub window: DockWindow,
    pub root: Option<DockNode>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DockNodes {
    pub main: Option<DockNode>,
    pub windows: Vec<DockFloating>,
}

enum Built {
    Node(DockNode),
    Entry(DockEntry),
    Window(DockFloating),
}

impl DockTabPath {
    pub fn own(tab: u64) -> Self {
        Self {
            via: Vec::new(),
            tab,
        }
    }

    pub fn is_own(&self) -> bool {
        self.via.is_empty()
    }

    pub fn through(mut self, child: ChildId) -> Self {
        self.via.insert(0, child);
        self
    }

    pub fn within(&self, child: ChildId) -> Option<Self> {
        let (first, rest) = self.via.split_first()?;
        (*first == child).then(|| Self {
            via: rest.to_vec(),
            tab: self.tab,
        })
    }

    fn validate(&self) -> Result<(), DecodeError> {
        if self.via.len() > MAX_DOCK_NESTING {
            Err(DecodeError::LimitExceeded("dock nesting"))
        } else {
            Ok(())
        }
    }
}

impl DockNode {
    fn tabs<'a>(&'a self, out: &mut Vec<&'a DockTabPath>) {
        match self {
            Self::Split { first, second, .. } => {
                first.tabs(out);
                second.tabs(out);
            }
            Self::Pane { entries, .. } => {
                for entry in entries {
                    match entry {
                        DockEntry::Tab(tab) => out.push(tab),
                        DockEntry::Group { root, .. } => root.tabs(out),
                    }
                }
            }
        }
    }

    fn keys<'a>(&'a self, out: &mut Vec<(&'static str, &'a str)>) {
        match self {
            Self::Split {
                split,
                first,
                second,
            } => {
                out.push(("split", &split.key));
                first.keys(out);
                second.keys(out);
            }
            Self::Pane { pane, entries } => {
                out.push(("pane", &pane.key));
                for entry in entries {
                    if let DockEntry::Group { group, root } = entry {
                        out.push(("group", &group.key));
                        root.keys(out);
                    }
                }
            }
        }
    }

    fn panes<'a>(&'a self, out: &mut Vec<&'a DockPane>) {
        match self {
            Self::Split { first, second, .. } => {
                first.panes(out);
                second.panes(out);
            }
            Self::Pane { pane, entries } => {
                out.push(pane);
                for entry in entries {
                    if let DockEntry::Group { root, .. } = entry {
                        root.panes(out);
                    }
                }
            }
        }
    }

    fn flatten(&self, depth: u32, out: &mut Vec<DockItem>) {
        match self {
            Self::Split {
                split,
                first,
                second,
            } => {
                out.push(item(depth, DockItemKind::Split(split.clone())));
                first.flatten(depth + 1, out);
                second.flatten(depth + 1, out);
            }
            Self::Pane { pane, entries } => {
                out.push(item(depth, DockItemKind::Pane(pane.clone())));
                for entry in entries {
                    match entry {
                        DockEntry::Tab(tab) => {
                            out.push(item(depth + 1, DockItemKind::Tab(tab.clone())));
                        }
                        DockEntry::Group { group, root } => {
                            out.push(item(depth + 1, DockItemKind::Group(group.clone())));
                            root.flatten(depth + 2, out);
                        }
                    }
                }
            }
        }
    }
}

fn item(depth: u32, kind: DockItemKind) -> DockItem {
    DockItem { depth, kind }
}

impl DockNodes {
    pub fn tabs(&self) -> Vec<&DockTabPath> {
        let mut tabs = Vec::new();
        for root in self.roots() {
            root.tabs(&mut tabs);
        }
        tabs
    }

    fn roots(&self) -> impl Iterator<Item = &DockNode> {
        self.main
            .iter()
            .chain(self.windows.iter().filter_map(|window| window.root.as_ref()))
    }

    fn keys(&self) -> Vec<(&'static str, &str)> {
        let mut keys = Vec::new();
        for window in &self.windows {
            keys.push(("window", window.window.key.as_str()));
        }
        for root in self.roots() {
            root.keys(&mut keys);
        }
        keys
    }

    fn panes(&self) -> Vec<&DockPane> {
        let mut panes = Vec::new();
        for root in self.roots() {
            root.panes(&mut panes);
        }
        panes
    }
}

impl DockTree {
    pub fn nest(&self) -> Result<DockNodes, DecodeError> {
        if let Some(focus) = &self.focus {
            focus.validate()?;
        }
        let built = nesting::nest_within(
            self.items.clone(),
            (MAX_DOCK_DEPTH, "dock depth"),
            |item| item.depth,
            |item, children| built(item.kind, children),
        )?;
        let mut nodes = DockNodes::default();
        for top in built {
            match top {
                Built::Node(node) if nodes.main.is_none() => nodes.main = Some(node),
                Built::Window(window) => nodes.windows.push(window),
                Built::Node(_) | Built::Entry(_) => return Err(DecodeError::MalformedPayload),
            }
        }
        let tabs = nodes.tabs();
        if tabs.iter().collect::<HashSet<_>>().len() != tabs.len() {
            return Err(DecodeError::MalformedPayload);
        }
        Ok(nodes)
    }

    pub fn flatten(nodes: &DockNodes, focus: Option<DockTabPath>) -> Self {
        let mut items = Vec::new();
        if let Some(main) = &nodes.main {
            main.flatten(0, &mut items);
        }
        for window in &nodes.windows {
            items.push(item(0, DockItemKind::Window(window.window.clone())));
            if let Some(root) = &window.root {
                root.flatten(1, &mut items);
            }
        }
        Self { items, focus }
    }
}

fn built(kind: DockItemKind, children: Vec<Built>) -> Result<Built, DecodeError> {
    match kind {
        DockItemKind::Split(split) => {
            string(&split.key)?;
            if !(split.fraction.is_finite() && (0.0..=1.0).contains(&split.fraction)) {
                return Err(DecodeError::MalformedPayload);
            }
            let [Built::Node(first), Built::Node(second)] = <[Built; 2]>::try_from(children)
                .map_err(|_| DecodeError::MalformedPayload)?
            else {
                return Err(DecodeError::MalformedPayload);
            };
            Ok(Built::Node(DockNode::Split {
                split,
                first: Box::new(first),
                second: Box::new(second),
            }))
        }
        DockItemKind::Pane(pane) => {
            string(&pane.key)?;
            if let Some(area) = &pane.area {
                string(area)?;
            }
            if !(pane.sidebar.is_finite() && pane.sidebar >= 0.0) {
                return Err(DecodeError::MalformedPayload);
            }
            let entries = children
                .into_iter()
                .map(|child| match child {
                    Built::Entry(entry) => Ok(entry),
                    Built::Node(_) | Built::Window(_) => Err(DecodeError::MalformedPayload),
                })
                .collect::<Result<Vec<_>, _>>()?;
            if pane
                .active
                .is_some_and(|active| active as usize >= entries.len().max(1))
            {
                return Err(DecodeError::MalformedPayload);
            }
            Ok(Built::Node(DockNode::Pane { pane, entries }))
        }
        DockItemKind::Tab(tab) if children.is_empty() => {
            tab.validate()?;
            Ok(Built::Entry(DockEntry::Tab(tab)))
        }
        DockItemKind::Group(group) => {
            string(&group.key)?;
            let [Built::Node(root)] =
                <[Built; 1]>::try_from(children).map_err(|_| DecodeError::MalformedPayload)?
            else {
                return Err(DecodeError::MalformedPayload);
            };
            Ok(Built::Entry(DockEntry::Group {
                group,
                root: Box::new(root),
            }))
        }
        DockItemKind::Window(window) => {
            string(&window.key)?;
            finite(window.rect)?;
            let mut children = children.into_iter();
            let root = match (children.next(), children.next()) {
                (None, _) => None,
                (Some(Built::Node(root)), None) => Some(root),
                _ => return Err(DecodeError::MalformedPayload),
            };
            Ok(Built::Window(DockFloating { window, root }))
        }
        DockItemKind::Tab(_) => Err(DecodeError::MalformedPayload),
    }
}

fn finite(rect: Rect) -> Result<(), DecodeError> {
    let values = [rect.min.x, rect.min.y, rect.max.x, rect.max.y];
    if values.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(DecodeError::MalformedPayload)
    }
}

impl DockDeclaration {
    pub fn tab(&self, tab: &DockTabPath) -> Option<&DockTab> {
        self.tabs.iter().find(|declared| declared.tab == *tab)
    }

    pub fn nest(&self) -> Result<DockNodes, DecodeError> {
        let nodes = self.tree.nest()?;
        let keys = nodes.keys();
        if keys.iter().any(|(_, key)| key.is_empty())
            || keys.iter().collect::<HashSet<_>>().len() != keys.len()
            || nodes.panes().iter().any(|pane| pane.area.is_some())
        {
            return Err(DecodeError::MalformedPayload);
        }
        collection(self.tabs.len())?;
        for tab in &self.tabs {
            tab.tab.validate()?;
            string(&tab.key)?;
            string(&tab.title)?;
            string(&tab.glyph)?;
        }
        let declared: HashSet<&DockTabPath> = self.tabs.iter().map(|tab| &tab.tab).collect();
        let keys: HashSet<&str> = self.tabs.iter().map(|tab| tab.key.as_str()).collect();
        let held: HashSet<&DockTabPath> = nodes.tabs().into_iter().collect();
        if declared.len() != self.tabs.len()
            || keys.len() != self.tabs.len()
            || keys.contains("")
            || declared != held
        {
            return Err(DecodeError::MalformedPayload);
        }
        Ok(nodes)
    }

    pub fn validate(&self) -> Result<(), DecodeError> {
        self.nest().map(|_| ())
    }
}

impl DockLayout {
    pub fn validate(&self) -> Result<(), DecodeError> {
        self.tree.nest()?;
        collection(self.homes.len())?;
        self.homes.iter().try_for_each(DockHome::validate)
    }
}

impl DockHome {
    fn validate(&self) -> Result<(), DecodeError> {
        self.tab.validate()?;
        match &self.place {
            DockPlace::Area(key) | DockPlace::Group(key) => string(key),
        }
    }
}

impl DockHosted {
    pub fn validate(&self) -> Result<(), DecodeError> {
        self.layout.validate()?;
        for tabs in [&self.tabs, &self.visible] {
            collection(tabs.len())?;
            tabs.iter().try_for_each(DockTabPath::validate)?;
        }
        [&self.focused, &self.shown, &self.fullscreen]
            .into_iter()
            .flatten()
            .try_for_each(DockTabPath::validate)
    }
}

impl DockRequest {
    pub fn validate(&self) -> Result<(), DecodeError> {
        match self {
            Self::Show(tab) => tab.validate(),
            Self::Restore(layout) => layout.validate(),
            Self::Fullscreen(None) => Ok(()),
            Self::Fullscreen(Some(fullscreen)) => {
                fullscreen.tab.validate()?;
                fullscreen.area.map_or(Ok(()), finite)
            }
        }
    }
}

impl DockMessage {
    pub fn validate(&self) -> Result<(), DecodeError> {
        match self {
            Self::Declare(declare) => declare
                .declaration
                .as_ref()
                .map_or(Ok(()), DockDeclaration::validate),
            Self::Request(request) => request.request.validate(),
            Self::Host(host) => host.hosted.as_ref().map_or(Ok(()), DockHosted::validate),
            Self::Close(close) => close.tab.validate(),
        }
    }
}

impl DockUpdate {
    pub fn validate(&self) -> Result<(), DecodeError> {
        match self {
            Self::Hosted(hosted) => hosted.hosted.as_ref().map_or(Ok(()), DockHosted::validate),
            Self::Close(close) => close.tab.validate(),
            Self::Child(child) => child
                .declaration
                .as_ref()
                .map_or(Ok(()), DockDeclaration::validate),
            Self::ChildRequest(request) => request.request.validate(),
        }
    }
}

impl DockUpdate {
    pub fn queue(updates: &mut Vec<Self>, update: Self) {
        match &update {
            Self::Hosted(hosted) => updates.retain(
                |waiting| !matches!(waiting, Self::Hosted(held) if held.region == hosted.region),
            ),
            Self::Child(child) => updates.retain(
                |waiting| !matches!(waiting, Self::Child(held) if held.child == child.child),
            ),
            Self::Close(_) | Self::ChildRequest(_) => {}
        }
        updates.push(update);
    }
}
