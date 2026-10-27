use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use beui_core::geometry::Rect;
use beui_core::icons::Glyph;
use beui_core::node::NodeId;
use beui_view::reactive::*;

use super::super::DockMode;
use super::super::state::{
    DockFullscreen, DockMapping, DockSpec, DockState, TabId, joined_host, joined_key, joined_prefix,
};

#[derive(Clone, Debug, PartialEq)]
pub struct DockDeclaration {
    pub spec: DockSpec,
    pub tabs: Vec<DockDeclaredTab>,
}

impl DockDeclaration {
    pub fn tab(&self, tab: TabId) -> Option<&DockDeclaredTab> {
        self.tabs.iter().find(|declared| declared.tab == tab)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DockDeclaredTab {
    pub tab: TabId,
    pub key: String,
    pub title: String,
    pub icon: Option<Glyph>,
    pub closable: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DockHosted {
    pub state: DockState,
    pub tabs: Vec<TabId>,
    pub focused: Option<TabId>,
    pub shown: Option<TabId>,
    pub visible: Vec<TabId>,
    pub fullscreen: Option<TabId>,
    pub mode: DockMode,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DockRequest {
    Show(TabId),
    Restore(DockState),
    Fullscreen(Option<(TabId, Option<Rect>)>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DockHostEvent {
    Close(TabId),
}

#[derive(Clone)]
pub struct DockBodies {
    pub scope: ScopeContext,
    pub body: RenderFn<TabId, NodeId>,
    pub empty: RenderFn<String, Option<NodeId>>,
}

#[derive(Clone)]
pub struct DockHostSlot {
    link: Rc<dyn DockHostLink>,
    claimed: Rc<Cell<bool>>,
}

impl DockHostSlot {
    pub fn new(link: Rc<dyn DockHostLink>) -> Self {
        Self {
            link,
            claimed: Rc::default(),
        }
    }

    pub fn claim(&self) -> Option<Rc<dyn DockHostLink>> {
        if self.claimed.replace(true) {
            return None;
        }
        let claimed = Rc::clone(&self.claimed);
        on_cleanup(move || claimed.set(false));
        Some(Rc::clone(&self.link))
    }
}

pub trait DockHostLink {
    fn declare(&self, declaration: DockDeclaration);
    fn hosted(&self) -> Memo<Option<DockHosted>>;
    fn request(&self, request: DockRequest);
    fn listen(&self, events: Callback<DockHostEvent>);
    fn place(&self, bodies: DockBodies);
    fn withdraw(&self);
}

#[derive(Default)]
pub(crate) struct Joined {
    pub(crate) declaration: RefCell<Option<DockDeclaration>>,
    pub(crate) bodies: RefCell<Option<DockBodies>>,
    pub(crate) events: RefCell<Option<Callback<DockHostEvent>>>,
    pub(crate) pending: RefCell<Option<DockState>>,
}

impl Joined {
    pub(crate) fn close(&self, tab: TabId) {
        let events = *self.events.borrow();
        if let Some(events) = events {
            events.call(DockHostEvent::Close(tab));
        }
    }
}

#[derive(Default)]
pub(crate) struct Foreign {
    outer: HashMap<(TabId, TabId), TabId>,
    inner: HashMap<TabId, (TabId, TabId)>,
}

impl Foreign {
    pub(crate) fn outer(&self, host: TabId, inner: TabId) -> Option<TabId> {
        self.outer.get(&(host, inner)).copied()
    }

    pub(crate) fn inner(&self, outer: TabId) -> Option<(TabId, TabId)> {
        self.inner.get(&outer).copied()
    }

    pub(crate) fn contains(&self, outer: TabId) -> bool {
        self.inner.contains_key(&outer)
    }

    pub(crate) fn newest(&self) -> u64 {
        self.inner.keys().map(|tab| tab.value()).max().unwrap_or(0)
    }

    pub(crate) fn insert(&mut self, host: TabId, inner: TabId, outer: TabId) {
        self.outer.insert((host, inner), outer);
        self.inner.insert(outer, (host, inner));
    }

    pub(crate) fn retain(&mut self, kept: impl Fn(TabId) -> bool) {
        self.inner.retain(|outer, _| kept(*outer));
        self.outer.retain(|_, outer| kept(*outer));
    }
}

pub(crate) struct Registry {
    pub(crate) version: Signal<u64>,
    pub(crate) joins: RefCell<HashMap<TabId, Rc<Joined>>>,
    pub(crate) foreign: RefCell<Foreign>,
    pub(crate) state: Signal<DockState>,
    pub(crate) mode: Signal<DockMode>,
}

impl Registry {
    pub(crate) fn bump(&self) {
        if self.version.is_alive() {
            self.version.update(|version| *version += 1);
        }
    }

    pub(crate) fn joined(&self, host: TabId) -> Option<Rc<Joined>> {
        self.joins.borrow().get(&host).cloned()
    }

    pub(crate) fn graft(&self, state: &mut DockState, host: TabId, from: &DockState) -> bool {
        let Some(group) = state.find_group(&joined_key(host)) else {
            return false;
        };
        let foreign = self.foreign.borrow();
        let prefix = joined_prefix(host);
        state.graft(
            group,
            from,
            &DockMapping {
                tab: &|inner| foreign.outer(host, inner),
                key: &|key| Some(format!("{prefix}{key}")),
                group: &|_| None,
            },
        );
        true
    }

    fn hosted(&self, host: TabId) -> Option<DockHosted> {
        let foreign = self.foreign.borrow();
        let inner = |outer: TabId| {
            foreign
                .inner(outer)
                .filter(|(held, _)| *held == host)
                .map(|(_, inner)| inner)
        };
        let mode = self.mode.get();
        self.state.with(|state| {
            let group = state.find_group(&joined_key(host))?;
            let prefix = joined_prefix(host);
            let subtree = state.subtree(
                group,
                &DockMapping {
                    tab: &inner,
                    key: &|key| key.strip_prefix(prefix.as_str()).map(str::to_owned),
                    group: &|key| joined_host(key).and_then(inner),
                },
            );
            let stacked = mode == DockMode::Stacked;
            let held = state.all_tabs();
            Some(DockHosted {
                state: subtree,
                tabs: held.iter().copied().filter_map(inner).collect(),
                focused: state.focused_tab().and_then(inner),
                shown: state.stacked_tab().and_then(inner),
                visible: held
                    .iter()
                    .copied()
                    .filter(|tab| state.is_shown(*tab, stacked))
                    .filter_map(inner)
                    .collect(),
                fullscreen: state.fullscreen().and_then(|shown| inner(shown.tab)),
                mode,
            })
        })
    }
}

#[derive(Clone)]
pub struct DockJoin {
    pub(crate) host: TabId,
    pub(crate) registry: Rc<Registry>,
    pub(crate) claimed: Rc<Cell<bool>>,
}

impl DockJoin {
    pub fn host(&self) -> TabId {
        self.host
    }

    pub fn claim(&self) -> Option<Rc<dyn DockHostLink>> {
        if self.claimed.replace(true) {
            return None;
        }
        Some(Rc::new(JoinedLink {
            join: self.clone(),
            joined: Rc::new(Joined::default()),
        }))
    }
}

pub fn use_dock_join() -> Option<DockJoin> {
    use_context::<DockJoin>()
}

struct JoinedLink {
    join: DockJoin,
    joined: Rc<Joined>,
}

impl JoinedLink {
    fn outer(&self, inner: TabId) -> Option<TabId> {
        self.join
            .registry
            .foreign
            .borrow()
            .outer(self.join.host, inner)
    }

    fn edit(&self, change: impl FnOnce(&mut DockState)) {
        let state = self.join.registry.state;
        if !state.is_alive() {
            return;
        }
        let current = state.get_untracked();
        let mut next = current.clone();
        change(&mut next);
        if next != current {
            state.set(next);
        }
    }
}

impl DockHostLink for JoinedLink {
    fn declare(&self, declaration: DockDeclaration) {
        let registry = &self.join.registry;
        *self.joined.declaration.borrow_mut() = Some(declaration);
        registry
            .joins
            .borrow_mut()
            .entry(self.join.host)
            .or_insert_with(|| Rc::clone(&self.joined));
        registry.bump();
    }

    fn hosted(&self) -> Memo<Option<DockHosted>> {
        let registry = Rc::clone(&self.join.registry);
        let host = self.join.host;
        let version = registry.version;
        create_memo(move || {
            if !version.is_alive() || !registry.state.is_alive() || !registry.mode.is_alive() {
                return None;
            }
            version.get();
            registry.hosted(host)
        })
    }

    fn request(&self, request: DockRequest) {
        match request {
            DockRequest::Show(tab) => {
                if let Some(tab) = self.outer(tab) {
                    self.edit(|state| state.show(tab));
                }
            }
            DockRequest::Fullscreen(fullscreen) => {
                let fullscreen = match fullscreen {
                    Some((tab, area)) => match self.outer(tab) {
                        Some(tab) => Some(DockFullscreen { tab, area }),
                        None => return,
                    },
                    None => None,
                };
                self.edit(|state| match fullscreen {
                    Some(fullscreen) => state.set_fullscreen(Some(fullscreen)),
                    None => {
                        let ours = state.fullscreen().is_some_and(|shown| {
                            self.join
                                .registry
                                .foreign
                                .borrow()
                                .inner(shown.tab)
                                .map(|(host, _)| host)
                                == Some(self.join.host)
                        });
                        if ours {
                            state.set_fullscreen(None);
                        }
                    }
                });
            }
            DockRequest::Restore(from) => {
                let host = self.join.host;
                let registry = Rc::clone(&self.join.registry);
                let mut grafted = false;
                self.edit(|state| grafted = registry.graft(state, host, &from));
                if !grafted {
                    *self.joined.pending.borrow_mut() = Some(from);
                    registry.bump();
                }
            }
        }
    }

    fn listen(&self, events: Callback<DockHostEvent>) {
        *self.joined.events.borrow_mut() = Some(events);
    }

    fn place(&self, bodies: DockBodies) {
        *self.joined.bodies.borrow_mut() = Some(bodies);
    }

    fn withdraw(&self) {
        let registry = &self.join.registry;
        let host = self.join.host;
        {
            let mut joins = registry.joins.borrow_mut();
            if joins
                .get(&host)
                .is_some_and(|held| Rc::ptr_eq(held, &self.joined))
            {
                joins.remove(&host);
            }
        }
        *self.joined.events.borrow_mut() = None;
        *self.joined.bodies.borrow_mut() = None;
        self.join.claimed.set(false);
        registry.bump();
    }
}
