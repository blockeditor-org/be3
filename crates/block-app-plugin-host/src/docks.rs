use stable_hash::{HashMap, HashSet};
use std::{cell::RefCell, collections::BTreeMap};

use block_plugin_api::{
    BarAction, ChildId, DockBarPlacement, DockChild, DockChildRequest, DockClosedTab,
    DockDeclaration, DockDeclaredChild, DockHold, DockHosted, DockHostedView, DockMessage,
    DockPlacedBar, DockRequest, DockTabPath, DockTree, DockUpdate, EditorInstanceId,
    MAX_DOCK_NESTING, MenuItem, ViewId, dock_prefix,
};
use uuid::Uuid;

pub(super) type Pick = (
    EditorInstanceId,
    ViewId,
    String,
    Option<block_plugin_api::Rect>,
);

thread_local! {
    static DOCKS: RefCell<Docks> = RefCell::new(Docks::default());
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct DockedChild {
    pub(super) child: ChildId,
    pub(super) tab: u64,
    pub(super) block: Uuid,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Placed {
    tab: u64,
    block: Uuid,
    placed: bool,
    frame: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Hosting {
    Awaited,
    Hosted,
    Placing,
}

#[derive(Default)]
pub(super) struct Docks {
    plugins: HashMap<EditorInstanceId, String>,
    blocks: HashMap<Uuid, EditorInstanceId>,
    children: HashMap<EditorInstanceId, BTreeMap<ChildId, Placed>>,
    declared: HashMap<EditorInstanceId, DockDeclaration>,
    hosting: HashMap<(EditorInstanceId, ChildId), Hosting>,
    delivered: HashMap<(EditorInstanceId, ChildId), DockDeclaration>,
    hosted: HashMap<EditorInstanceId, DockHosted>,
    docked: HashSet<EditorInstanceId>,
    offscreen: HashSet<EditorInstanceId>,
    framed: HashSet<EditorInstanceId>,
    parked: HashMap<(EditorInstanceId, u64), (DockBarPlacement, bool)>,
    shown_bars: HashSet<(EditorInstanceId, u64)>,
    outbox: Vec<(EditorInstanceId, DockUpdate)>,
    bars: Vec<(EditorInstanceId, ViewId, ChildId, BarAction)>,
    menus: HashMap<(EditorInstanceId, ViewId), Vec<MenuItem>>,
    picks: Vec<Pick>,
    changed: HashSet<String>,
}

impl Docks {
    pub(super) fn register(
        &mut self,
        instance: EditorInstanceId,
        plugin: &str,
        block: Option<Uuid>,
    ) {
        self.plugins.insert(instance, plugin.to_owned());
        if let Some(block) = block
            && self.blocks.insert(block, instance) != Some(instance)
        {
            self.refresh();
        }
    }

    pub(super) fn forget(&mut self, instance: EditorInstanceId) {
        self.menus.retain(|(held, _), _| *held != instance);
        self.picks.retain(|(held, _, _, _)| *held != instance);
        self.blocks.retain(|_, held| *held != instance);
        self.children.remove(&instance);
        self.offscreen.remove(&instance);
        self.declared.remove(&instance);
        self.hosted.remove(&instance);
        self.hosting.retain(|(root, _), _| *root != instance);
        self.delivered.retain(|(root, _), _| *root != instance);
        self.outbox.retain(|(to, _)| *to != instance);
        self.parked.retain(|(held, _), _| *held != instance);
        self.shown_bars.retain(|(held, _)| *held != instance);
        self.refresh();
        self.plugins.remove(&instance);
    }

    pub(super) fn restarted(&mut self, plugin: &str) {
        let gone: Vec<EditorInstanceId> = self
            .plugins
            .iter()
            .filter(|(_, held)| *held == plugin)
            .map(|(instance, _)| *instance)
            .collect();
        for instance in gone {
            self.children.remove(&instance);
            self.declared.remove(&instance);
            self.hosted.remove(&instance);
            self.hosting.retain(|(root, _), _| *root != instance);
            self.delivered.retain(|(root, _), _| *root != instance);
            self.outbox.retain(|(to, _)| *to != instance);
        }
        self.refresh();
    }

    pub(super) fn placed(
        &mut self,
        placer: EditorInstanceId,
        placed: &[DockedChild],
        framed: Option<DockedChild>,
    ) {
        let mut changed = self.offscreen.remove(&placer);
        let held = self.children.entry(placer).or_default();
        let before = held.len();
        held.retain(|child, record| {
            !record.frame || framed.is_some_and(|framed| framed.child == *child)
        });
        changed |= held.len() != before;
        for (child, record) in held.iter_mut().filter(|(_, record)| !record.frame) {
            let now = placed.iter().any(|docked| docked.child == *child);
            changed |= record.placed != now;
            record.placed = now;
            let key = (placer, *child);
            if now && self.hosting.get(&key) == Some(&Hosting::Placing) {
                self.hosting.remove(&key);
                changed = true;
            }
        }
        for docked in placed {
            let record = Placed {
                tab: docked.tab,
                block: docked.block,
                placed: true,
                frame: false,
            };
            if held.insert(docked.child, record) != Some(record) {
                changed = true;
            }
        }
        if let Some(framed) = framed {
            let record = Placed {
                tab: framed.tab,
                block: framed.block,
                placed: true,
                frame: true,
            };
            if held.insert(framed.child, record) != Some(record) {
                changed = true;
            }
        }
        if held.is_empty() {
            self.children.remove(&placer);
        }
        if changed {
            self.refresh();
        }
    }

    pub(super) fn offscreen(&mut self, placer: EditorInstanceId) {
        if self.offscreen.insert(placer) {
            self.refresh();
        }
    }

    pub(super) fn heard(&mut self, plugin: &str, messages: Vec<DockMessage>) {
        if messages.is_empty() {
            return;
        }
        for message in messages {
            match message {
                DockMessage::Declare(declare) => {
                    let instance = declare.view.instance;
                    if declare.view.view != ViewId::MAIN || !self.owns(plugin, instance) {
                        continue;
                    }
                    match declare.declaration {
                        Some(declaration) => self.declared.insert(instance, declaration),
                        None => self.declared.remove(&instance),
                    };
                    self.refresh();
                }
                DockMessage::Request(request) => {
                    let instance = request.view.instance;
                    if request.view.view == ViewId::MAIN && self.owns(plugin, instance) {
                        self.request(instance, request.request);
                    }
                }
                DockMessage::Host(host) => {
                    if self.owns(plugin, host.child.instance) {
                        self.host(host.child, host.hold);
                    }
                }
                DockMessage::Close(close) => {
                    if self.owns(plugin, close.child.instance)
                        && let Some((owner, tab)) = self.owner(close.child, close.tab)
                    {
                        self.send(
                            owner,
                            DockUpdate::Close(DockClosedTab {
                                view: ViewId::MAIN,
                                tab,
                            }),
                        );
                    }
                }
                DockMessage::Pick(pick) => {
                    if self.owns(plugin, pick.child.instance)
                        && let Some((owner, tab)) = self.owner(pick.child, pick.tab)
                    {
                        if let Some(plugin) = self.plugins.get(&owner) {
                            self.changed.insert(plugin.clone());
                        }
                        self.picks
                            .push((owner, ViewId(tab.tab), pick.id, pick.anchor));
                    }
                }
            }
        }
    }

    pub(super) fn docked(&self, instance: EditorInstanceId) -> bool {
        self.docked.contains(&instance)
    }

    pub(super) fn resolve(
        &self,
        placer: EditorInstanceId,
        path: &DockTabPath,
    ) -> Option<(Uuid, u64)> {
        if path.via.is_empty() || self.docked(placer) {
            return None;
        }
        let mut owner = placer;
        let mut block = None;
        for child in &path.via {
            let record = self.children.get(&owner)?.get(child)?;
            owner = self.instance_of(owner, record)?;
            block = Some(record.block);
        }
        let own = DockTabPath::own(path.tab);
        let shown = self
            .hosted
            .get(&owner)
            .is_some_and(|hosted| hosted.visible.contains(&own))
            || self
                .declared
                .get(&owner)
                .is_some_and(|declaration| declaration.shows_body(&own));
        shown.then_some((block?, path.tab))
    }

    pub(super) fn take(&mut self, plugin: &str) -> Vec<(EditorInstanceId, DockUpdate)> {
        let plugins = &self.plugins;
        let (taken, kept) = std::mem::take(&mut self.outbox)
            .into_iter()
            .partition(|(to, _)| plugins.get(to).is_some_and(|held| held == plugin));
        self.outbox = kept;
        taken
    }

    pub(super) fn bar(&mut self, instance: EditorInstanceId, action: BarAction) -> bool {
        if !self.docked(instance) || !self.declared.contains_key(&instance) {
            return false;
        }
        let Some((placer, child, record)) = self.placer_of(instance) else {
            return false;
        };
        if let Some(plugin) = self.plugins.get(&placer) {
            self.changed.insert(plugin.clone());
        }
        let view = match self.docked(placer) {
            true => ViewId(record.tab),
            false => ViewId::MAIN,
        };
        self.bars.push((placer, view, child, action));
        true
    }

    pub(super) fn take_bars(
        &mut self,
        plugin: &str,
    ) -> Vec<(EditorInstanceId, ViewId, ChildId, BarAction)> {
        let plugins = &self.plugins;
        let (taken, kept) = std::mem::take(&mut self.bars)
            .into_iter()
            .partition(|(to, _, _, _)| plugins.get(to).is_some_and(|held| held == plugin));
        self.bars = kept;
        taken
    }

    pub(super) fn set_menu(
        &mut self,
        instance: EditorInstanceId,
        view: ViewId,
        items: Vec<MenuItem>,
    ) {
        let key = (instance, view);
        if self
            .menus
            .get(&key)
            .map_or(items.is_empty(), |held| *held == items)
        {
            return;
        }
        match items.is_empty() {
            true => self.menus.remove(&key),
            false => self.menus.insert(key, items),
        };
        if self.declared.contains_key(&instance) {
            self.refresh();
        }
    }

    pub(super) fn take_picks(&mut self, plugin: &str) -> Vec<Pick> {
        let plugins = &self.plugins;
        let (taken, kept) = std::mem::take(&mut self.picks)
            .into_iter()
            .partition(|(to, _, _, _)| plugins.get(to).is_some_and(|held| held == plugin));
        self.picks = kept;
        taken
    }

    pub(super) fn take_changed(&mut self) -> Vec<String> {
        self.changed.drain().collect()
    }

    fn owns(&self, plugin: &str, instance: EditorInstanceId) -> bool {
        self.plugins
            .get(&instance)
            .is_some_and(|held| held == plugin)
    }

    fn instance_of(&self, placer: EditorInstanceId, record: &Placed) -> Option<EditorInstanceId> {
        self.blocks
            .get(&record.block)
            .copied()
            .filter(|instance| *instance != placer)
    }

    fn placer_of(&self, instance: EditorInstanceId) -> Option<(EditorInstanceId, ChildId, Placed)> {
        self.children.iter().find_map(|(placer, children)| {
            children.iter().find_map(|(child, record)| {
                (self.instance_of(*placer, record) == Some(instance))
                    .then_some((*placer, *child, *record))
            })
        })
    }

    fn prefix(&self, placer: EditorInstanceId, record: &Placed) -> Option<String> {
        if record.frame && record.tab == ViewId::MAIN.0 {
            return Some(dock_prefix("frame", record.block.into_bytes()));
        }
        let declaration = self.declared.get(&placer)?;
        let tab = declaration.tab(&DockTabPath::own(record.tab))?;
        Some(dock_prefix(&tab.key, record.block.into_bytes()))
    }

    fn send(&mut self, to: EditorInstanceId, update: DockUpdate) {
        if let Some(plugin) = self.plugins.get(&to) {
            self.changed.insert(plugin.clone());
        }
        self.outbox.push((to, update));
    }

    fn docked_set(&self) -> HashSet<EditorInstanceId> {
        self.children
            .iter()
            .flat_map(|(placer, children)| {
                children
                    .values()
                    .filter(|record| !record.frame)
                    .filter_map(|record| self.instance_of(*placer, record))
            })
            .collect()
    }

    fn framed_set(&self, docked: &HashSet<EditorInstanceId>) -> HashSet<EditorInstanceId> {
        let mut framed = HashSet::default();
        for _ in 0..=MAX_DOCK_NESTING {
            let next: HashSet<EditorInstanceId> = self
                .children
                .iter()
                .filter(|(placer, _)| docked.contains(placer) || framed.contains(*placer))
                .flat_map(|(placer, children)| {
                    let within = framed.contains(placer);
                    children
                        .values()
                        .filter(move |record| {
                            record.frame && (within || record.tab != ViewId::MAIN.0)
                        })
                        .filter_map(|record| self.instance_of(*placer, record))
                })
                .filter(|instance| !docked.contains(instance))
                .collect();
            if next == framed {
                break;
            }
            framed = next;
        }
        framed
    }

    fn frame_record(&self, instance: EditorInstanceId) -> Option<(ChildId, Placed)> {
        if !self.joins(instance) {
            return None;
        }
        let framed = self.framed.contains(&instance);
        self.children
            .get(&instance)?
            .iter()
            .find(|(_, record)| record.frame && (framed || record.tab != ViewId::MAIN.0))
            .map(|(child, record)| (*child, *record))
    }

    fn joins(&self, instance: EditorInstanceId) -> bool {
        self.docked.contains(&instance) || self.framed.contains(&instance)
    }

    fn alive(
        &self,
        docked: &HashSet<EditorInstanceId>,
        placer: EditorInstanceId,
        child: ChildId,
        record: &Placed,
    ) -> bool {
        if record.placed || self.offscreen.contains(&placer) {
            return true;
        }
        match docked.contains(&placer) {
            true => {
                self.instance_of(placer, record)
                    .is_some_and(|instance| self.declared.contains_key(&instance))
                    && self.declared.get(&placer).is_some_and(|declaration| {
                        declaration.tab(&DockTabPath::own(record.tab)).is_some()
                    })
            }
            false => self.hosting.contains_key(&(placer, child)),
        }
    }

    fn refresh(&mut self) {
        let mut docked = self.docked_set();
        loop {
            let dead: Vec<(EditorInstanceId, ChildId)> = self
                .children
                .iter()
                .flat_map(|(placer, children)| {
                    children
                        .iter()
                        .filter(|(child, record)| !self.alive(&docked, *placer, **child, record))
                        .map(|(child, _)| (*placer, *child))
                })
                .collect();
            if dead.is_empty() {
                break;
            }
            for (placer, child) in dead {
                if let Some(children) = self.children.get_mut(&placer) {
                    children.remove(&child);
                    if children.is_empty() {
                        self.children.remove(&placer);
                    }
                }
                self.hosting.remove(&(placer, child));
            }
            docked = self.docked_set();
        }
        let framed = self.framed_set(&docked);
        for instance in docked
            .symmetric_difference(&self.docked)
            .chain(framed.symmetric_difference(&self.framed))
        {
            if let Some(plugin) = self.plugins.get(instance) {
                self.changed.insert(plugin.clone());
            }
        }
        self.docked = docked;
        self.framed = framed;
        let released: Vec<EditorInstanceId> = self
            .hosted
            .keys()
            .filter(|instance| !self.joins(**instance))
            .copied()
            .collect();
        for instance in released {
            self.hosted.remove(&instance);
            self.send(
                instance,
                DockUpdate::Hosted(DockHostedView {
                    view: ViewId::MAIN,
                    hosted: None,
                }),
            );
        }
        let mut wanted: HashMap<(EditorInstanceId, ChildId), DockDeclaration> = HashMap::default();
        for (placer, children) in &self.children {
            if self.joins(*placer) {
                continue;
            }
            for (child, record) in children.iter().filter(|(_, record)| !record.frame) {
                if let Some(spliced) = self
                    .instance_of(*placer, record)
                    .and_then(|instance| self.spliced(instance, 0))
                {
                    wanted.insert((*placer, *child), spliced);
                }
            }
        }
        let withdrawn: Vec<(EditorInstanceId, ChildId)> = self
            .delivered
            .keys()
            .filter(|key| !wanted.contains_key(key))
            .copied()
            .collect();
        for (root, child) in withdrawn {
            self.delivered.remove(&(root, child));
            if self.hosting.get(&(root, child)) == Some(&Hosting::Awaited) {
                self.hosting.remove(&(root, child));
            }
            self.send(
                root,
                DockUpdate::Child(DockDeclaredChild {
                    child,
                    declaration: None,
                }),
            );
        }
        let mut wanted: Vec<_> = wanted.into_iter().collect();
        wanted.sort_by_key(|((root, child), _)| (*root, *child));
        for ((root, child), declaration) in wanted {
            if self.delivered.get(&(root, child)) == Some(&declaration) {
                continue;
            }
            self.delivered.insert((root, child), declaration.clone());
            let hosting = self
                .hosting
                .entry((root, child))
                .or_insert(Hosting::Awaited);
            if *hosting == Hosting::Placing {
                *hosting = Hosting::Awaited;
            }
            self.send(
                root,
                DockUpdate::Child(DockDeclaredChild {
                    child,
                    declaration: Some(declaration),
                }),
            );
        }
    }

    fn spliced(&self, instance: EditorInstanceId, depth: usize) -> Option<DockDeclaration> {
        if depth > MAX_DOCK_NESTING {
            return None;
        }
        let mut declaration = self.declared.get(&instance)?.clone();
        for tab in &mut declaration.tabs {
            if tab.tab.via.is_empty() {
                tab.menu = self
                    .menus
                    .get(&(instance, ViewId(tab.tab.tab)))
                    .cloned()
                    .unwrap_or_default();
            }
        }
        for bar in &mut declaration.bars {
            if let Some(placement) = self.returning(instance, &bar.tab) {
                bar.placement = placement;
            }
        }
        let kept: HashMap<String, DockBarPlacement> = declaration
            .bars
            .iter()
            .filter(|bar| bar.tab.is_own())
            .map(|bar| {
                let placement = self.parked.get(&(instance, bar.tab.tab));
                (
                    bar.key.clone(),
                    placement.map_or_else(|| bar.placement.clone(), |(held, _)| held.clone()),
                )
            })
            .collect();
        for (child, record) in self.children.get(&instance).into_iter().flatten() {
            if record.frame {
                continue;
            }
            let Some(inner) = self
                .instance_of(instance, record)
                .and_then(|inner| self.spliced(inner, depth + 1))
            else {
                continue;
            };
            let Some(prefix) = self.prefix(instance, record) else {
                continue;
            };
            let _ = declaration.splice(record.tab, *child, &prefix, &inner);
        }
        if let Some((child, record)) = self.frame_record(instance)
            && let Some(prefix) = self.prefix(instance, &record)
        {
            let framer = self.instance_of(instance, &record);
            let mut inner = framer
                .and_then(|inner| self.spliced(inner, depth + 1))
                .unwrap_or_else(empty_declaration);
            if let Some(framer) = framer {
                for bar in &mut inner.bars {
                    let entering =
                        bar.tab.is_own() && !self.shown_bars.contains(&(framer, bar.tab.tab));
                    if let Some(placement) = kept.get(&bar.key).filter(|_| entering) {
                        bar.placement = placement.clone();
                    }
                }
            }
            let host = (record.tab != ViewId::MAIN.0).then_some(record.tab);
            declaration.splice_bars(host, child, &prefix, &inner);
        }
        Some(declaration)
    }

    fn returning(&self, instance: EditorInstanceId, bar: &DockTabPath) -> Option<DockBarPlacement> {
        if !bar.is_own() || self.shown_bars.contains(&(instance, bar.tab)) {
            return None;
        }
        self.parked
            .get(&(instance, bar.tab))
            .map(|(placement, _)| placement.clone())
    }

    fn request(&mut self, asker: EditorInstanceId, request: DockRequest) {
        let mut request = request;
        let mut instance = asker;
        let mut owner = Vec::new();
        for _ in 0..=MAX_DOCK_NESTING {
            let Some((placer, child, record)) = self.placer_of(instance) else {
                return;
            };
            let bar = matches!(
                request,
                DockRequest::PlaceBar(_) | DockRequest::MoveBar(_) | DockRequest::HideBar(_)
            );
            if record.frame && !bar {
                return;
            }
            if !self.joins(placer) {
                owner.reverse();
                self.send(
                    placer,
                    DockUpdate::ChildRequest(DockChildRequest {
                        child,
                        owner,
                        request,
                    }),
                );
                return;
            }
            let Some(prefix) = self.prefix(placer, &record) else {
                return;
            };
            let Ok(through) = request.through(child, &prefix) else {
                return;
            };
            request = through;
            owner.push(child);
            instance = placer;
        }
    }

    fn host(&mut self, child: DockChild, hold: DockHold) {
        let DockChild {
            instance: root,
            child,
        } = child;
        if self.joins(root) {
            return;
        }
        let Some(record) = self
            .children
            .get(&root)
            .and_then(|children| children.get(&child))
            .copied()
        else {
            return;
        };
        let hosted = match hold {
            DockHold::Hosted(hosted) => {
                self.hosting.insert((root, child), Hosting::Hosted);
                Some(hosted)
            }
            DockHold::Placing if record.placed => {
                self.hosting.remove(&(root, child));
                None
            }
            DockHold::Placing => {
                self.hosting.insert((root, child), Hosting::Placing);
                None
            }
            DockHold::Released => {
                self.hosting.remove(&(root, child));
                None
            }
        };
        if let Some(instance) = self.instance_of(root, &record) {
            self.split(instance, hosted, 0);
        }
        self.refresh();
    }

    fn split(&mut self, instance: EditorInstanceId, hosted: Option<DockHosted>, depth: usize) {
        if depth > MAX_DOCK_NESTING || !self.joins(instance) {
            return;
        }
        let children: Vec<(ChildId, Placed)> = self
            .children
            .get(&instance)
            .map(|children| {
                children
                    .iter()
                    .map(|(child, record)| (*child, *record))
                    .collect()
            })
            .unwrap_or_default();
        let hosts: Vec<(ChildId, u64)> = children
            .iter()
            .filter(|(_, record)| !record.frame)
            .map(|(child, record)| (*child, record.tab))
            .collect();
        let mut own = hosted.as_ref().and_then(|hosted| hosted.own(&hosts).ok());
        if let Some(own) = &mut own {
            self.park(instance, own);
        }
        if self.hosted.get(&instance) != own.as_ref() {
            match &own {
                Some(own) => self.hosted.insert(instance, own.clone()),
                None => self.hosted.remove(&instance),
            };
            self.send(
                instance,
                DockUpdate::Hosted(DockHostedView {
                    view: ViewId::MAIN,
                    hosted: own,
                }),
            );
        }
        for (child, record) in children {
            let Some(inner) = self.instance_of(instance, &record) else {
                continue;
            };
            if record.frame && !self.framed.contains(&inner) {
                continue;
            }
            let declaration = hosted.as_ref().and_then(|_| self.spliced(inner, depth + 1));
            let part = match (&hosted, record.frame) {
                (Some(hosted), true) => {
                    let host = (record.tab != ViewId::MAIN.0).then_some(record.tab);
                    Some(hosted.frame_child(child, host, &|bar| {
                        declaration.as_ref()?.bar(bar).map(|bar| bar.owner.clone())
                    }))
                }
                (Some(hosted), false) => self
                    .prefix(instance, &record)
                    .and_then(|prefix| hosted.child(child, &prefix).ok().flatten())
                    .map(|mut part| {
                        if let Some(declaration) = &declaration {
                            part.own_dock_bars(declaration);
                        }
                        part
                    }),
                (None, _) => None,
            };
            self.split(inner, part, depth + 1);
        }
    }

    fn park(&mut self, instance: EditorInstanceId, own: &mut DockHosted) {
        self.shown_bars.retain(|(held, _)| *held != instance);
        for bar in &own.layout.bars {
            self.shown_bars.insert((instance, bar.bar.tab));
            self.parked.insert(
                (instance, bar.bar.tab),
                (bar.placement.clone(), bar.hidden),
            );
        }
        let Some((_, record)) = self.frame_record(instance) else {
            return;
        };
        let Some(declaration) = self.declared.get(&instance) else {
            return;
        };
        let taken = (record.tab != ViewId::MAIN.0).then(|| DockTabPath::own(record.tab));
        let hidden: Vec<DockPlacedBar> = declaration
            .bars
            .iter()
            .filter(|bar| bar.tab.is_own())
            .filter(|bar| {
                taken
                    .as_ref()
                    .is_none_or(|taken| bar.owner.as_ref() == Some(taken))
            })
            .filter(|bar| !self.shown_bars.contains(&(instance, bar.tab.tab)))
            .map(|bar| {
                let parked = self.parked.get(&(instance, bar.tab.tab));
                DockPlacedBar {
                    bar: bar.tab.clone(),
                    owner: bar.owner.clone(),
                    placement: parked
                        .map_or_else(|| bar.placement.clone(), |(placement, _)| placement.clone()),
                    hidden: parked.is_some_and(|(_, hidden)| *hidden),
                }
            })
            .collect();
        own.layout.bars.extend(hidden);
        if let Some(previous) = self.hosted.get(&instance) {
            let order = |bar: &DockPlacedBar| {
                previous
                    .layout
                    .bars
                    .iter()
                    .position(|held| held.bar == bar.bar)
                    .unwrap_or(usize::MAX)
            };
            own.layout.bars.sort_by_key(order);
        }
    }

    fn owner(&self, child: DockChild, tab: DockTabPath) -> Option<(EditorInstanceId, DockTabPath)> {
        let DockChild {
            instance: root,
            child,
        } = child;
        if self.joins(root) {
            return None;
        }
        let mut placer = root;
        let mut path = tab.through(child);
        while let Some(first) = path.via.first().copied() {
            let record = self.children.get(&placer)?.get(&first)?;
            let instance = self.instance_of(placer, record)?;
            let within = path.within(first)?;
            placer = instance;
            path = within;
        }
        (placer != root).then_some((placer, path))
    }
}

fn empty_declaration() -> DockDeclaration {
    DockDeclaration {
        tree: DockTree::flatten(&Default::default(), None),
        tabs: Vec::new(),
        dialogs: Vec::new(),
        bars: Vec::new(),
    }
}

fn with<R>(act: impl FnOnce(&mut Docks) -> R) -> R {
    let (result, changed) = DOCKS.with(|docks| {
        let mut docks = docks.borrow_mut();
        let result = act(&mut docks);
        (result, docks.take_changed())
    });
    if !changed.is_empty() {
        for plugin in changed {
            super::runtime::mark(&plugin);
        }
        block_app_base::host::request_repaint();
    }
    result
}

pub(super) fn register(instance: EditorInstanceId, plugin: &str, block: Option<Uuid>) {
    with(|docks| docks.register(instance, plugin, block));
}

pub(super) fn forget(instance: EditorInstanceId) {
    with(|docks| docks.forget(instance));
}

pub(super) fn restarted(plugin: &str) {
    with(|docks| docks.restarted(plugin));
}

pub(super) fn placed(
    placer: EditorInstanceId,
    children: &[DockedChild],
    framed: Option<DockedChild>,
) {
    with(|docks| docks.placed(placer, children, framed));
}

pub(super) fn offscreen(placer: EditorInstanceId) {
    with(|docks| docks.offscreen(placer));
}

pub(super) fn heard(plugin: &str, messages: Vec<DockMessage>) {
    with(|docks| docks.heard(plugin, messages));
}

pub(super) fn docked(instance: EditorInstanceId) -> bool {
    DOCKS.with(|docks| docks.borrow().docked(instance))
}

pub(super) fn resolve(placer: EditorInstanceId, path: &DockTabPath) -> Option<(Uuid, u64)> {
    DOCKS.with(|docks| docks.borrow().resolve(placer, path))
}

pub(super) fn is_bar(instance: EditorInstanceId, view: u64) -> bool {
    DOCKS.with(|docks| {
        docks
            .borrow()
            .declared
            .get(&instance)
            .is_some_and(|declaration| declaration.bar(&DockTabPath::own(view)).is_some())
    })
}

pub(super) fn bar(instance: EditorInstanceId, action: BarAction) -> bool {
    with(|docks| docks.bar(instance, action))
}

pub(super) fn take_bars(plugin: &str) -> Vec<(EditorInstanceId, ViewId, ChildId, BarAction)> {
    DOCKS.with(|docks| docks.borrow_mut().take_bars(plugin))
}

pub(super) fn set_menu(instance: EditorInstanceId, view: ViewId, items: Vec<MenuItem>) {
    with(|docks| docks.set_menu(instance, view, items));
}

pub(super) fn take_picks(plugin: &str) -> Vec<Pick> {
    DOCKS.with(|docks| docks.borrow_mut().take_picks(plugin))
}

pub(super) fn take(plugin: &str) -> Vec<(EditorInstanceId, DockUpdate)> {
    DOCKS.with(|docks| docks.borrow_mut().take(plugin))
}

#[cfg(test)]
mod tests;
