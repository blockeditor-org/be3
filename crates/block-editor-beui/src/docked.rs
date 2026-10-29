use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use beui::NodeId;
use beui::dock::unstyled::{
    DockBodies, DockDeclaration, DockDeclaredDialog, DockDeclaredTab, DockDialogPresentation,
    DockHostEvent, DockHostLink, DockHostSlot, DockHosted, DockMapping, DockMode, DockPlain,
    DockPlainEntry, DockPlainHome, DockPlainNode, DockPlainPlace, DockPlainWindow, DockRequest,
    DockSpec, DockSpecEntry, DockSpecNode, DockSpecPane, DockSpecWindow, DockState, TabId,
};
use beui::dock::unstyled::{DockMenus, DockTabControl, DockTabMenu, dock_menu};
use beui::icons::Glyph;
use beui::reactive::*;
use beui::styled::WindowHandle;
use block_editor_plugin::{EditorHost, EditorRegion, Pushed, RegionId};
use block_plugin_api as api;
use block_plugin_api::{ChildId, DockTabPath};
use uuid::Uuid;

use crate::child::{ChildDockTab, MenuMirror, MenuPicks};
use crate::{Editor, Subregion};

const EMPTY_BODIES: u64 = 1 << 62;

pub(crate) struct DockedTabs {
    link: RefCell<Option<Rc<ProtocolLink>>>,
    revision: OwnedSignal<u64>,
    slot: RefCell<Option<DockHostSlot>>,
    built: Cell<Option<bool>>,
    asking: OwnedSignal<Option<TabId>>,
    windows: RefCell<HashMap<u64, beui::WindowId>>,
}

impl Default for DockedTabs {
    fn default() -> Self {
        Self {
            link: RefCell::default(),
            revision: OwnedSignal::new(0),
            slot: RefCell::default(),
            built: Cell::new(None),
            asking: OwnedSignal::new(None),
            windows: RefCell::default(),
        }
    }
}

impl DockedTabs {
    pub(crate) fn provide_slot(&self, link: Option<Rc<dyn DockHostLink>>, docked: bool) {
        let slot = DockHostSlot::new(link);
        *self.slot.borrow_mut() = Some(slot.clone());
        self.built.set(Some(docked));
        provide_context(slot);
    }

    pub(crate) fn key(&self, docked: bool) -> bool {
        let asked = self.slot.borrow().as_ref().is_some_and(DockHostSlot::asked);
        match (asked, self.built.get()) {
            (false, Some(built)) => built,
            _ => docked,
        }
    }

    fn bump(&self) {
        self.revision.handle().update(|revision| *revision += 1);
    }

    pub(crate) fn link(
        self: &Rc<Self>,
        host: EditorHost,
        editor: Editor,
        frame: ReadSignal<Vec<MenuNode>>,
    ) -> Rc<dyn DockHostLink> {
        let link = Rc::new_cyclic(|this| ProtocolLink {
            this: Weak::clone(this),
            host,
            region: EditorRegion::Frame,
            pushed: editor.pushed(Pushed::Docks),
            tabs: Rc::downgrade(self),
            bodies: RefCell::default(),
            panels: RefCell::default(),
            menus: OwnedSignal::new(DockMenus::new()),
            frame,
            declared: RefCell::default(),
            watched: Cell::new(None),
            empties: RefCell::default(),
            asking: self.asking.handle(),
        });
        *self.link.borrow_mut() = Some(Rc::clone(&link));
        let menus = link.menus.handle();
        let menued = Rc::downgrade(&link);
        create_effect(move || {
            frame.with(|_| ());
            menus.with(|menus| {
                for (_, items) in menus.values() {
                    items.with(|_| ());
                }
            });
            if let Some(link) = menued.upgrade() {
                untrack(|| link.send());
            }
        });
        let picking = Rc::downgrade(&link);
        let pushed = link.pushed;
        let picks = link.host.clone();
        create_effect(move || {
            pushed.get();
            let taken = untrack(|| picks.take_dock_picks(EditorRegion::Frame));
            if let Some(link) = picking.upgrade() {
                for (tab, id) in taken {
                    if let Some(tab) = own(&tab) {
                        untrack(|| link.pick(tab, &id));
                    }
                }
            }
        });
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

    fn panel(&self, tab: TabId, window: Option<WindowHandle>) -> Option<NodeId> {
        let link = self.link.borrow().clone()?;
        link.panel(tab, window)
    }

    fn leave(&self, tab: TabId, window: WindowHandle) {
        if let Some(link) = self.link.borrow().clone() {
            link.leave(tab, window);
        }
    }

    pub(crate) fn window_id(&self, tab: u64) -> beui::WindowId {
        *self
            .windows
            .borrow_mut()
            .entry(tab)
            .or_insert_with(beui::WindowId::next)
    }

    pub(crate) fn presenting(&self, host: &EditorHost) -> Option<beui::WindowId> {
        let asking = self.asking.get();
        let link = self.link.borrow().clone()?;
        let hosted = link.watched.get()?.get()?;
        let tab = asking
            .or(hosted.focused)
            .filter(|tab| hosted.visible.contains(tab))
            .or_else(|| hosted.visible.first().copied())?;
        crate::module::Module::of(host)?.window(RegionId {
            instance: host.instance()?,
            region: EditorRegion::Tab(tab.value()),
        })
    }

    pub(crate) fn release(&self) {
        if self.asking.get_untracked().is_some() {
            self.asking.handle().set(None);
        }
    }
}

#[component]
pub(crate) fn DockedTabBody(tabs: Rc<DockedTabs>, tab: u64) -> NodeId {
    let revised = tabs.revision.read_only();
    let node = create_signal(None);
    let window = beui::styled::use_window();
    let left = Rc::clone(&tabs);
    create_effect(move || {
        revised.get();
        let panel = untrack(|| tabs.panel(TabId::new(tab), window));
        node.set(panel);
    });
    on_cleanup(move || {
        if let Some(window) = window {
            left.leave(TabId::new(tab), window);
        }
    });
    view! {
        <beui::styled::List spacing=beui::styled::Space::None>
            <beui::styled::Portal @sizing=beui::ItemSize::Percent(100.0) node={node} />
        </beui::styled::List>
    }
}

struct Panel {
    node: NodeId,
    window: Option<Signal<Option<WindowHandle>>>,
    _scope: Scope,
}

struct ProtocolLink {
    this: Weak<ProtocolLink>,
    host: EditorHost,
    region: EditorRegion,
    pushed: ReadSignal<u64>,
    tabs: Weak<DockedTabs>,
    bodies: RefCell<Option<DockBodies>>,
    panels: RefCell<HashMap<TabId, Panel>>,
    menus: OwnedSignal<DockMenus>,
    frame: ReadSignal<Vec<MenuNode>>,
    declared: RefCell<Option<DockDeclaration>>,
    watched: Cell<Option<Memo<Option<DockHosted>>>>,
    empties: RefCell<HashMap<String, u64>>,
    asking: Signal<Option<TabId>>,
}

impl ProtocolLink {
    fn empty_body(&self, key: &str) -> u64 {
        let mut empties = self.empties.borrow_mut();
        let next = EMPTY_BODIES + empties.len() as u64;
        *empties.entry(key.to_owned()).or_insert(next)
    }

    fn empty_key(&self, tab: TabId) -> Option<String> {
        self.empties
            .borrow()
            .iter()
            .find(|(_, held)| **held == tab.value())
            .map(|(key, _)| key.clone())
    }

    fn pick(&self, tab: TabId, id: &str) {
        let menus = self.menus.handle();
        let own = menus.with_untracked(|menus| {
            menus
                .get(&tab)
                .and_then(|(_, items)| menu_action(&items.get_untracked(), id))
        });
        let Some(action) = own.or_else(|| menu_action(&self.frame.get_untracked(), id)) else {
            return;
        };
        self.asking.set(Some(tab));
        action.run();
    }

    fn send(&self) {
        let declared = self.declared.borrow();
        let Some(declaration) = declared.as_ref() else {
            return;
        };
        for pane in spec_panes(&declaration.spec)
            .into_iter()
            .filter(|pane| pane.keep)
        {
            self.empty_body(&pane.key);
        }
        let empties = self.empties.borrow().clone();
        let menus = self.menus.handle();
        let frame = self.frame.get_untracked();
        let menu = |tab: TabId| {
            let mut nodes = menus.with_untracked(|menus| {
                menus
                    .get(&tab)
                    .map(|(_, items)| items.get_untracked())
                    .unwrap_or_default()
            });
            if !nodes.is_empty() && !frame.is_empty() {
                nodes.push(MenuNode::Separator);
            }
            nodes.extend(frame.iter().cloned());
            api::MenuEntry::flatten(&crate::beui_frame::menu_entries(&nodes))
        };
        let empty = |pane: &DockSpecPane| {
            pane.keep
                .then(|| empties.get(&pane.key).copied())
                .flatten()
                .map(DockTabPath::own)
        };
        self.host.declare_dock(
            self.region,
            Some(declaration_to_api(declaration, &own_path, &menu, &empty)),
        );
    }

    fn bump(&self) {
        if let Some(tabs) = self.tabs.upgrade() {
            tabs.bump();
        }
    }

    fn panel(&self, tab: TabId, window: Option<WindowHandle>) -> Option<NodeId> {
        if let Some(panel) = self.panels.borrow().get(&tab) {
            if let (Some(shown), Some(window)) = (panel.window, window) {
                shown.set(Some(window));
            }
            return Some(panel.node);
        }
        let bodies = self.bodies.borrow().clone()?;
        let scope = bodies.scope.child()?;
        let menu = DockTabMenu {
            tab,
            menu: self.menus.handle(),
        };
        let this = Weak::clone(&self.this);
        let watched = self.watched.get();
        let empty = self.empty_key(tab);
        let mut shown = None;
        let panel = scope.context().run(|| {
            if let Some(window) = window {
                let current = create_signal(Some(window));
                shown = Some(current);
                provide_context(WindowHandle::following(window.id, move || current.get()));
            }
            if let Some(key) = empty {
                return bodies.empty.call(key);
            }
            provide_context(menu);
            if let Some(hosted) = watched {
                provide_context(DockTabControl::hosted(
                    tab,
                    hosted,
                    Callback::new(move |request: DockRequest| {
                        if let Some(link) = this.upgrade() {
                            link.request(request);
                        }
                    }),
                ));
            }
            Some(bodies.body.call(tab))
        })?;
        self.panels.borrow_mut().insert(
            tab,
            Panel {
                node: panel,
                window: shown,
                _scope: scope,
            },
        );
        Some(panel)
    }

    fn leave(&self, tab: TabId, window: WindowHandle) {
        let shown = self
            .panels
            .borrow()
            .get(&tab)
            .and_then(|panel| panel.window);
        if let Some(shown) = shown
            && shown.get_untracked() == Some(window)
        {
            shown.set(None);
        }
    }

    fn keep(&self, tabs: &[TabId]) {
        let mut tabs = tabs.to_vec();
        if let Some(bodies) = self.bodies.borrow().as_ref() {
            tabs.extend(bodies.hosts.get_untracked());
        }
        if let Some(declaration) = self.declared.borrow().as_ref() {
            tabs.extend(declaration.dialogs.iter().map(|dialog| dialog.tab));
        }
        tabs.extend(self.empties.borrow().values().copied().map(TabId::new));
        let dropped: Vec<Panel> = {
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
        *self.declared.borrow_mut() = Some(declaration);
        self.send();
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
        self.watched.set(Some(hosted));
        let this = Weak::clone(&self.this);
        let hosts = self.bodies.borrow().as_ref().map(|bodies| bodies.hosts);
        create_effect(move || {
            if let Some(hosts) = hosts {
                hosts.get();
            }
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
        self.declared.borrow_mut().take();
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
    docked_tab: Option<RenderFn<ChildDockTab, NodeId>>,
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
        let declared = host.clone();
        let menus = host.clone();
        let host = host.clone();
        let body = RenderFn::new(move |tab: TabId| {
            let path = paths.borrow().path(tab);
            let child = shown.get();
            let declaration = child.and_then(|child| untrack(|| declared.child_dock(child)));
            let dialog = declaration
                .as_ref()
                .zip(path.as_ref())
                .is_some_and(|(declaration, path)| declaration.dialog(path).is_some());
            let owner = declaration
                .as_ref()
                .zip(path.as_ref())
                .and_then(|(declaration, path)| {
                    declaration.owner_block(&path.via).map(Uuid::from_bytes)
                });
            let menu = tab_menu(editor, menus.clone(), pushed, child, path.clone());
            let shown = child.zip(path);
            let body = view! {
                <DockTabBody editor shown />
            };
            match (dialog, docked_tab) {
                (true, _) => body,
                (false, Some(framed)) => framed.call(ChildDockTab { menu, body, owner }),
                (false, None) => {
                    dock_menu(menu);
                    body
                }
            }
        });
        let emptied = Rc::clone(&last);
        let empty = RenderFn::new(move |key: String| {
            let child = emptied.get()?;
            let declaration = untrack(|| host.child_dock(child))?;
            let nodes = declaration.nest().ok()?;
            let path = nodes
                .panes()
                .into_iter()
                .find(|pane| pane.key == key)?
                .empty
                .clone()?;
            let shown = Some((child, path));
            Some(view! {
                <DockTabBody editor shown />
            })
        });
        link.place(DockBodies {
            scope,
            body,
            empty,
            hosts: create_memo(Vec::new),
        });
    }
    on_cleanup(move || {
        if let Some(child) = last.get() {
            host.host_child_dock(child, None);
        }
        link.withdraw();
    });
}

#[component]
fn DockTabBody(editor: Editor, shown: Option<(ChildId, DockTabPath)>) -> NodeId {
    let placed = shown.map(|(child, path)| crate::SubregionContent::DockTab(path.through(child)));
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
}

fn tab_menu(
    editor: Editor,
    host: EditorHost,
    pushed: ReadSignal<u64>,
    child: Option<ChildId>,
    path: Option<DockTabPath>,
) -> Memo<Vec<MenuNode>> {
    let nodes = create_signal(Vec::<MenuNode>::new());
    let picks = path.clone().map(MenuPicks::DockTab);
    let mirror = RefCell::new(MenuMirror::picking(
        editor,
        picks.unwrap_or(MenuPicks::Child),
    ));
    let shown = child.zip(path);
    create_effect(move || {
        pushed.get();
        let Some((child, path)) = shown.clone() else {
            return;
        };
        let entries = untrack(|| host.child_dock(child))
            .and_then(|declaration| declaration.tab(&path).map(|tab| tab.menu.clone()))
            .and_then(|menu| api::MenuEntry::nest(menu).ok())
            .unwrap_or_default();
        let updated = mirror.borrow_mut().update(Some(child), &entries);
        if let Some(updated) = updated {
            untrack(|| nodes.set(updated));
        }
    });
    create_memo(move || nodes.get())
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
                place: rekeyed_place(&home.place, key),
            })
            .collect(),
        unlocked: plain
            .unlocked
            .iter()
            .map(|place| rekeyed_place(place, key))
            .collect(),
    }
}

fn rekeyed_place(place: &DockPlainPlace, key: &dyn Fn(&str) -> String) -> DockPlainPlace {
    match place {
        DockPlainPlace::Area(area) => DockPlainPlace::Area(key(area)),
        DockPlainPlace::Group(group) => DockPlainPlace::Group(key(group)),
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

fn spec_panes(spec: &DockSpec) -> Vec<&DockSpecPane> {
    fn walk<'a>(node: &'a DockSpecNode, out: &mut Vec<&'a DockSpecPane>) {
        match node {
            DockSpecNode::Split { first, second, .. } => {
                walk(first, out);
                walk(second, out);
            }
            DockSpecNode::Pane(pane) => {
                out.push(pane);
                for entry in &pane.entries {
                    if let DockSpecEntry::Group { root, .. } = entry {
                        walk(root, out);
                    }
                }
            }
        }
    }
    let mut panes = Vec::new();
    for root in spec.main.iter().chain(
        spec.windows
            .iter()
            .filter_map(|window| window.root.as_ref()),
    ) {
        walk(root, &mut panes);
    }
    panes
}

type EmptyOf<'a> = &'a dyn Fn(&DockSpecPane) -> Option<DockTabPath>;

fn spec_node(node: &DockSpecNode, path: PathOf<'_>, empty: EmptyOf<'_>) -> api::DockNode {
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
            first: Box::new(spec_node(first, path, empty)),
            second: Box::new(spec_node(second, path, empty)),
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
                            root: Box::new(spec_node(root, path, empty)),
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
                    empty: empty(pane),
                },
                entries: entries.into_iter().map(|(_, entry)| entry).collect(),
            }
        }
    }
}

pub(crate) fn declaration_to_api(
    declaration: &DockDeclaration,
    path: PathOf<'_>,
    menu: &dyn Fn(TabId) -> Vec<api::MenuItem>,
    empty: EmptyOf<'_>,
) -> api::DockDeclaration {
    let spec = &declaration.spec;
    let nodes = api::DockNodes {
        main: spec.main.as_ref().map(|main| spec_node(main, path, empty)),
        windows: spec
            .windows
            .iter()
            .map(|window| api::DockFloating {
                window: api::DockWindow {
                    key: window.key.clone(),
                    rect: window.rect,
                },
                root: window
                    .root
                    .as_ref()
                    .map(|root| spec_node(root, path, empty)),
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
                    menu: menu(tab.tab),
                })
            })
            .collect(),
        dialogs: declaration
            .dialogs
            .iter()
            .filter_map(|dialog| {
                Some(api::DockDialog {
                    tab: path(dialog.tab)?,
                    parent: path(dialog.parent)?,
                    title: dialog.title.clone(),
                    glyph: dialog
                        .icon
                        .map(|icon| icon.as_str().to_owned())
                        .unwrap_or_default(),
                    size: api::Size {
                        width: dialog.size.x,
                        height: dialog.size.y,
                    },
                    modal: dialog.modal,
                    card: dialog.presentation == DockDialogPresentation::Card,
                    dismiss_on_outside_press: dialog.dismiss_on_outside_press,
                    anchor: dialog.anchor,
                    closable: dialog.closable,
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
        see_through: Vec::new(),
    };
    let tabs = declaration
        .tabs
        .iter()
        .map(|tab| DockDeclaredTab {
            tab: paths.tab(&tab.tab),
            key: tab.key.clone(),
            title: tab.title.clone(),
            icon: Glyph::from_codepoint(&tab.glyph),
            picture: None,
            closable: tab.closable,
        })
        .collect();
    let dialogs = declaration
        .dialogs
        .iter()
        .map(|dialog| DockDeclaredDialog {
            tab: paths.tab(&dialog.tab),
            parent: paths.tab(&dialog.parent),
            title: dialog.title.clone(),
            icon: Glyph::from_codepoint(&dialog.glyph),
            size: beui::vec2(dialog.size.width, dialog.size.height),
            modal: dialog.modal,
            presentation: match dialog.card {
                true => DockDialogPresentation::Card,
                false => DockDialogPresentation::Window,
            },
            dismiss_on_outside_press: dialog.dismiss_on_outside_press,
            anchor: dialog.anchor,
            closable: dialog.closable,
        })
        .collect();
    Some(DockDeclaration {
        spec,
        tabs,
        dialogs,
    })
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
                    empty: None,
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
                    place: api_place(&home.place),
                })
            })
            .collect(),
        unlocked: plain.unlocked.iter().map(api_place).collect(),
    }
}

fn api_place(place: &DockPlainPlace) -> api::DockPlace {
    match place {
        DockPlainPlace::Area(area) => api::DockPlace::Area(area.clone()),
        DockPlainPlace::Group(group) => api::DockPlace::Group(group.clone()),
    }
}

fn plain_place(place: &api::DockPlace) -> DockPlainPlace {
    match place {
        api::DockPlace::Area(area) => DockPlainPlace::Area(area.clone()),
        api::DockPlace::Group(group) => DockPlainPlace::Group(group.clone()),
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
                    place: plain_place(&home.place),
                })
            })
            .collect(),
        unlocked: layout.unlocked.iter().map(plain_place).collect(),
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
