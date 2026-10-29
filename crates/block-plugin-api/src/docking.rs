use std::collections::HashSet;

use crate::{
    ChildId, DecodeError, DockBar, DockBarPlacement, DockDeclaration, DockGroup, DockHome,
    DockHosted, DockItem, DockItemKind, DockLayout, DockMessage, DockOutside, DockOutsidePlace,
    DockPane, DockPlace, DockPlacedBar, DockRequest, DockSplit, DockTab, DockTabPath, DockTree,
    DockUpdate, DockWindow, Rect, collection, nesting, string,
};

mod splice;

pub use splice::{dock_prefix, dock_prefix_block};

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
    Group {
        group: DockGroup,
        root: Box<DockNode>,
    },
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

    fn groups<'a>(&'a self, out: &mut Vec<&'a DockGroup>) {
        match self {
            Self::Split { first, second, .. } => {
                first.groups(out);
                second.groups(out);
            }
            Self::Pane { entries, .. } => {
                for entry in entries {
                    if let DockEntry::Group { group, root } = entry {
                        out.push(group);
                        root.groups(out);
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
        self.main.iter().chain(
            self.windows
                .iter()
                .filter_map(|window| window.root.as_ref()),
        )
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

    pub fn panes(&self) -> Vec<&DockPane> {
        let mut panes = Vec::new();
        for root in self.roots() {
            root.panes(&mut panes);
        }
        panes
    }

    fn spliced(&self) -> bool {
        let mut keys = Vec::new();
        for root in self.roots() {
            root.groups(&mut keys);
        }
        keys.iter().any(|group| group.child.is_some())
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
            let [Built::Node(first), Built::Node(second)] =
                <[Built; 2]>::try_from(children).map_err(|_| DecodeError::MalformedPayload)?
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
            if let Some(empty) = &pane.empty {
                empty.validate()?;
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
            || nodes
                .panes()
                .iter()
                .any(|pane| pane.area.as_ref().is_some_and(|area| *area != pane.key))
        {
            return Err(DecodeError::MalformedPayload);
        }
        collection(self.tabs.len())?;
        for tab in &self.tabs {
            tab.tab.validate()?;
            string(&tab.key)?;
            string(&tab.title)?;
            string(&tab.glyph)?;
            collection(tab.menu.len())?;
            crate::MenuEntry::nest(tab.menu.clone())?;
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
        collection(self.dialogs.len())?;
        let mut bodies: HashSet<&DockTabPath> = HashSet::new();
        for dialog in &self.dialogs {
            dialog.tab.validate()?;
            dialog.parent.validate()?;
            string(&dialog.title)?;
            string(&dialog.glyph)?;
            let size = [dialog.size.width, dialog.size.height];
            if !size.iter().all(|side| side.is_finite() && *side >= 0.0) {
                return Err(DecodeError::MalformedPayload);
            }
            if held.contains(&dialog.tab) || !bodies.insert(&dialog.tab) {
                return Err(DecodeError::MalformedPayload);
            }
        }
        for empty in nodes.panes().iter().filter_map(|pane| pane.empty.as_ref()) {
            if held.contains(empty) || !bodies.insert(empty) {
                return Err(DecodeError::MalformedPayload);
            }
        }
        collection(self.bars.len())?;
        let mut bar_keys: HashSet<&str> = HashSet::new();
        for bar in &self.bars {
            bar.validate()?;
            string(&bar.key)?;
            string(&bar.kind)?;
            let size = [bar.size.width, bar.size.height];
            if !size.iter().all(|side| side.is_finite() && *side >= 0.0)
                || bar.key.is_empty()
                || keys.contains(bar.key.as_str())
                || !bar_keys.insert(&bar.key)
                || !held.contains(&bar.owner)
                || held.contains(&bar.tab)
                || !bodies.insert(&bar.tab)
            {
                return Err(DecodeError::MalformedPayload);
            }
        }
        Ok(nodes)
    }

    pub fn bar(&self, tab: &DockTabPath) -> Option<&DockBar> {
        self.bars.iter().find(|bar| bar.tab == *tab)
    }

    pub fn dialog(&self, tab: &DockTabPath) -> Option<&crate::DockDialog> {
        self.dialogs.iter().find(|dialog| dialog.tab == *tab)
    }

    pub fn shows_body(&self, tab: &DockTabPath) -> bool {
        self.dialog(tab).is_some()
            || self.bar(tab).is_some()
            || self.nest().is_ok_and(|nodes| {
                nodes
                    .panes()
                    .iter()
                    .any(|pane| pane.empty.as_ref() == Some(tab))
            })
    }

    pub fn validate(&self) -> Result<(), DecodeError> {
        self.nest().map(|_| ())
    }

    pub fn validate_own(&self) -> Result<(), DecodeError> {
        let nodes = self.nest()?;
        let bodies_own = self
            .dialogs
            .iter()
            .all(|dialog| dialog.tab.is_own() && dialog.parent.is_own())
            && nodes
                .panes()
                .iter()
                .filter_map(|pane| pane.empty.as_ref())
                .all(DockTabPath::is_own)
            && self
                .bars
                .iter()
                .all(|bar| bar.tab.is_own() && bar.owner.is_own());
        if nodes.tabs().iter().all(|tab| tab.is_own()) && !nodes.spliced() && bodies_own {
            Ok(())
        } else {
            Err(DecodeError::MalformedPayload)
        }
    }
}

impl DockLayout {
    pub fn validate(&self) -> Result<(), DecodeError> {
        self.tree.nest()?;
        collection(self.homes.len())?;
        self.homes.iter().try_for_each(DockHome::validate)?;
        collection(self.bars.len())?;
        self.bars.iter().try_for_each(DockPlacedBar::validate)?;
        collection(self.outside.len())?;
        self.outside.iter().try_for_each(DockOutside::validate)?;
        collection(self.unlocked.len())?;
        self.unlocked.iter().try_for_each(DockPlace::validate)
    }
}

impl DockBarPlacement {
    fn validate(&self) -> Result<(), DecodeError> {
        match self {
            Self::Over(_) | Self::Beside(_) => Ok(()),
            Self::Window(window) if window.origin.x.is_finite() && window.origin.y.is_finite() => {
                Ok(())
            }
            Self::Window(_) => Err(DecodeError::MalformedPayload),
        }
    }
}

impl DockBar {
    fn validate(&self) -> Result<(), DecodeError> {
        self.tab.validate()?;
        self.owner.validate()?;
        self.placement.validate()
    }
}

impl DockPlacedBar {
    fn validate(&self) -> Result<(), DecodeError> {
        self.bar.validate()?;
        self.owner.validate()?;
        self.placement.validate()
    }
}

impl DockOutside {
    fn validate(&self) -> Result<(), DecodeError> {
        self.tab.validate()?;
        match &self.place {
            DockOutsidePlace::Pane(pane) => string(&pane.key),
            DockOutsidePlace::Window(rect) => finite(*rect),
        }
    }
}

impl DockHome {
    fn validate(&self) -> Result<(), DecodeError> {
        self.tab.validate()?;
        self.place.validate()
    }
}

impl DockPlace {
    fn validate(&self) -> Result<(), DecodeError> {
        match self {
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
        collection(self.covers.len())?;
        for cover in &self.covers {
            cover.tab.validate()?;
            finite(cover.rect)?;
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
            Self::PlaceBar(place) => {
                place.bar.validate()?;
                place
                    .placement
                    .as_ref()
                    .map_or(Ok(()), DockBarPlacement::validate)
            }
            Self::MoveBar(moved) => {
                moved.bar.validate()?;
                match moved.at.x.is_finite() && moved.at.y.is_finite() {
                    true => Ok(()),
                    false => Err(DecodeError::MalformedPayload),
                }
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
                .map_or(Ok(()), DockDeclaration::validate_own),
            Self::Request(request) => request.request.validate(),
            Self::Host(host) => host.hosted.as_ref().map_or(Ok(()), DockHosted::validate),
            Self::Close(close) => close.tab.validate(),
            Self::Pick(pick) => {
                pick.tab.validate()?;
                string(&pick.id)
            }
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
            Self::ChildRequest(request) => {
                if request.owner.len() > MAX_DOCK_NESTING {
                    return Err(DecodeError::LimitExceeded("dock nesting"));
                }
                request.request.validate()
            }
        }
    }
}

impl DockUpdate {
    pub fn queue(updates: &mut Vec<Self>, update: Self) {
        match &update {
            Self::Hosted(hosted) => updates.retain(
                |waiting| !matches!(waiting, Self::Hosted(held) if held.view == hosted.view),
            ),
            Self::Child(child) => updates.retain(
                |waiting| !matches!(waiting, Self::Child(held) if held.child == child.child),
            ),
            Self::Close(_) | Self::ChildRequest(_) => {}
        }
        updates.push(update);
    }
}
