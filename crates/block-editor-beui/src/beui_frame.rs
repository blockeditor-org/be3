use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui::NodeId;
use beui::icons::{
    ICON_INFO, ICON_PENCIL_LINE, ICON_REDO_2, ICON_SHARE_2, ICON_TERMINAL, ICON_UNDO_2, ICON_X,
};
use beui::reactive::*;
use beui::styled::*;
use beui::unstyled::Container;
use beui::{Key, KeyAt, KeyPhase, Pos2, Rect, Vec2, pos2};
use block_plugin_api::{BarAction, MenuCheck, MenuCommand, MenuEntry, MenuPin, MenuSubmenu};

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
    content: Rc<Cell<Option<NodeId>>>,
    bar: Signal<FrameBar>,
    exit: Rc<Cell<bool>>,
    menu: Rc<RefCell<Vec<MenuNode>>>,
    _bar: OwnedSignal<FrameBar>,
}

impl Default for BeuiFrame {
    fn default() -> Self {
        let owned_bar = OwnedSignal::new(FrameBar::default());
        Self {
            content: Rc::default(),
            bar: owned_bar.handle(),
            exit: Rc::new(Cell::new(false)),
            menu: Rc::default(),
            _bar: owned_bar,
        }
    }
}

impl BeuiFrame {
    pub fn content(&self) -> Option<NodeId> {
        self.content.get()
    }

    pub fn bar(&self) -> Signal<FrameBar> {
        self.bar
    }

    pub fn exit(&self) -> Rc<Cell<bool>> {
        self.exit.clone()
    }

    pub fn menu(&self) -> Rc<RefCell<Vec<MenuNode>>> {
        Rc::clone(&self.menu)
    }
}

#[component]
pub(crate) fn FrameChrome(
    chrome: Rc<BeuiFrame>,
    editor: Editor,
    view: Rc<dyn Fn() -> beui::styled::Root>,
) -> beui::styled::Root {
    let bar = chrome.bar;
    let exit_writer = chrome.exit.clone();
    let published = Rc::clone(&chrome.menu);
    let content_slot_writer = chrome.content.clone();
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
    let root: Rc<Cell<Option<beui::styled::Root>>> = Rc::default();
    let rooted = Rc::clone(&root);
    let frame = view! {
        <Layers>
            <PlacedHole editor />
            <List spacing=0.0>
                <FrameMenu
                    editor
                    bar={bar.read_only()}
                    palette={opener}
                    published
                    on_exit={move || exit_writer.set(true)}
                />
                <CommandPalette open={palette} />
                <ChromeRoot @sizing=ItemSize::Percent(100.0) phone>|| {
                    let built = view();
                    let content = built.child();
                    rooted.set(Some(built));
                    content_slot_writer.set(Some(content));
                    content
                }</ChromeRoot>
            </List>
        </Layers>
    };
    root.take()
        .expect("view() builds its root synchronously")
        .holding(frame)
}

pub(crate) fn run_menu_pick(menu: &RefCell<Vec<MenuNode>>, id: &str) {
    let picked = menu_action(&menu.borrow(), id);
    if let Some(action) = picked {
        untrack(|| action.run());
    }
}

pub(crate) fn menu_entries(nodes: &[MenuNode]) -> Vec<MenuEntry> {
    nodes
        .iter()
        .map(|node| match node {
            MenuNode::Action { action, label } => MenuEntry::Command(MenuCommand {
                id: action.id().to_owned(),
                label: label.clone().unwrap_or_else(|| action.label().get()),
                glyph: action
                    .glyph()
                    .map(|glyph| glyph.as_str().to_owned())
                    .unwrap_or_default(),
                enabled: action.enabled().get(),
                check: match (action.checked(), action.is_radio()) {
                    (None, _) => MenuCheck::None,
                    (Some(checked), false) => MenuCheck::Checkbox(checked.get()),
                    (Some(checked), true) => MenuCheck::Radio(checked.get()),
                },
                shortcut: action.shortcuts().first().map(|chord| chord.key_chord()),
                pin: match action.pinned() {
                    None => MenuPin::None,
                    Some(Pin::Icon) => MenuPin::Icon,
                    Some(Pin::Label) => MenuPin::Label,
                    Some(Pin::Title) => MenuPin::Title,
                },
            }),
            MenuNode::Submenu(submenu) => MenuEntry::Submenu(MenuSubmenu {
                id: submenu.id.clone(),
                label: submenu.label.clone(),
                glyph: submenu
                    .glyph
                    .map(|glyph| glyph.as_str().to_owned())
                    .unwrap_or_default(),
                entries: menu_entries(&submenu.items),
            }),
            MenuNode::Separator => MenuEntry::Separator,
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
        list.read().ok()?.into_iter().next()
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
        .glyph(ICON_UNDO_2)
        .pin(Pin::Icon)
        .shortcut(Chord::ctrl(Key::Z))
        .enabled(can(|state| state.can_undo))
        .register(),
        redo: Action::new("editor.redo", "Redo", move || {
            watched.with(|watched| watched.try_step(true));
        })
        .glyph(ICON_REDO_2)
        .pin(Pin::Icon)
        .shortcut(Chord::ctrl(Key::Z).shift())
        .shortcut(Chord::ctrl(Key::Y))
        .enabled(can(|state| state.can_redo))
        .register(),
        rename: Action::new("editor.rename", "Rename", move || {
            editor.host().rename_block(id, None)
        })
        .glyph(ICON_PENCIL_LINE)
        .pin(Pin::Title)
        .enabled(can(|state| state.can_rename))
        .register(),
        share: Action::new("editor.share", "Share", move || {
            editor.host().share_block(id)
        })
        .glyph(ICON_SHARE_2)
        .pin(Pin::Label)
        .enabled(can(|state| state.can_share))
        .register(),
        details: Action::new("editor.details", "File details", move || {
            editor.host().bar_action(BarAction::Details)
        })
        .glyph(ICON_INFO)
        .register(),
        close: Action::new("editor.close", "Close", move || on_exit.call())
            .glyph(ICON_X)
            .enabled(closable)
            .register(),
    }
}

#[component]
pub(crate) fn FrameMenu(
    editor: Editor,
    bar: ReadSignal<FrameBar>,
    palette: Action,
    published: Rc<RefCell<Vec<MenuNode>>>,
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
    let items = document_menu();
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
        let mut listed: Vec<MenuNode> = [&undo, &redo, &rename, &share, &palette]
            .into_iter()
            .map(MenuNode::action)
            .collect();
        listed.push(MenuNode::Separator);
        listed.extend(items.get());
        listed.push(MenuNode::Separator);
        listed.push(MenuNode::action(&details));
        if closable.get() {
            listed.push(MenuNode::action(&close));
        }
        tidy_menu(listed)
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

#[component]
fn PlacedHole(editor: Editor) -> NodeId {
    let placed = editor.placed_content();
    view! {
        <Container>|size: ReadSignal<Vec2>| {
            let strips = create_memo(move || hole_strips(size.get(), placed.get()));
            view! {
                <Canvas>
                    <ForEach keys={vec![0usize, 1, 2, 3]}>|strip: usize| {
                        let rect = create_memo(move || strips.get()[strip]);
                        let x = create_memo(move || rect.get().min.x);
                        let y = create_memo(move || rect.get().min.y);
                        let width = create_memo(move || rect.get().width().max(0.0));
                        let height = create_memo(move || rect.get().height().max(0.0));
                        view! {
                            <CanvasItem x={x} y={y} width={width} height={height} pointer=false>
                                <Embed slot={EmbedSlot::new()} />
                            </CanvasItem>
                        }
                    }</ForEach>
                </Canvas>
            }
        }</Container>
    }
}

fn hole_strips(size: Vec2, placed: Option<Rect>) -> [Rect; 4] {
    let Some(placed) = placed else {
        return [Rect::ZERO; 4];
    };
    let frame = Rect::from_min_size(Pos2::ZERO, size);
    let placed = placed.intersect(frame);
    [
        Rect::from_min_max(frame.min, pos2(frame.max.x, placed.min.y)),
        Rect::from_min_max(pos2(frame.min.x, placed.max.y), frame.max),
        Rect::from_min_max(
            pos2(frame.min.x, placed.min.y),
            pos2(placed.min.x, placed.max.y),
        ),
        Rect::from_min_max(
            pos2(placed.max.x, placed.min.y),
            pos2(frame.max.x, placed.max.y),
        ),
    ]
}
