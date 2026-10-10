use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use beui_core::callback::NodeRef;
use beui_core::document::{GlobalKey, GlobalKeyPress, UnhandledKey, UnhandledKeyPress};
use beui_core::input::{Key, KeyChord, KeyPress, Modifiers};
use beui_core::node::NodeId;
use reactive::{
    Memo, ReadSignal, WriteSignal, create_effect, create_memo, create_signal, on_cleanup,
    provide_context, untrack, use_context,
};

use crate::reactive::{IntoProp, Prop, with_document};

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

    fn heard(self, global: GlobalKeyPress) -> bool {
        match global.tap {
            true => self.tap && global.press.key == self.key,
            false => self.matches(global.press) && !(global.typing && self.typed()),
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

struct ActionData {
    key: Cell<u64>,
    id: String,
    label: Prop<String>,
    glyph: String,
    shortcuts: Vec<Chord>,
    enabled: Prop<bool>,
    checked: Option<Prop<bool>>,
    menu: bool,
    global: bool,
    intercepts: bool,
    run: Rc<dyn Fn()>,
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
            glyph: String::new(),
            shortcuts: Vec::new(),
            enabled: Prop::Static(true),
            checked: None,
            menu: false,
            global: false,
            intercepts: false,
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

    pub fn glyph(&self) -> &str {
        &self.0.glyph
    }

    pub fn shortcuts(&self) -> &[Chord] {
        &self.0.shortcuts
    }

    pub fn shortcut_label(&self) -> Option<String> {
        self.0.shortcuts.first().map(|chord| chord.label())
    }

    pub fn enabled(&self) -> Prop<bool> {
        self.0.enabled.clone()
    }

    pub fn disabled(&self) -> Prop<bool> {
        self.0.enabled.clone().map(|enabled| !enabled)
    }

    pub fn checked(&self) -> Option<Prop<bool>> {
        self.0.checked.clone()
    }

    pub fn in_menu(&self) -> bool {
        self.0.menu
    }

    pub fn is_global(&self) -> bool {
        self.0.global
    }

    pub fn intercepts(&self) -> bool {
        self.0.intercepts
    }

    pub fn is_enabled(&self) -> bool {
        self.0.enabled.peek()
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
    glyph: String,
    shortcuts: Vec<Chord>,
    enabled: Prop<bool>,
    checked: Option<Prop<bool>>,
    menu: bool,
    global: bool,
    intercepts: bool,
    run: Rc<dyn Fn()>,
}

impl ActionBuilder {
    pub fn glyph(mut self, glyph: &str) -> Self {
        glyph.clone_into(&mut self.glyph);
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
        self
    }

    pub fn in_menu(mut self) -> Self {
        self.menu = true;
        self
    }

    pub fn global(mut self) -> Self {
        self.global = true;
        self
    }

    pub fn intercepts(mut self) -> Self {
        self.global = true;
        self.intercepts = true;
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
            menu: self.menu,
            global: self.global,
            intercepts: self.intercepts,
            run: self.run,
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
        scope.add(action.clone());
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
        node: Some(node.clone()),
        actions: RefCell::new(Vec::new()),
    }));
    registry.0.scopes.borrow_mut().push(Rc::downgrade(&scope.0));
    provide_context(scope.clone());
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
    version: ReadSignal<u64>,
    set_version: WriteSignal<u64>,
    keys: RefCell<Option<Rc<UnhandledKey>>>,
    global_keys: RefCell<Option<Rc<GlobalKey>>>,
}

#[derive(Clone)]
struct Registry(Rc<RegistryData>);

impl Registry {
    fn new() -> Self {
        let (version, set_version) = create_signal(0);
        Self(Rc::new(RegistryData {
            global: Rc::new(ScopeData {
                node: None,
                actions: RefCell::new(Vec::new()),
            }),
            scopes: RefCell::new(Vec::new()),
            next: Cell::new(1),
            version,
            set_version,
            keys: RefCell::new(None),
            global_keys: RefCell::new(None),
        }))
    }

    fn touch(&self) {
        self.0
            .set_version
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

    fn key(&self, unhandled: &UnhandledKeyPress) -> bool {
        let actions = self.actions(&unhandled.focus_path);
        let local = actions.iter().filter(|action| !action.is_global());
        run_matching(
            local,
            GlobalKeyPress {
                press: unhandled.press,
                typing: unhandled.typing,
                in_app: false,
                held: false,
                tap: false,
            },
        )
    }

    fn global_key(&self, global: GlobalKeyPress) -> bool {
        let actions = self.all();
        let live = actions
            .iter()
            .filter(|action| action.is_global() && (action.intercepts() || !global.in_app));
        run_matching(live, global)
    }

    fn intercepted(&self) -> Vec<KeyChord> {
        let mut chords: Vec<KeyChord> = Vec::new();
        for action in self.all() {
            if !action.intercepts() || !action.0.enabled.get() {
                continue;
            }
            for chord in action
                .shortcuts()
                .iter()
                .filter(|chord| !chord.typed())
            {
                let chord = chord.key_chord();
                if !chords.contains(&chord) {
                    chords.push(chord);
                }
            }
        }
        chords
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

fn run_matching<'a>(actions: impl Iterator<Item = &'a Action>, global: GlobalKeyPress) -> bool {
    for action in actions {
        let matched = action.shortcuts().iter().any(|chord| chord.heard(global));
        if matched && untrack(|| action.run()) {
            return true;
        }
    }
    false
}

fn registry() -> Registry {
    with_document(|document| {
        if let Some(registry) = document.extension::<Registry>() {
            return registry;
        }
        let registry = document.extension_or_insert_with(Registry::new);
        let weak = Rc::downgrade(&registry.0);
        let keys: Rc<UnhandledKey> = Rc::new(move |unhandled: UnhandledKeyPress| {
            weak.upgrade()
                .is_some_and(|registry| Registry(registry).key(&unhandled))
        });
        document.register_unhandled_key(Rc::downgrade(&keys));
        *registry.0.keys.borrow_mut() = Some(keys);
        let weak = Rc::downgrade(&registry.0);
        let global: Rc<GlobalKey> = Rc::new(move |global: GlobalKeyPress| {
            weak.upgrade()
                .is_some_and(|registry| Registry(registry).global_key(global))
        });
        document.register_global_key(Rc::downgrade(&global));
        *registry.0.global_keys.borrow_mut() = Some(global);
        let intercepted = document.intercepted_keys_writer();
        let weak = Rc::downgrade(&registry.0);
        let reported = RefCell::new(Vec::new());
        document.reactive_scope().context().run(|| {
            create_effect(move || {
                let Some(registry) = weak.upgrade() else {
                    return;
                };
                let registry = Registry(registry);
                registry.0.version.get();
                let chords = registry.intercepted();
                if *reported.borrow() != chords {
                    reported.replace(chords.clone());
                    intercepted.set(chords);
                }
            });
        });
        registry
    })
}

pub fn active_actions() -> Vec<Action> {
    let registry = registry();
    let path = with_document(|document| document.focus_ancestry());
    registry.actions(&path)
}

pub fn active_actions_from(focus_path: &[NodeId]) -> Vec<Action> {
    registry().actions(focus_path)
}

pub fn menu_actions() -> Memo<Vec<Action>> {
    let registry = registry();
    create_memo(move || {
        registry.0.version.get();
        registry.all().into_iter().filter(Action::in_menu).collect()
    })
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

pub fn action_glyph(action: Option<&Action>, glyph: Prop<String>) -> Prop<String> {
    match action {
        Some(action) if unset(&glyph) => Prop::Static(action.glyph().to_owned()),
        _ => glyph,
    }
}

pub fn action_disabled(action: Option<&Action>, disabled: Prop<bool>) -> Prop<bool> {
    match action {
        Some(action) => {
            let enabled = action.enabled();
            Prop::Dynamic(Rc::new(move || disabled.get() || !enabled.get()))
        }
        None => disabled,
    }
}

pub fn action_pressed(action: Option<&Action>, pressed: Prop<bool>) -> Prop<bool> {
    match action.and_then(Action::checked) {
        Some(checked) => checked,
        None => pressed,
    }
}

pub fn action_tooltip(action: Option<&Action>, label: Prop<String>) -> Prop<String> {
    match action.and_then(Action::shortcut_label) {
        Some(shortcut) => label.map(move |label| format!("{label} ({shortcut})")),
        None => label,
    }
}
