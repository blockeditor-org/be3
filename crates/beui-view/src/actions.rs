use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use std::any::Any;

use crate::reactive::{
    Get, IntoProp, KeyAt, KeyEvent, KeyInput, Memo, NodeId, NodeRef, OwnedSignal, Prop, Set,
    create_effect, create_memo, on_cleanup, provide_context, untrack, use_context,
};
use crate::types::icons::Glyph;
use crate::types::input::KeyChord;
use crate::types::{ActionGroup, ActionInfo, Key, KeyPhase, KeyPress, Modifiers, WindowId};

type KeyHandler = dyn Fn(KeyEvent) -> bool;

pub(crate) type Describe = dyn Fn(
    &[(Option<WindowId>, String)],
    &[NodeId],
    &[WindowId],
    &[(WindowId, WindowId)],
) -> Described;

pub(crate) type Described = (Vec<String>, Vec<ActionGroup>);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    pub key: Key,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub logo: bool,
    pub tap: bool,
}

impl Chord {
    pub const fn key(key: Key) -> Self {
        Self {
            key,
            ctrl: false,
            shift: false,
            alt: false,
            logo: false,
            tap: false,
        }
    }

    pub const fn tap(key: Key) -> Self {
        Self {
            tap: true,
            ..Self::key(key)
        }
    }

    pub const fn logo(key: Key) -> Self {
        Self {
            logo: true,
            ..Self::key(key)
        }
    }

    pub const fn ctrl(key: Key) -> Self {
        Self {
            ctrl: true,
            ..Self::key(key)
        }
    }

    pub const fn shift(self) -> Self {
        Self {
            shift: true,
            ..self
        }
    }

    pub const fn alt(self) -> Self {
        Self { alt: true, ..self }
    }

    pub fn matches(self, press: KeyPress) -> bool {
        !self.tap
            && press.pressed
            && press.key == self.key
            && press.modifiers.ctrl == self.ctrl
            && press.modifiers.shift == self.shift
            && press.modifiers.alt == self.alt
            && press.modifiers.logo == self.logo
    }

    pub fn label(self) -> String {
        let mut parts = Vec::new();
        if self.logo {
            parts.push("Super".to_owned());
        }
        if self.ctrl {
            parts.push("Ctrl".to_owned());
        }
        if self.alt {
            parts.push("Alt".to_owned());
        }
        if self.shift {
            parts.push("Shift".to_owned());
        }
        parts.push(key_label(self.key));
        parts.join("+")
    }

    fn typed(self) -> bool {
        !self.tap && !self.ctrl && !self.alt && !self.logo && !self.key.is_media()
    }

    fn heard(self, event: KeyEvent) -> bool {
        match event.input {
            KeyInput::Tap(key) => self.tap && key == self.key,
            KeyInput::Press(press) => self.matches(press) && !(event.target.typing && self.typed()),
        }
    }

    pub fn key_chord(self) -> KeyChord {
        KeyChord {
            key: self.key,
            modifiers: Modifiers {
                alt: self.alt,
                ctrl: self.ctrl,
                shift: self.shift,
                logo: self.logo,
            },
            tap: self.tap,
        }
    }
}

impl From<KeyChord> for Chord {
    fn from(chord: KeyChord) -> Self {
        Self {
            key: chord.key,
            ctrl: chord.modifiers.ctrl,
            shift: chord.modifiers.shift,
            alt: chord.modifiers.alt,
            logo: chord.modifiers.logo,
            tap: chord.tap,
        }
    }
}

fn key_label(key: Key) -> String {
    let symbol = match key {
        Key::ArrowDown => "Down",
        Key::ArrowLeft => "Left",
        Key::ArrowRight => "Right",
        Key::ArrowUp => "Up",
        Key::BracketLeft => "[",
        Key::BracketRight => "]",
        Key::Minus => "-",
        Key::Plus => "+",
        Key::Zero => "0",
        Key::One => "1",
        Key::Two => "2",
        Key::Three => "3",
        Key::Four => "4",
        Key::Five => "5",
        Key::Six => "6",
        Key::Seven => "7",
        Key::Eight => "8",
        Key::Nine => "9",
        Key::Backtick => "`",
        Key::Comma => ",",
        Key::Period => ".",
        Key::Slash => "/",
        Key::Backslash => "\\",
        Key::Semicolon => ";",
        Key::Quote => "'",
        Key::PageDown => "Page Down",
        Key::PageUp => "Page Up",
        Key::BrowserBack => "Back",
        Key::VolumeUp => "Volume Up",
        Key::VolumeDown => "Volume Down",
        Key::VolumeMute => "Mute",
        Key::MicMute => "Mic Mute",
        Key::BrightnessUp => "Brightness Up",
        Key::BrightnessDown => "Brightness Down",
        Key::MediaPlayPause => "Play/Pause",
        Key::MediaNext => "Next Track",
        Key::MediaPrevious => "Previous Track",
        Key::MediaStop => "Stop",
        Key::Logo => "Super",
        _ => return format!("{key:?}"),
    };
    symbol.to_owned()
}

thread_local! {
    static DETACHED: Cell<u64> = const { Cell::new(1 << 63) };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pin {
    Icon,
    Label,
    Title,
}

struct ActionData {
    key: Cell<u64>,
    id: String,
    label: Prop<String>,
    glyph: Option<Glyph>,
    shortcuts: Vec<Chord>,
    enabled: Prop<bool>,
    checked: Option<Prop<bool>>,
    radio: bool,
    pin: Option<Pin>,
    at: Option<KeyAt>,
    capture: bool,
    run: Rc<dyn Fn()>,
    window: Cell<Option<WindowId>>,
}

#[derive(Clone)]
pub struct Action(Rc<ActionData>);

impl PartialEq for Action {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Action {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        id: impl Into<String>,
        label: impl IntoProp<String>,
        run: impl Fn() + 'static,
    ) -> ActionBuilder {
        ActionBuilder {
            id: id.into(),
            label: label.into_prop(),
            glyph: None,
            shortcuts: Vec::new(),
            enabled: Prop::Static(true),
            checked: None,
            radio: false,
            pin: None,
            at: None,
            capture: false,
            run: Rc::new(run),
        }
    }

    pub fn key(&self) -> u64 {
        self.0.key.get()
    }

    pub fn id(&self) -> &str {
        &self.0.id
    }

    pub fn label(&self) -> Prop<String> {
        self.0.label.clone()
    }

    pub fn glyph(&self) -> Option<Glyph> {
        self.0.glyph
    }

    pub fn shortcuts(&self) -> &[Chord] {
        &self.0.shortcuts
    }

    pub fn shortcut_label(&self) -> Option<String> {
        self.0.shortcuts.first().map(|chord| chord.label())
    }

    pub fn enabled(&self) -> Prop<bool> {
        self.0.enabled
    }

    pub fn disabled(&self) -> Prop<bool> {
        self.0.enabled.map(|enabled| !enabled)
    }

    pub fn checked(&self) -> Option<Prop<bool>> {
        self.0.checked
    }

    pub fn is_radio(&self) -> bool {
        self.0.radio
    }

    pub fn pinned(&self) -> Option<Pin> {
        self.0.pin
    }

    pub fn is_global(&self) -> bool {
        matches!(self.0.at, Some(KeyAt::Window | KeyAt::Host))
    }

    pub fn intercepts(&self) -> bool {
        self.is_global() && self.0.capture
    }

    fn listens(&self, at: &KeyAt, phase: KeyPhase) -> bool {
        let same = matches!(
            (&self.0.at, at),
            (Some(KeyAt::Window), KeyAt::Window)
                | (Some(KeyAt::Host), KeyAt::Host)
                | (None, KeyAt::Root)
        );
        same && self.hears(phase)
    }

    fn hears(&self, phase: KeyPhase) -> bool {
        self.0.capture == (phase != KeyPhase::Bubble)
    }

    pub fn window(&self) -> Option<WindowId> {
        self.0.window.get()
    }

    fn belongs(&self, window: Option<WindowId>, host: &dyn Fn(WindowId) -> WindowId) -> bool {
        match (self.window(), window) {
            (Some(own), Some(window)) => host(own) == window,
            _ => true,
        }
    }

    fn rooted(&self, phase: KeyPhase) -> bool {
        self.0.at.is_none() && self.hears(phase)
    }

    pub fn is_enabled(&self) -> bool {
        self.0.enabled.get_untracked()
    }

    pub fn run(&self) -> bool {
        if !self.is_enabled() {
            return false;
        }
        (self.0.run)();
        true
    }
}

pub struct ActionBuilder {
    id: String,
    label: Prop<String>,
    glyph: Option<Glyph>,
    shortcuts: Vec<Chord>,
    enabled: Prop<bool>,
    checked: Option<Prop<bool>>,
    radio: bool,
    pin: Option<Pin>,
    at: Option<KeyAt>,
    capture: bool,
    run: Rc<dyn Fn()>,
}

impl ActionBuilder {
    pub fn glyph(mut self, glyph: impl Into<Option<Glyph>>) -> Self {
        self.glyph = glyph.into();
        self
    }

    pub fn shortcut(mut self, chord: Chord) -> Self {
        self.shortcuts.push(chord);
        self
    }

    pub fn enabled(mut self, enabled: impl IntoProp<bool>) -> Self {
        self.enabled = enabled.into_prop();
        self
    }

    pub fn checked(mut self, checked: impl IntoProp<bool>) -> Self {
        self.checked = Some(checked.into_prop());
        self.radio = false;
        self
    }

    pub fn radio(mut self, chosen: impl IntoProp<bool>) -> Self {
        self.checked = Some(chosen.into_prop());
        self.radio = true;
        self
    }

    pub fn pin(mut self, pin: Pin) -> Self {
        self.pin = Some(pin);
        self
    }

    pub fn at(mut self, at: KeyAt) -> Self {
        self.at = match at {
            KeyAt::Root => None,
            at => Some(at),
        };
        self
    }

    pub fn capture(mut self) -> Self {
        self.capture = true;
        self
    }

    fn into_action(self) -> Action {
        Action(Rc::new(ActionData {
            key: Cell::new(0),
            id: self.id,
            label: self.label,
            glyph: self.glyph,
            shortcuts: self.shortcuts,
            enabled: self.enabled,
            checked: self.checked,
            radio: self.radio,
            pin: self.pin,
            at: self.at,
            capture: self.capture,
            run: self.run,
            window: Cell::new(None),
        }))
    }

    pub fn detached(self) -> Action {
        let action = self.into_action();
        let key = DETACHED.with(|next| next.replace(next.get() + 1));
        action.0.key.set(key);
        action
    }

    pub fn register(self) -> Action {
        let action = self.into_action();
        let registry = registry();
        let scope = use_context::<ActionScope>()
            .map_or_else(|| Rc::clone(&registry.0.global), |scope| scope.0);
        let key = registry.0.next.get();
        registry.0.next.set(key + 1);
        action.0.key.set(key);
        action
            .0
            .window
            .set(crate::window::use_window().map(|window| window.id));
        scope.add(action.clone());
        if let Some(KeyAt::Node(node)) = &action.0.at {
            let phase = match action.0.capture {
                true => KeyPhase::Capture,
                false => KeyPhase::Bubble,
            };
            let listening = action.clone();
            crate::reactive::on_key(KeyAt::Node(*node), phase, move |event| {
                run_matching(std::iter::once(&listening), event)
            });
        }
        registry.touch();
        on_cleanup(move || {
            scope.remove(key);
            registry.touch();
        });
        action
    }
}

struct ScopeData {
    node: Option<NodeRef>,
    actions: RefCell<Vec<Action>>,
}

impl ScopeData {
    fn add(&self, action: Action) {
        self.actions.borrow_mut().push(action);
    }

    fn remove(&self, key: u64) {
        self.actions
            .borrow_mut()
            .retain(|action| action.key() != key);
    }

    fn node(&self) -> Option<NodeId> {
        self.node.as_ref().and_then(NodeRef::try_get)
    }
}

#[derive(Clone)]
pub struct ActionScope(Rc<ScopeData>);

pub fn action_scope(node: &NodeRef) -> ActionScope {
    let registry = registry();
    let scope = ActionScope(Rc::new(ScopeData {
        node: Some(*node),
        actions: RefCell::new(Vec::new()),
    }));
    registry.0.scopes.borrow_mut().push(Rc::downgrade(&scope.0));
    provide_context(scope.clone());
    for phase in [KeyPhase::Capture, KeyPhase::Bubble] {
        let listening = Rc::downgrade(&scope.0);
        crate::reactive::on_key(KeyAt::Node(*node), phase, move |event| {
            listening.upgrade().is_some_and(|scope| {
                let actions = scope.actions.borrow().clone();
                run_matching(
                    actions.iter().filter(|action| action.rooted(event.phase)),
                    event,
                )
            })
        });
    }
    let held = Rc::clone(&scope.0);
    on_cleanup(move || {
        registry.0.scopes.borrow_mut().retain(|other| {
            other
                .upgrade()
                .is_some_and(|other| !Rc::ptr_eq(&other, &held))
        });
        registry.touch();
    });
    scope
}

struct RegistryData {
    global: Rc<ScopeData>,
    scopes: RefCell<Vec<Weak<ScopeData>>>,
    next: Cell<u64>,
    version: OwnedSignal<u64>,
    listeners: RefCell<Vec<Rc<KeyHandler>>>,
    described: RefCell<Option<Rc<dyn Any>>>,
    menus: RefCell<Vec<(u64, Vec<MenuNode>)>>,
}

#[derive(Clone)]
struct Registry(Rc<RegistryData>);

impl Registry {
    fn new() -> Self {
        Self(Rc::new(RegistryData {
            global: Rc::new(ScopeData {
                node: None,
                actions: RefCell::new(Vec::new()),
            }),
            scopes: RefCell::new(Vec::new()),
            next: Cell::new(1),
            version: OwnedSignal::new(0),
            listeners: RefCell::new(Vec::new()),
            described: RefCell::new(None),
            menus: RefCell::new(Vec::new()),
        }))
    }

    fn touch(&self) {
        self.0
            .version
            .handle()
            .update(|version| *version = version.wrapping_add(1));
    }

    fn scopes(&self) -> Vec<Rc<ScopeData>> {
        self.0
            .scopes
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .collect()
    }

    fn active(&self, focus_path: &[NodeId]) -> Vec<Rc<ScopeData>> {
        let scopes = self.scopes();
        let mut active: Vec<Rc<ScopeData>> = Vec::new();
        for node in focus_path {
            for scope in &scopes {
                if scope.node() == Some(*node) && !active.iter().any(|seen| Rc::ptr_eq(seen, scope))
                {
                    active.push(Rc::clone(scope));
                }
            }
        }
        if active.is_empty() {
            active.extend(scopes.into_iter().rev());
        }
        active.push(Rc::clone(&self.0.global));
        active
    }

    fn actions(&self, focus_path: &[NodeId]) -> Vec<Action> {
        let mut listed: Vec<Action> = Vec::new();
        let scoped = self
            .active(focus_path)
            .into_iter()
            .flat_map(|scope| scope.actions.borrow().clone());
        let global = self.all().into_iter().filter(Action::is_global);
        for action in scoped.chain(global) {
            if !listed.iter().any(|seen| seen.id() == action.id()) {
                listed.push(action);
            }
        }
        listed
    }

    fn root_key(&self, event: KeyEvent) -> bool {
        let path = crate::core::actions::focus_ancestry();
        let window = crate::core::actions::key_window();
        let hosts = crate::core::actions::window_hosts();
        let host = |own: WindowId| host_of(&hosts, own);
        let scopes = self.scopes();
        let scoped = path
            .iter()
            .any(|node| scopes.iter().any(|scope| scope.node() == Some(*node)));
        let mut actions: Vec<Action> = self.0.global.actions.borrow().clone();
        if !scoped && event.target.node.is_some() {
            for scope in scopes.iter().rev() {
                actions.extend(scope.actions.borrow().iter().cloned());
            }
        }
        run_matching(
            actions
                .iter()
                .filter(|action| action.rooted(event.phase) && action.belongs(window, &host)),
            event,
        )
    }

    fn outer_key(&self, at: &KeyAt, event: KeyEvent) -> bool {
        let actions = self.all();
        let window = crate::core::actions::key_window();
        let hosts = crate::core::actions::window_hosts();
        let host = |own: WindowId| host_of(&hosts, own);
        run_matching(
            actions
                .iter()
                .filter(|action| action.listens(at, event.phase) && action.belongs(window, &host)),
            event,
        )
    }

    fn intercepted(&self) -> Vec<KeyChord> {
        let mut chords: Vec<KeyChord> = Vec::new();
        for action in self.all() {
            if !action.intercepts() || !action.0.enabled.get() {
                continue;
            }
            for chord in action.shortcuts().iter().filter(|chord| !chord.typed()) {
                let chord = chord.key_chord();
                if !chords.contains(&chord) {
                    chords.push(chord);
                }
            }
        }
        chords
    }

    fn describe(
        &self,
        requests: &[(Option<WindowId>, String)],
        focus_path: &[NodeId],
        windows: &[WindowId],
        hosts: &[(WindowId, WindowId)],
    ) -> Described {
        let live = self.actions(focus_path);
        let all = self.all();
        let host = |own: WindowId| host_of(hosts, own);
        let hosted = |action: &Action| action.window().map(host);
        let mut ran = Vec::new();
        for (window, requested) in requests {
            let window = *window;
            let found = live
                .iter()
                .chain(all.iter())
                .filter(|action| action.id() == requested)
                .find(|action| window.is_none() || hosted(action) == window)
                .or_else(|| {
                    live.iter()
                        .chain(all.iter())
                        .find(|action| action.id() == requested && action.belongs(window, &host))
                });
            if let Some(action) = found {
                untrack(|| action.run());
                ran.push(requested.clone());
            }
        }
        let info = |action: &Action, live: bool| ActionInfo {
            id: action.id().to_owned(),
            label: action.label().get_untracked(),
            shortcut: action.shortcut_label(),
            enabled: action.is_enabled(),
            checked: action.checked().map(|checked| checked.get_untracked()),
            live,
        };
        let mut actions: Vec<ActionInfo> = live.iter().map(|action| info(action, true)).collect();
        for action in &all {
            if !actions.iter().any(|listed| listed.id == action.id()) {
                actions.push(info(action, false));
            }
        }
        let mut groups = vec![ActionGroup {
            name: String::new(),
            window: None,
            actions,
        }];
        for window in windows {
            let actions: Vec<ActionInfo> = all
                .iter()
                .filter(|action| hosted(action) == Some(*window))
                .map(|action| info(action, live.contains(action)))
                .collect();
            if !actions.is_empty() {
                groups.push(ActionGroup {
                    name: String::new(),
                    window: Some(*window),
                    actions,
                });
            }
        }
        (ran, groups)
    }

    fn all(&self) -> Vec<Action> {
        let mut all: Vec<Action> = self.0.global.actions.borrow().clone();
        for scope in self.scopes() {
            all.extend(scope.actions.borrow().iter().cloned());
        }
        all.sort_by_key(Action::key);
        all
    }
}

fn host_of(hosts: &[(WindowId, WindowId)], window: WindowId) -> WindowId {
    hosts
        .iter()
        .find(|(id, _)| *id == window)
        .map_or(WindowId::MAIN, |(_, host)| *host)
}

fn run_matching<'a>(actions: impl Iterator<Item = &'a Action>, event: KeyEvent) -> bool {
    for action in actions {
        let matched = action.shortcuts().iter().any(|chord| chord.heard(event));
        if matched && untrack(|| action.run()) {
            return true;
        }
    }
    false
}

fn registry() -> Registry {
    let mut fresh = false;
    let registry = crate::reactive::tree_local(|| {
        fresh = true;
        Registry::new()
    });
    if fresh {
        install(&registry);
    }
    registry
}

fn install(registry: &Registry) {
    let listen = |at: KeyAt, phase: KeyPhase| {
        let weak = Rc::downgrade(&registry.0);
        let place = at.clone();
        let handler: Rc<KeyHandler> = Rc::new(move |event: KeyEvent| {
            weak.upgrade().is_some_and(|registry| {
                let registry = Registry(registry);
                match place {
                    KeyAt::Root => registry.root_key(event),
                    _ => registry.outer_key(&place, event),
                }
            })
        });
        crate::core::actions::listen_keys(at, phase, Rc::downgrade(&handler));
        registry.0.listeners.borrow_mut().push(handler);
    };
    for at in [KeyAt::Root, KeyAt::Window, KeyAt::Host] {
        listen(at.clone(), KeyPhase::Capture);
        listen(at, KeyPhase::Bubble);
    }
    let weak = Rc::downgrade(&registry.0);
    let described: Rc<Describe> = Rc::new(
        move |requests: &[(Option<WindowId>, String)],
              path: &[NodeId],
              windows: &[WindowId],
              hosts: &[(WindowId, WindowId)]| match weak.upgrade() {
            Some(registry) => Registry(registry).describe(requests, path, windows, hosts),
            None => (Vec::new(), Vec::new()),
        },
    );
    *registry.0.described.borrow_mut() = Some(crate::core::actions::register(described));
    let intercepted = crate::core::actions::intercepted_keys();
    let weak = Rc::downgrade(&registry.0);
    let reported = RefCell::new(Vec::new());
    crate::core::actions::in_document_scope(Box::new(move || {
        create_effect(move || {
            let Some(registry) = weak.upgrade() else {
                return;
            };
            let registry = Registry(registry);
            registry.0.version.get();
            let chords = registry.intercepted();
            if *reported.borrow() != chords {
                reported.replace(chords.clone());
                intercepted(chords);
            }
        });
    }));
}

pub fn active_actions() -> Vec<Action> {
    let registry = registry();
    let path = crate::core::actions::focus_ancestry();
    registry.actions(&path)
}

pub fn active_actions_from(focus_path: &[NodeId]) -> Vec<Action> {
    registry().actions(focus_path)
}

pub fn add_to_menu(items: Vec<MenuNode>) {
    let registry = registry();
    let key = registry.0.next.get();
    registry.0.next.set(key + 1);
    registry.0.menus.borrow_mut().push((key, items));
    registry.touch();
    on_cleanup(move || {
        registry
            .0
            .menus
            .borrow_mut()
            .retain(|(held, _)| *held != key);
        registry.touch();
    });
}

pub fn document_menu() -> Memo<Vec<MenuNode>> {
    let registry = registry();
    create_memo(move || {
        registry.0.version.get();
        let mut nodes = Vec::new();
        for (_, items) in registry.0.menus.borrow().iter() {
            nodes.push(MenuNode::Separator);
            nodes.extend(items.iter().cloned());
        }
        tidy_menu(nodes)
    })
}

#[derive(Clone, PartialEq)]
pub enum MenuNode {
    Action {
        action: Action,
        label: Option<String>,
    },
    Submenu(Submenu),
    Separator,
}

#[derive(Clone, PartialEq)]
pub struct Submenu {
    key: u64,
    pub id: String,
    pub label: String,
    pub glyph: Option<Glyph>,
    pub items: Vec<MenuNode>,
}

impl Submenu {
    pub fn key(&self) -> u64 {
        self.key
    }
}

impl MenuNode {
    pub fn action(action: &Action) -> Self {
        Self::Action {
            action: action.clone(),
            label: None,
        }
    }

    pub fn labelled(action: &Action, label: impl Into<String>) -> Self {
        Self::Action {
            action: action.clone(),
            label: Some(label.into()),
        }
    }

    pub fn submenu(
        id: impl Into<String>,
        label: impl Into<String>,
        glyph: impl Into<Option<Glyph>>,
        items: Vec<MenuNode>,
    ) -> Self {
        let key = DETACHED.with(|next| next.replace(next.get() + 1));
        Self::Submenu(Submenu {
            key,
            id: id.into(),
            label: label.into(),
            glyph: glyph.into(),
            items,
        })
    }

    pub fn key(&self) -> Option<u64> {
        match self {
            Self::Action { action, .. } => Some(action.key()),
            Self::Submenu(submenu) => Some(submenu.key),
            Self::Separator => None,
        }
    }

    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Action { action, .. } => Some(action.id()),
            Self::Submenu(submenu) => Some(&submenu.id),
            Self::Separator => None,
        }
    }
}

pub fn menu_action(nodes: &[MenuNode], id: &str) -> Option<Action> {
    nodes.iter().find_map(|node| match node {
        MenuNode::Action { action, .. } => (action.id() == id).then(|| action.clone()),
        MenuNode::Submenu(submenu) => menu_action(&submenu.items, id),
        MenuNode::Separator => None,
    })
}

pub fn tidy_menu(nodes: Vec<MenuNode>) -> Vec<MenuNode> {
    let mut tidied: Vec<MenuNode> = Vec::new();
    let mut pending = false;
    for node in nodes {
        let node = match node {
            MenuNode::Separator => {
                pending = !tidied.is_empty();
                continue;
            }
            MenuNode::Submenu(mut submenu) => {
                submenu.items = tidy_menu(std::mem::take(&mut submenu.items));
                if submenu.items.is_empty() {
                    continue;
                }
                MenuNode::Submenu(submenu)
            }
            node => node,
        };
        if std::mem::take(&mut pending) {
            tidied.push(MenuNode::Separator);
        }
        tidied.push(node);
    }
    tidied
}

fn unset(text: &Prop<String>) -> bool {
    matches!(text, Prop::Static(text) if text.is_empty())
}

pub fn action_label(action: Option<&Action>, label: Prop<String>) -> Prop<String> {
    match action {
        Some(action) if unset(&label) => action.label(),
        _ => label,
    }
}

pub fn action_glyph(action: Option<&Action>, glyph: Prop<Option<Glyph>>) -> Prop<Option<Glyph>> {
    match action {
        Some(action) if matches!(glyph, Prop::Static(None)) => Prop::Static(action.glyph()),
        _ => glyph,
    }
}

pub fn action_disabled(action: Option<&Action>, disabled: Prop<bool>) -> Prop<bool> {
    match action {
        Some(action) => {
            let enabled = action.enabled();
            Prop::dynamic(move || disabled.get() || !enabled.get())
        }
        None => disabled,
    }
}

pub fn action_checked(action: Option<&Action>, checked: Prop<Option<bool>>) -> Prop<Option<bool>> {
    match action.and_then(Action::checked) {
        Some(ticked) if matches!(checked, Prop::Static(None)) => ticked.map(Some),
        _ => checked,
    }
}

pub fn action_tooltip(action: Option<&Action>, label: Prop<String>) -> Prop<String> {
    match action.and_then(Action::shortcut_label) {
        Some(shortcut) => label.map(move |label| format!("{label} ({shortcut})")),
        None => label,
    }
}
