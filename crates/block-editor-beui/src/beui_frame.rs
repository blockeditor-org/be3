use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui::NodeId;
use beui::icons::{
    ICON_CLOSE, ICON_DRIVE_FILE_RENAME_OUTLINE, ICON_INFO, ICON_REDO, ICON_SHARE, ICON_TERMINAL,
    ICON_UNDO,
};
use beui::reactive::*;
use beui::styled::*;
use beui::{Document, Key, KeyAt, KeyPhase};
use block_plugin_api::{BarAction, MenuEntry};

use crate::chrome::ChromeRoot;
use crate::{BlockInfo, BlockList, BlockQuery, Editor};

const PALETTE: Chord = Chord::ctrl(Key::P).shift();

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameBar {
    pub shown: bool,
    pub framed: bool,
    pub closable: bool,
    pub on_phone: bool,
}

#[derive(Clone)]
struct PhoneLayout(Memo<bool>);

pub fn phone_layout() -> Memo<bool> {
    match use_context::<PhoneLayout>() {
        Some(PhoneLayout(phone)) => phone,
        None => create_memo(|| false),
    }
}

pub struct BeuiFrame {
    document: Document,
    content: NodeId,
    bar: Signal<FrameBar>,
    exit: Rc<Cell<bool>>,
    menu: Rc<RefCell<Vec<Action>>>,
    _bar: OwnedSignal<FrameBar>,
}

impl BeuiFrame {
    pub fn build(editor: Editor, view: impl FnOnce() -> NodeId + 'static) -> Self {
        let owned_bar = OwnedSignal::new(FrameBar::default());
        let bar = owned_bar.handle();
        let exit = Rc::new(Cell::new(false));
        let exit_writer = exit.clone();
        let menu: Rc<RefCell<Vec<Action>>> = Rc::default();
        let published = Rc::clone(&menu);
        let content_slot: Rc<Cell<Option<NodeId>>> = Rc::new(Cell::new(None));
        let content_slot_writer = content_slot.clone();
        let document = beui::reactive::build(move || {
            provide_context(PhoneLayout(create_memo(move || bar.get().on_phone)));
            let phone = create_memo(move || bar.get().on_phone);
            provide_density(create_memo(move || match phone.get() {
                true => Density::Comfortable,
                false => Density::Compact,
            }));
            let palette = create_signal(false);
            let opener = Action::new("editor.palette", "Command palette", move || {
                palette.set(true)
            })
            .glyph(ICON_TERMINAL)
            .shortcut(PALETTE)
            .register();
            let escaping = exit_writer.clone();
            on_key(KeyAt::Window, KeyPhase::Bubble, move |event| {
                let escaped = event
                    .press()
                    .is_some_and(|press| press.pressed && press.key == Key::Escape);
                if escaped {
                    escaping.set(true);
                }
                escaped
            });
            view! {
                <List spacing=0.0>
                    <FrameMenu
                        editor
                        bar={bar.read_only()}
                        palette={opener}
                        published
                        on_exit={move || exit_writer.set(true)}
                    />
                    <CommandPalette open={palette} on_close={move || palette.set(false)} />
                    <ChromeRoot @sizing=ItemSize::Percent(100.0) phone>|| {
                        let content = view();
                        content_slot_writer.set(Some(content));
                        content
                    }</ChromeRoot>
                </List>
            }
        });
        Self {
            document,
            content: content_slot
                .get()
                .expect("view() builds its root node synchronously"),
            bar,
            exit,
            menu,
            _bar: owned_bar,
        }
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn document_mut(&mut self) -> &mut Document {
        &mut self.document
    }

    pub fn content(&self) -> NodeId {
        self.content
    }

    pub fn bar(&self) -> Signal<FrameBar> {
        self.bar
    }

    pub fn exit(&self) -> Rc<Cell<bool>> {
        self.exit.clone()
    }

    pub fn menu(&self) -> Rc<RefCell<Vec<Action>>> {
        Rc::clone(&self.menu)
    }
}

pub(crate) fn run_menu_pick(menu: &RefCell<Vec<Action>>, id: &str) {
    let picked = menu
        .borrow()
        .iter()
        .find(|action| action.id() == id)
        .cloned();
    if let Some(action) = picked {
        untrack(|| action.run());
    }
}

pub(crate) fn menu_entries(actions: &[Action]) -> Vec<MenuEntry> {
    actions
        .iter()
        .map(|action| MenuEntry {
            id: action.id().to_owned(),
            label: action.label().get(),
            glyph: action.glyph().to_owned(),
            enabled: action.enabled().get(),
        })
        .collect()
}

#[derive(Clone, Default, PartialEq)]
struct BarState {
    can_undo: bool,
    can_redo: bool,
    can_rename: bool,
    can_share: bool,
}

struct Watched {
    editor: Editor,
    list: RefCell<Option<BlockList>>,
    watching: Cell<bool>,
}

impl Watched {
    fn info(&self) -> Option<BlockInfo> {
        let mut list = self.list.borrow_mut();
        let list = list.get_or_insert_with(|| {
            self.editor
                .blocks()
                .watch(BlockQuery::Block(self.editor.block_id()))
        });
        list.read().into_iter().next()
    }

    fn can_edit(&self) -> bool {
        self.info()
            .is_none_or(|info| info.access == crate::AccessLevel::Edit)
    }

    fn read(&self) -> BarState {
        let id = self.editor.block_id();
        if !self.watching.replace(true) {
            self.editor.host().watch_history([id]);
        }
        let editable = self.editor.editable().get() && self.can_edit();
        let (can_undo, can_redo) = match editable {
            true => self.history(),
            false => (false, false),
        };
        BarState {
            can_undo,
            can_redo,
            can_rename: editable,
            can_share: self.can_edit(),
        }
    }

    fn history(&self) -> (bool, bool) {
        self.editor.histories().get();
        let history = self.editor.host().history(self.editor.block_id());
        (history.can_undo, history.can_redo)
    }

    fn finger_tap(&self, fingers: usize) -> bool {
        match fingers {
            2 => self.try_step(false),
            3 => self.try_step(true),
            _ => false,
        }
    }

    fn try_step(&self, redo: bool) -> bool {
        if !self.editor.host().editable() || !self.can_edit() {
            return false;
        }
        let (can_undo, can_redo) = self.history();
        let possible = match redo {
            true => can_redo,
            false => can_undo,
        };
        if !possible {
            return false;
        }
        self.step(redo);
        true
    }

    fn step(&self, redo: bool) {
        let id = self.editor.block_id();
        match redo {
            true => self.editor.host().redo(id),
            false => self.editor.host().undo(id),
        }
    }
}

#[derive(Clone)]
struct FrameActions {
    undo: Action,
    redo: Action,
    rename: Action,
    share: Action,
    details: Action,
    close: Action,
}

fn frame_actions(
    editor: Editor,
    watched: Stored<Watched>,
    state: ReadSignal<BarState>,
    live: Memo<bool>,
    closable: Memo<bool>,
    on_exit: ClickCallback,
) -> FrameActions {
    let id = editor.block_id();
    let can = |read: fn(&BarState) -> bool| create_memo(move || live.get() && state.with(read));
    FrameActions {
        undo: Action::new("editor.undo", "Undo", move || {
            watched.with(|watched| watched.try_step(false));
        })
        .glyph(ICON_UNDO)
        .shortcut(Chord::ctrl(Key::Z))
        .enabled(can(|state| state.can_undo))
        .register(),
        redo: Action::new("editor.redo", "Redo", move || {
            watched.with(|watched| watched.try_step(true));
        })
        .glyph(ICON_REDO)
        .shortcut(Chord::ctrl(Key::Z).shift())
        .shortcut(Chord::ctrl(Key::Y))
        .enabled(can(|state| state.can_redo))
        .register(),
        rename: Action::new("editor.rename", "Rename", move || {
            editor.host().rename_block(id)
        })
        .glyph(ICON_DRIVE_FILE_RENAME_OUTLINE)
        .enabled(can(|state| state.can_rename))
        .register(),
        share: Action::new("editor.share", "Share", move || {
            editor.host().share_block(id)
        })
        .glyph(ICON_SHARE)
        .enabled(can(|state| state.can_share))
        .register(),
        details: Action::new("editor.details", "File details", move || {
            editor.host().bar_action(BarAction::Details)
        })
        .glyph(ICON_INFO)
        .register(),
        close: Action::new("editor.close", "Close", move || on_exit.call())
            .glyph(ICON_CLOSE)
            .enabled(closable)
            .register(),
    }
}

#[component]
pub(crate) fn FrameMenu(
    editor: Editor,
    bar: ReadSignal<FrameBar>,
    palette: Action,
    published: Rc<RefCell<Vec<Action>>>,
    on_exit: ClickCallback,
) -> NodeId {
    let on_phone = create_memo(move || bar.get().on_phone);
    let shown = create_memo(move || bar.get().shown && !on_phone.get());
    let closable = create_memo(move || bar.get().closable);
    let live = create_memo(move || shown.get() || on_phone.get());
    let framed = create_memo(move || bar.get().framed);
    let answering = create_memo(move || live.get() || framed.get());
    let state = create_signal(BarState::default());
    let watched = store(Watched {
        editor,
        list: RefCell::new(None),
        watching: Cell::new(false),
    });
    let actions = frame_actions(
        editor,
        watched,
        state.read_only(),
        answering,
        closable,
        on_exit,
    );
    on_finger_tap(move |fingers: usize| {
        answering.get_untracked() && watched.with(|watched| watched.finger_tap(fingers))
    });
    create_effect(move || {
        if answering.get() {
            state.set(watched.with(Watched::read));
        }
    });
    let items = menu_actions();
    let FrameActions {
        undo,
        redo,
        rename,
        share,
        details,
        close,
    } = actions;
    let listed = create_memo(move || {
        if !live.get() {
            return Vec::new();
        }
        let mut listed = vec![
            undo.clone(),
            redo.clone(),
            rename.clone(),
            share.clone(),
            palette.clone(),
        ];
        listed.extend(items.get());
        listed.push(details.clone());
        if closable.get() {
            listed.push(close.clone());
        }
        listed
    });
    let host = editor.host();
    create_effect(move || {
        let listed = listed.get();
        let entries = menu_entries(&listed);
        *published.borrow_mut() = listed;
        host.set_menu(entries);
    });
    view! {
        <List spacing=0.0 />
    }
}
