use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use beui::NodeId;
use beui::dock::unstyled::{
    DockBodies, DockDeclaration, DockDeclaredTab, DockHostEvent, DockHostLink, DockHosted,
    DockMapping, DockMode, DockPlain, DockPlainEntry, DockPlainHome, DockPlainNode, DockPlainPlace,
    DockPlainWindow, DockRequest, DockSpec, DockSpecEntry, DockSpecNode, DockSpecPane,
    DockSpecWindow, DockState, TabId,
};
use beui::icons::Glyph;
use beui::reactive::*;
use block_editor_plugin::{EditorHost, EditorRegion, Pushed};
use block_plugin_api as api;
use block_plugin_api::{ChildId, DockTabPath};

use crate::{Editor, Subregion};

#[derive(Clone)]
pub(crate) struct DockedTab {
    pub(crate) tab: u64,
    pub(crate) claimed: Rc<Cell<bool>>,
}

impl DockedTab {
    pub(crate) fn claim(&self) -> bool {
        if self.claimed.replace(true) {
            return false;
        }
        let claimed = Rc::clone(&self.claimed);
        on_cleanup(move || claimed.set(false));
        true
    }
}

pub(crate) struct DockedTabs {
    link: RefCell<Option<Rc<ProtocolLink>>>,
    revision: OwnedSignal<u64>,
}

impl Default for DockedTabs {
    fn default() -> Self {
        Self {
            link: RefCell::default(),
            revision: OwnedSignal::new(0),
        }
    }
}

impl DockedTabs {
    fn bump(&self) {
        self.revision.handle().update(|revision| *revision += 1);
    }

    pub(crate) fn link(self: &Rc<Self>, host: EditorHost, editor: Editor) -> Rc<dyn DockHostLink> {
        let link = Rc::new_cyclic(|this| ProtocolLink {
            this: Weak::clone(this),
            host,
            region: EditorRegion::Frame,
            pushed: editor.pushed(Pushed::Docks),
            tabs: Rc::downgrade(self),
            bodies: RefCell::default(),
            panels: RefCell::default(),
        });
        *self.link.borrow_mut() = Some(Rc::clone(&link));
        let tabs = Rc::downgrade(self);
        let held = Rc::downgrade(&link);
        on_cleanup(move || {
            let Some(tabs) = tabs.upgrade() else {
                return;
            };
            let mine = tabs
                .link
                .borrow()
                .as_ref()
                .is_some_and(|link| Weak::ptr_eq(&Rc::downgrade(link), &held));
            if mine {
                tabs.link.borrow_mut().take();
                tabs.bump();
            }
        });
        link
    }

    fn panel(&self, tab: TabId) -> Option<NodeId> {
        let link = self.link.borrow().clone()?;
        link.panel(tab)
    }
}

#[component]
pub(crate) fn DockedTabBody(tabs: Rc<DockedTabs>, tab: u64) -> NodeId {
    let revised = tabs.revision.read_only();
    let node = create_signal(None);
    create_effect(move || {
        revised.get();
        let panel = untrack(|| tabs.panel(TabId::new(tab)));
        node.set(panel);
    });
    view! {
        <beui::styled::List spacing=beui::styled::Space::None>
            <beui::styled::Portal @sizing=beui::ItemSize::Percent(100.0) node={node} />
        </beui::styled::List>
    }
}

struct ProtocolLink {
    this: Weak<ProtocolLink>,
    host: EditorHost,
    region: EditorRegion,
    pushed: ReadSignal<u64>,
    tabs: Weak<DockedTabs>,
    bodies: RefCell<Option<DockBodies>>,
    panels: RefCell<HashMap<TabId, (NodeId, Scope)>>,
}

impl ProtocolLink {
    fn bump(&self) {
        if let Some(tabs) = self.tabs.upgrade() {
            tabs.bump();
        }
    }

    fn panel(&self, tab: TabId) -> Option<NodeId> {
        if let Some((panel, _)) = self.panels.borrow().get(&tab) {
            return Some(*panel);
        }
        let bodies = self.bodies.borrow().clone()?;
        let scope = bodies.scope.child()?;
        let docked = DockedTab {
            tab: tab.value(),
            claimed: Rc::default(),
        };
        let panel = scope.context().run(|| {
            provide_context(docked);
            bodies.body.call(tab)
        });
        self.panels.borrow_mut().insert(tab, (panel, scope));
        Some(panel)
    }

    fn keep(&self, tabs: &[TabId]) {
        let dropped: Vec<(NodeId, Scope)> = {
            let mut panels = self.panels.borrow_mut();
            let gone: Vec<TabId> = panels
                .keys()
                .filter(|tab| !tabs.contains(tab))
                .copied()
                .collect();
            gone.into_iter()
                .filter_map(|tab| panels.remove(&tab))
                .collect()
        };
        if !dropped.is_empty() {
            drop(dropped);
            self.bump();
        }
    }
}

fn own(tab: &DockTabPath) -> Option<TabId> {
    tab.is_own().then(|| TabId::new(tab.tab))
}

fn own_path(tab: TabId) -> Option<DockTabPath> {
    Some(DockTabPath::own(tab.value()))
}

impl DockHostLink for ProtocolLink {
    fn declare(&self, declaration: DockDeclaration) {
        self.host.declare_dock(
            self.region,
            Some(declaration_to_api(&declaration, &own_path)),
        );
    }

    fn hosted(&self) -> Memo<Option<DockHosted>> {
        let host = self.host.clone();
        let region = self.region;
        let pushed = self.pushed;
        create_memo(move || {
            pushed.get();
            let hosted = host.dock_hosted(region)?;
            Some(hosted_from_api(&hosted, &mut |path| own(path)))
        })
    }

    fn request(&self, request: DockRequest) {
        if let Some(request) = request_to_api(&request, &own_path) {
            self.host.request_dock(self.region, request);
        }
    }

    fn listen(&self, events: Callback<DockHostEvent>) {
        let host = self.host.clone();
        let region = self.region;
        let pushed = self.pushed;
        create_effect(move || {
            pushed.get();
            for tab in untrack(|| host.take_dock_closes(region)) {
                if let Some(tab) = own(&tab) {
                    untrack(|| events.call(DockHostEvent::Close(tab)));
                }
            }
        });
    }

    fn place(&self, bodies: DockBodies) {
        *self.bodies.borrow_mut() = Some(bodies);
        let hosted = self.hosted();
        let this = Weak::clone(&self.this);
        create_effect(move || {
            let held = hosted.with(|hosted| {
                hosted
                    .as_ref()
                    .map(|hosted| hosted.tabs.clone())
                    .unwrap_or_default()
            });
            if let Some(link) = this.upgrade() {
                untrack(|| link.keep(&held));
            }
        });
        self.bump();
    }

    fn withdraw(&self) {
        self.host.declare_dock(self.region, None);
        self.bodies.borrow_mut().take();
        let panels = std::mem::take(&mut *self.panels.borrow_mut());
        drop(panels);
        self.bump();
    }
}

#[derive(Default)]
struct Paths {
    tabs: HashMap<DockTabPath, TabId>,
    paths: HashMap<TabId, DockTabPath>,
    next: u64,
}

impl Paths {
    fn tab(&mut self, path: &DockTabPath) -> TabId {
        if let Some(tab) = self.tabs.get(path) {
            return *tab;
        }
        self.next += 1;
        let tab = TabId::new(self.next);
        self.tabs.insert(path.clone(), tab);
        self.paths.insert(tab, path.clone());
        tab
    }

    fn path(&self, tab: TabId) -> Option<DockTabPath> {
        self.paths.get(&tab).cloned()
    }
}

fn group_key(nodes: &api::DockNodes, owner: &[ChildId]) -> Option<String> {
    fn find(node: &api::DockNode, child: ChildId) -> Option<(&api::DockGroup, &api::DockNode)> {
        match node {
            api::DockNode::Split { first, second, .. } => {
                find(first, child).or_else(|| find(second, child))
            }
            api::DockNode::Pane { entries, .. } => entries.iter().find_map(|entry| match entry {
                api::DockEntry::Tab(_) => None,
                api::DockEntry::Group { group, root } if group.child == Some(child) => {
                    Some((group, root.as_ref()))
                }
                api::DockEntry::Group { root, .. } => find(root, child),
            }),
        }
    }
    let roots: Vec<&api::DockNode> = nodes
        .main
        .iter()
        .chain(
            nodes
                .windows
                .iter()
                .filter_map(|window| window.root.as_ref()),
        )
        .collect();
    let (first, rest) = owner.split_first()?;
    let (mut group, mut root) = roots.iter().find_map(|root| find(root, *first))?;
    for child in rest {
        (group, root) = find(root, *child)?;
    }
    Some(group.key.clone())
}

pub(crate) fn bridge_child_dock(
    editor: Editor,
    link: Rc<dyn DockHostLink>,
    state: ReadSignal<crate::ChildState>,
) {
    let host = editor.host();
    let pushed = editor.pushed(Pushed::Docks);
    let child = create_signal(None::<ChildId>);
    let last = Rc::new(Cell::new(None::<ChildId>));
    {
        let last = Rc::clone(&last);
        create_effect(move || {
            let placed = state.with(|state| state.child);
            if placed.is_some() && placed != last.get() {
                last.set(placed);
                child.set(placed);
            }
        });
    }
    let paths = Rc::new(RefCell::new(Paths::default()));
    let declared = Rc::new(Cell::new(false));
    {
        let link = Rc::clone(&link);
        let host = host.clone();
        let paths = Rc::clone(&paths);
        create_effect(move || {
            pushed.get();
            let Some(child) = child.get() else {
                return;
            };
            untrack(|| {
                let declaration = host.child_dock(child);
                let converted = declaration.as_ref().and_then(|declaration| {
                    declaration_from_api(declaration, &mut paths.borrow_mut())
                });
                match converted {
                    Some(converted) => {
                        link.declare(converted);
                        declared.set(true);
                    }
                    None if declared.replace(false) => link.withdraw(),
                    None => {}
                }
                for (owner, request) in host.take_child_dock_requests(child) {
                    let request = match owner.is_empty() {
                        true => request_from_api(&request, &mut paths.borrow_mut()),
                        false => declaration.as_ref().and_then(|declaration| {
                            nested_restore(&*link, declaration, &owner, &request, &paths)
                        }),
                    };
                    if let Some(request) = request {
                        link.request(request);
                    }
                }
            });
        });
    }
    let hosted = link.hosted();
    {
        let host = host.clone();
        let paths = Rc::clone(&paths);
        create_effect(move || {
            let Some(child) = child.get() else {
                return;
            };
            let hosted = hosted.get();
            untrack(|| {
                let paths = paths.borrow();
                let hosted = hosted.map(|hosted| hosted_to_api(&hosted, &|tab| paths.path(tab)));
                host.host_child_dock(child, hosted);
            });
        });
    }
    {
        let host = host.clone();
        let paths = Rc::clone(&paths);
        let closing = Rc::clone(&last);
        link.listen(Callback::new(move |event: DockHostEvent| {
            let DockHostEvent::Close(tab) = event;
            let path = paths.borrow().path(tab);
            if let (Some(child), Some(path)) = (closing.get(), path) {
                host.close_child_dock_tab(child, path);
            }
        }));
    }
    if let Some(scope) = owner_scope() {
        let paths = Rc::clone(&paths);
        let shown = Rc::clone(&last);
        let body = RenderFn::new(move |tab: TabId| {
            let path = paths.borrow().path(tab);
            let placed = shown
                .get()
                .zip(path)
                .map(|(child, path)| crate::SubregionContent::DockTab(path.through(child)));
            view! {
                <beui::styled::List spacing=beui::styled::Space::None>
                    <Subregion
                        @sizing=beui::ItemSize::Percent(100.0)
                        editor={editor}
                        placed={Prop::Static(placed)}
                        mode=api::ChildMode::Live
                    />
                </beui::styled::List>
            }
        });
        link.place(DockBodies {
            scope,
            body,
            empty: RenderFn::new(|_: String| None),
        });
    }
    on_cleanup(move || {
        if let Some(child) = last.get() {
            host.host_child_dock(child, None);
        }
        link.withdraw();
    });
}

fn nested_restore(
    link: &dyn DockHostLink,
    declaration: &api::DockDeclaration,
    owner: &[ChildId],
    request: &api::DockRequest,
    paths: &Rc<RefCell<Paths>>,
) -> Option<DockRequest> {
    let api::DockRequest::Restore(layout) = request else {
        let mut through = request.clone();
        for child in owner.iter().rev() {
            through = through_owner(&through, *child);
        }
        return request_from_api(&through, &mut paths.borrow_mut());
    };
    let nodes = declaration.nest().ok()?;
    let group = group_key(&nodes, owner)?;
    let prefixed = |key: &str| match key.is_empty() {
        true => String::new(),
        false => format!("{group}/{key}"),
    };
    let mut layout = layout.clone();
    for child in owner.iter().rev() {
        layout = through_layout(&layout, *child);
    }
    let from = {
        let mut paths = paths.borrow_mut();
        let plain = layout_to_plain(&layout, &mut |path| Some(paths.tab(path)));
        DockState::from_plain(&rekeyed(&plain, &prefixed))
    };
    let hosted = untrack(|| link.hosted().get_untracked())?;
    let mut state = hosted.state;
    let found = state.find_group(&group)?;
    state.graft(
        found,
        &from,
        &DockMapping {
            tab: &|tab| Some(tab),
            key: &|key| Some(key.to_owned()),
            group: &|_| None,
        },
    );
    Some(DockRequest::Restore(state))
}

fn through_owner(request: &api::DockRequest, child: ChildId) -> api::DockRequest {
    match request {
        api::DockRequest::Show(tab) => api::DockRequest::Show(tab.clone().through(child)),
        api::DockRequest::Restore(layout) => {
            api::DockRequest::Restore(through_layout(layout, child))
        }
        api::DockRequest::Fullscreen(fullscreen) => {
            api::DockRequest::Fullscreen(fullscreen.as_ref().map(|fullscreen| {
                api::DockFullscreen {
                    tab: fullscreen.tab.clone().through(child),
                    area: fullscreen.area,
                }
            }))
        }
    }
}

fn through_layout(layout: &api::DockLayout, child: ChildId) -> api::DockLayout {
    let mut layout = layout.clone();
    for item in &mut layout.tree.items {
        if let api::DockItemKind::Tab(tab) = &mut item.kind {
            *tab = tab.clone().through(child);
        }
    }
    layout.tree.focus = layout.tree.focus.map(|focus| focus.through(child));
    for home in &mut layout.homes {
        home.tab = home.tab.clone().through(child);
    }
    layout
}

fn rekeyed(plain: &DockPlain, key: &dyn Fn(&str) -> String) -> DockPlain {
    fn node(held: &DockPlainNode, key: &dyn Fn(&str) -> String) -> DockPlainNode {
        match held {
            DockPlainNode::Split {
                key: own,
                direction,
                fraction,
                first,
                second,
            } => DockPlainNode::Split {
                key: key(own),
                direction: *direction,
                fraction: *fraction,
                first: Box::new(node(first, key)),
                second: Box::new(node(second, key)),
            },
            DockPlainNode::Pane {
                key: own,
                active,
                vertical,
                icons_only,
                sidebar,
                keep,
                area,
                entries,
            } => DockPlainNode::Pane {
                key: key(own),
                active: *active,
                vertical: *vertical,
                icons_only: *icons_only,
                sidebar: *sidebar,
                keep: *keep,
                area: area.as_deref().map(key),
                entries: entries
                    .iter()
                    .map(|entry| match entry {
                        DockPlainEntry::Tab(tab) => DockPlainEntry::Tab(*tab),
                        DockPlainEntry::Group {
                            key: own,
                            pinned,
                            root,
                        } => DockPlainEntry::Group {
                            key: key(own),
                            pinned: *pinned,
                            root: Box::new(node(root, key)),
                        },
                    })
                    .collect(),
            },
        }
    }
    DockPlain {
        main: plain.main.as_ref().map(|main| node(main, key)),
        windows: plain
            .windows
            .iter()
            .map(|window| DockPlainWindow {
                key: key(&window.key),
                rect: window.rect,
                root: window.root.as_ref().map(|root| node(root, key)),
            })
            .collect(),
        focus: plain.focus,
        homes: plain
            .homes
            .iter()
            .map(|home| DockPlainHome {
                tab: home.tab,
                place: match &home.place {
                    DockPlainPlace::Area(area) => DockPlainPlace::Area(key(area)),
                    DockPlainPlace::Group(group) => DockPlainPlace::Group(key(group)),
                },
                pinned: home.pinned,
            })
            .collect(),
    }
}

type PathOf<'a> = &'a dyn Fn(TabId) -> Option<DockTabPath>;
type TabOf<'a> = &'a mut dyn FnMut(&DockTabPath) -> Option<TabId>;

fn direction_to_api(direction: beui::Direction) -> api::DockDirection {
    match direction {
        beui::Direction::Horizontal => api::DockDirection::Horizontal,
        beui::Direction::Vertical => api::DockDirection::Vertical,
    }
}

fn direction_from_api(direction: api::DockDirection) -> beui::Direction {
    match direction {
        api::DockDirection::Horizontal => beui::Direction::Horizontal,
        api::DockDirection::Vertical => beui::Direction::Vertical,
    }
}

fn spec_node(node: &DockSpecNode, path: PathOf<'_>) -> api::DockNode {
    match node {
        DockSpecNode::Split {
            key,
            direction,
            fraction,
            first,
            second,
        } => api::DockNode::Split {
            split: api::DockSplit {
                key: key.clone(),
                direction: direction_to_api(*direction),
                fraction: *fraction,
            },
            first: Box::new(spec_node(first, path)),
            second: Box::new(spec_node(second, path)),
        },
        DockSpecNode::Pane(pane) => {
            let entries: Vec<(Option<TabId>, api::DockEntry)> = pane
                .entries
                .iter()
                .filter_map(|entry| match entry {
                    DockSpecEntry::Tab(tab) => Some((Some(*tab), api::DockEntry::Tab(path(*tab)?))),
                    DockSpecEntry::Group { key, pinned, root } => Some((
                        None,
                        api::DockEntry::Group {
                            group: api::DockGroup {
                                key: key.clone(),
                                pinned: *pinned,
                                child: None,
                            },
                            root: Box::new(spec_node(root, path)),
                        },
                    )),
                })
                .collect();
            let active = pane.active.and_then(|active| {
                entries
                    .iter()
                    .position(|(tab, _)| *tab == Some(active))
                    .map(|index| index as u32)
            });
            api::DockNode::Pane {
                pane: api::DockPane {
                    key: pane.key.clone(),
                    active,
                    vertical: pane.vertical,
                    icons_only: pane.icons_only,
                    sidebar: pane.sidebar,
                    keep: pane.keep,
                    area: pane.locked.then(|| pane.key.clone()),
                },
                entries: entries.into_iter().map(|(_, entry)| entry).collect(),
            }
        }
    }
}

pub(crate) fn declaration_to_api(
    declaration: &DockDeclaration,
    path: PathOf<'_>,
) -> api::DockDeclaration {
    let spec = &declaration.spec;
    let nodes = api::DockNodes {
        main: spec.main.as_ref().map(|main| spec_node(main, path)),
        windows: spec
            .windows
            .iter()
            .map(|window| api::DockFloating {
                window: api::DockWindow {
                    key: window.key.clone(),
                    rect: window.rect,
                },
                root: window.root.as_ref().map(|root| spec_node(root, path)),
            })
            .collect(),
    };
    api::DockDeclaration {
        tree: api::DockTree::flatten(&nodes, spec.focus.and_then(path)),
        tabs: declaration
            .tabs
            .iter()
            .filter_map(|tab| {
                Some(api::DockTab {
                    tab: path(tab.tab)?,
                    key: tab.key.clone(),
                    title: tab.title.clone(),
                    glyph: tab
                        .icon
                        .map(|icon| icon.as_str().to_owned())
                        .unwrap_or_default(),
                    closable: tab.closable,
                })
            })
            .collect(),
    }
}

fn api_node(node: &api::DockNode, paths: &mut Paths) -> DockSpecNode {
    match node {
        api::DockNode::Split {
            split,
            first,
            second,
        } => DockSpecNode::Split {
            key: split.key.clone(),
            direction: direction_from_api(split.direction),
            fraction: split.fraction,
            first: Box::new(api_node(first, paths)),
            second: Box::new(api_node(second, paths)),
        },
        api::DockNode::Pane { pane, entries } => {
            let entries: Vec<DockSpecEntry> = entries
                .iter()
                .map(|entry| match entry {
                    api::DockEntry::Tab(tab) => DockSpecEntry::Tab(paths.tab(tab)),
                    api::DockEntry::Group { group, root } => DockSpecEntry::Group {
                        key: group.key.clone(),
                        pinned: group.pinned,
                        root: Box::new(api_node(root, paths)),
                    },
                })
                .collect();
            let active = pane
                .active
                .and_then(|active| entries.get(active as usize))
                .and_then(|entry| match entry {
                    DockSpecEntry::Tab(tab) => Some(*tab),
                    DockSpecEntry::Group { .. } => None,
                });
            DockSpecNode::Pane(DockSpecPane {
                key: pane.key.clone(),
                entries,
                active,
                vertical: pane.vertical,
                icons_only: pane.icons_only,
                sidebar: pane.sidebar,
                keep: pane.keep,
                locked: pane.area.as_ref() == Some(&pane.key),
            })
        }
    }
}

fn declaration_from_api(
    declaration: &api::DockDeclaration,
    paths: &mut Paths,
) -> Option<DockDeclaration> {
    let nodes = declaration.nest().ok()?;
    let spec = DockSpec {
        main: nodes.main.as_ref().map(|main| api_node(main, paths)),
        windows: nodes
            .windows
            .iter()
            .map(|window| DockSpecWindow {
                key: window.window.key.clone(),
                rect: window.window.rect,
                root: window.root.as_ref().map(|root| api_node(root, paths)),
            })
            .collect(),
        focus: declaration
            .tree
            .focus
            .as_ref()
            .map(|focus| paths.tab(focus)),
    };
    let tabs = declaration
        .tabs
        .iter()
        .map(|tab| DockDeclaredTab {
            tab: paths.tab(&tab.tab),
            key: tab.key.clone(),
            title: tab.title.clone(),
            icon: Glyph::from_codepoint(&tab.glyph),
            closable: tab.closable,
        })
        .collect();
    Some(DockDeclaration { spec, tabs })
}

fn plain_node(node: &DockPlainNode, path: PathOf<'_>) -> api::DockNode {
    match node {
        DockPlainNode::Split {
            key,
            direction,
            fraction,
            first,
            second,
        } => api::DockNode::Split {
            split: api::DockSplit {
                key: key.clone(),
                direction: direction_to_api(*direction),
                fraction: *fraction,
            },
            first: Box::new(plain_node(first, path)),
            second: Box::new(plain_node(second, path)),
        },
        DockPlainNode::Pane {
            key,
            active,
            vertical,
            icons_only,
            sidebar,
            keep,
            area,
            entries,
        } => {
            let mut kept = Vec::new();
            let mut shown = None;
            for (index, entry) in entries.iter().enumerate() {
                let entry = match entry {
                    DockPlainEntry::Tab(tab) => path(*tab).map(api::DockEntry::Tab),
                    DockPlainEntry::Group { key, pinned, root } => Some(api::DockEntry::Group {
                        group: api::DockGroup {
                            key: key.clone(),
                            pinned: *pinned,
                            child: None,
                        },
                        root: Box::new(plain_node(root, path)),
                    }),
                };
                if let Some(entry) = entry {
                    if index == *active {
                        shown = Some(kept.len() as u32);
                    }
                    kept.push(entry);
                }
            }
            api::DockNode::Pane {
                pane: api::DockPane {
                    key: key.clone(),
                    active: shown.or((!kept.is_empty()).then_some(0)),
                    vertical: *vertical,
                    icons_only: *icons_only,
                    sidebar: *sidebar,
                    keep: *keep,
                    area: area.clone(),
                },
                entries: kept,
            }
        }
    }
}

fn plain_to_layout(plain: &DockPlain, path: PathOf<'_>) -> api::DockLayout {
    let nodes = api::DockNodes {
        main: plain.main.as_ref().map(|main| plain_node(main, path)),
        windows: plain
            .windows
            .iter()
            .map(|window| api::DockFloating {
                window: api::DockWindow {
                    key: window.key.clone(),
                    rect: window.rect,
                },
                root: window.root.as_ref().map(|root| plain_node(root, path)),
            })
            .collect(),
    };
    api::DockLayout {
        tree: api::DockTree::flatten(&nodes, plain.focus.and_then(path)),
        homes: plain
            .homes
            .iter()
            .filter_map(|home| {
                Some(api::DockHome {
                    tab: path(home.tab)?,
                    place: match &home.place {
                        DockPlainPlace::Area(area) => api::DockPlace::Area(area.clone()),
                        DockPlainPlace::Group(group) => api::DockPlace::Group(group.clone()),
                    },
                    pinned: home.pinned,
                })
            })
            .collect(),
    }
}

fn api_plain_node(node: &api::DockNode, tab: TabOf<'_>) -> DockPlainNode {
    match node {
        api::DockNode::Split {
            split,
            first,
            second,
        } => DockPlainNode::Split {
            key: split.key.clone(),
            direction: direction_from_api(split.direction),
            fraction: split.fraction,
            first: Box::new(api_plain_node(first, tab)),
            second: Box::new(api_plain_node(second, tab)),
        },
        api::DockNode::Pane { pane, entries } => {
            let mut kept = Vec::new();
            let mut active = 0;
            for (index, entry) in entries.iter().enumerate() {
                let entry = match entry {
                    api::DockEntry::Tab(path) => tab(path).map(DockPlainEntry::Tab),
                    api::DockEntry::Group { group, root } => Some(DockPlainEntry::Group {
                        key: group.key.clone(),
                        pinned: group.pinned,
                        root: Box::new(api_plain_node(root, tab)),
                    }),
                };
                if let Some(entry) = entry {
                    if pane.active == Some(index as u32) {
                        active = kept.len();
                    }
                    kept.push(entry);
                }
            }
            DockPlainNode::Pane {
                key: pane.key.clone(),
                active,
                vertical: pane.vertical,
                icons_only: pane.icons_only,
                sidebar: pane.sidebar,
                keep: pane.keep,
                area: pane.area.clone(),
                entries: kept,
            }
        }
    }
}

fn layout_to_plain(layout: &api::DockLayout, tab: TabOf<'_>) -> DockPlain {
    let Ok(nodes) = layout.tree.nest() else {
        return DockPlain::default();
    };
    DockPlain {
        main: nodes.main.as_ref().map(|main| api_plain_node(main, tab)),
        windows: nodes
            .windows
            .iter()
            .map(|window| DockPlainWindow {
                key: window.window.key.clone(),
                rect: window.window.rect,
                root: window.root.as_ref().map(|root| api_plain_node(root, tab)),
            })
            .collect(),
        focus: layout.tree.focus.as_ref().and_then(&mut *tab),
        homes: layout
            .homes
            .iter()
            .filter_map(|home| {
                Some(DockPlainHome {
                    tab: tab(&home.tab)?,
                    place: match &home.place {
                        api::DockPlace::Area(area) => DockPlainPlace::Area(area.clone()),
                        api::DockPlace::Group(group) => DockPlainPlace::Group(group.clone()),
                    },
                    pinned: home.pinned,
                })
            })
            .collect(),
    }
}

fn hosted_from_api(hosted: &api::DockHosted, tab: TabOf<'_>) -> DockHosted {
    let state = DockState::from_plain(&layout_to_plain(&hosted.layout, tab));
    let mut list =
        |paths: &[DockTabPath]| -> Vec<TabId> { paths.iter().filter_map(&mut *tab).collect() };
    let tabs = list(&hosted.tabs);
    let visible = list(&hosted.visible);
    DockHosted {
        state,
        tabs,
        focused: hosted.focused.as_ref().and_then(&mut *tab),
        shown: hosted.shown.as_ref().and_then(&mut *tab),
        visible,
        fullscreen: hosted.fullscreen.as_ref().and_then(&mut *tab),
        mode: match hosted.stacked {
            true => DockMode::Stacked,
            false => DockMode::Tiled,
        },
    }
}

fn hosted_to_api(hosted: &DockHosted, path: PathOf<'_>) -> api::DockHosted {
    let list = |tabs: &[TabId]| tabs.iter().filter_map(|tab| path(*tab)).collect();
    api::DockHosted {
        layout: plain_to_layout(&hosted.state.plain(), path),
        tabs: list(&hosted.tabs),
        focused: hosted.focused.and_then(path),
        shown: hosted.shown.and_then(path),
        visible: list(&hosted.visible),
        fullscreen: hosted.fullscreen.and_then(path),
        stacked: hosted.mode == DockMode::Stacked,
    }
}

fn request_to_api(request: &DockRequest, path: PathOf<'_>) -> Option<api::DockRequest> {
    Some(match request {
        DockRequest::Show(tab) => api::DockRequest::Show(path(*tab)?),
        DockRequest::Restore(state) => {
            api::DockRequest::Restore(plain_to_layout(&state.plain(), path))
        }
        DockRequest::Fullscreen(None) => api::DockRequest::Fullscreen(None),
        DockRequest::Fullscreen(Some((tab, area))) => {
            api::DockRequest::Fullscreen(Some(api::DockFullscreen {
                tab: path(*tab)?,
                area: *area,
            }))
        }
    })
}

fn request_from_api(request: &api::DockRequest, paths: &mut Paths) -> Option<DockRequest> {
    Some(match request {
        api::DockRequest::Show(tab) => DockRequest::Show(paths.tab(tab)),
        api::DockRequest::Restore(layout) => DockRequest::Restore(DockState::from_plain(
            &layout_to_plain(layout, &mut |path| Some(paths.tab(path))),
        )),
        api::DockRequest::Fullscreen(None) => DockRequest::Fullscreen(None),
        api::DockRequest::Fullscreen(Some(fullscreen)) => {
            DockRequest::Fullscreen(Some((paths.tab(&fullscreen.tab), fullscreen.area)))
        }
    })
}
