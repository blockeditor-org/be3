use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet},
};

use block_plugin_api::{
    BarAction, ChildId, DockChild, DockChildRequest, DockClosedTab, DockDeclaration,
    DockDeclaredChild, DockHosted, DockHostedRegion, DockMessage, DockPickedTab, DockRequest,
    DockTabPath, DockUpdate, EditorInstanceId, EditorRegion, MAX_DOCK_NESTING, dock_prefix,
};
use uuid::Uuid;

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
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Hosting {
    Awaited,
    Hosted,
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
    outbox: Vec<(EditorInstanceId, DockUpdate)>,
    bars: Vec<(EditorInstanceId, ChildId, BarAction)>,
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
        self.blocks.retain(|_, held| *held != instance);
        self.children.remove(&instance);
        self.declared.remove(&instance);
        self.hosted.remove(&instance);
        self.hosting.retain(|(root, _), _| *root != instance);
        self.delivered.retain(|(root, _), _| *root != instance);
        self.outbox.retain(|(to, _)| *to != instance);
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

    pub(super) fn placed(&mut self, placer: EditorInstanceId, placed: &[DockedChild]) {
        let held = self.children.entry(placer).or_default();
        let mut changed = false;
        for (child, record) in held.iter_mut() {
            let now = placed.iter().any(|docked| docked.child == *child);
            changed |= record.placed != now;
            record.placed = now;
        }
        for docked in placed {
            let record = Placed {
                tab: docked.tab,
                block: docked.block,
                placed: true,
            };
            if held.insert(docked.child, record) != Some(record) {
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

    pub(super) fn heard(&mut self, plugin: &str, messages: Vec<DockMessage>) {
        if messages.is_empty() {
            return;
        }
        for message in messages {
            match message {
                DockMessage::Declare(declare) => {
                    let instance = declare.region.instance;
                    if declare.region.region != EditorRegion::Frame || !self.owns(plugin, instance)
                    {
                        continue;
                    }
                    match declare.declaration {
                        Some(declaration) => self.declared.insert(instance, declaration),
                        None => self.declared.remove(&instance),
                    };
                    self.refresh();
                }
                DockMessage::Request(request) => {
                    let instance = request.region.instance;
                    if request.region.region == EditorRegion::Frame && self.owns(plugin, instance) {
                        self.request(instance, request.request);
                    }
                }
                DockMessage::Host(host) => {
                    if self.owns(plugin, host.child.instance) {
                        self.host(host.child, host.hosted);
                    }
                }
                DockMessage::Close(close) => {
                    if self.owns(plugin, close.child.instance)
                        && let Some((owner, tab)) = self.owner(close.child, close.tab)
                    {
                        self.send(
                            owner,
                            DockUpdate::Close(DockClosedTab {
                                region: EditorRegion::Frame,
                                tab,
                            }),
                        );
                    }
                }
                DockMessage::Pick(pick) => {
                    if self.owns(plugin, pick.child.instance)
                        && let Some((owner, tab)) = self.owner(pick.child, pick.tab)
                    {
                        self.send(
                            owner,
                            DockUpdate::Pick(DockPickedTab {
                                region: EditorRegion::Frame,
                                tab,
                                id: pick.id,
                            }),
                        );
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
        let Some((placer, child, _)) = self.placer_of(instance) else {
            return false;
        };
        if let Some(plugin) = self.plugins.get(&placer) {
            self.changed.insert(plugin.clone());
        }
        self.bars.push((placer, child, action));
        true
    }

    pub(super) fn take_bars(
        &mut self,
        plugin: &str,
    ) -> Vec<(EditorInstanceId, ChildId, BarAction)> {
        let plugins = &self.plugins;
        let (taken, kept) = std::mem::take(&mut self.bars)
            .into_iter()
            .partition(|(to, _, _)| plugins.get(to).is_some_and(|held| held == plugin));
        self.bars = kept;
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
                    .filter_map(|record| self.instance_of(*placer, record))
            })
            .collect()
    }

    fn alive(
        &self,
        docked: &HashSet<EditorInstanceId>,
        placer: EditorInstanceId,
        child: ChildId,
        record: &Placed,
    ) -> bool {
        if record.placed {
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
        for instance in docked.symmetric_difference(&self.docked) {
            if let Some(plugin) = self.plugins.get(instance) {
                self.changed.insert(plugin.clone());
            }
        }
        self.docked = docked;
        let released: Vec<EditorInstanceId> = self
            .hosted
            .keys()
            .filter(|instance| !self.docked.contains(instance))
            .copied()
            .collect();
        for instance in released {
            self.hosted.remove(&instance);
            self.send(
                instance,
                DockUpdate::Hosted(DockHostedRegion {
                    region: EditorRegion::Frame,
                    hosted: None,
                }),
            );
        }
        let mut wanted: HashMap<(EditorInstanceId, ChildId), DockDeclaration> = HashMap::new();
        for (placer, children) in &self.children {
            if self.docked.contains(placer) {
                continue;
            }
            for (child, record) in children {
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
            self.hosting
                .entry((root, child))
                .or_insert(Hosting::Awaited);
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
        for (child, record) in self.children.get(&instance).into_iter().flatten() {
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
        Some(declaration)
    }

    fn request(&mut self, asker: EditorInstanceId, request: DockRequest) {
        let mut request = request;
        let mut instance = asker;
        let mut owner = Vec::new();
        for _ in 0..=MAX_DOCK_NESTING {
            let Some((placer, child, record)) = self.placer_of(instance) else {
                return;
            };
            if !self.docked(placer) {
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

    fn host(&mut self, child: DockChild, hosted: Option<DockHosted>) {
        let DockChild {
            instance: root,
            child,
        } = child;
        if self.docked(root) {
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
        match hosted.is_some() {
            true => {
                self.hosting.insert((root, child), Hosting::Hosted);
            }
            false => {
                self.hosting.remove(&(root, child));
            }
        }
        if let Some(instance) = self.instance_of(root, &record) {
            self.split(instance, hosted, 0);
        }
        self.refresh();
    }

    fn split(&mut self, instance: EditorInstanceId, hosted: Option<DockHosted>, depth: usize) {
        if depth > MAX_DOCK_NESTING || !self.docked(instance) {
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
            .map(|(child, record)| (*child, record.tab))
            .collect();
        let own = hosted.as_ref().and_then(|hosted| hosted.own(&hosts).ok());
        if self.hosted.get(&instance) != own.as_ref() {
            match &own {
                Some(own) => self.hosted.insert(instance, own.clone()),
                None => self.hosted.remove(&instance),
            };
            self.send(
                instance,
                DockUpdate::Hosted(DockHostedRegion {
                    region: EditorRegion::Frame,
                    hosted: own,
                }),
            );
        }
        for (child, record) in children {
            let Some(inner) = self.instance_of(instance, &record) else {
                continue;
            };
            let part = match (&hosted, self.prefix(instance, &record)) {
                (Some(hosted), Some(prefix)) => hosted.child(child, &prefix).ok().flatten(),
                _ => None,
            };
            self.split(inner, part, depth + 1);
        }
    }

    fn owner(&self, child: DockChild, tab: DockTabPath) -> Option<(EditorInstanceId, DockTabPath)> {
        let DockChild {
            instance: root,
            child,
        } = child;
        if self.docked(root) {
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
        crate::host::request_repaint();
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

pub(super) fn placed(placer: EditorInstanceId, children: &[DockedChild]) {
    with(|docks| docks.placed(placer, children));
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

pub(super) fn bar(instance: EditorInstanceId, action: BarAction) -> bool {
    with(|docks| docks.bar(instance, action))
}

pub(super) fn take_bars(plugin: &str) -> Vec<(EditorInstanceId, ChildId, BarAction)> {
    DOCKS.with(|docks| docks.borrow_mut().take_bars(plugin))
}

pub(super) fn take(plugin: &str) -> Vec<(EditorInstanceId, DockUpdate)> {
    DOCKS.with(|docks| docks.borrow_mut().take(plugin))
}

#[cfg(test)]
mod tests;
