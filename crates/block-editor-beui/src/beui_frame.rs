use std::cell::{Cell, RefCell};
use std::rc::Rc;

use be_block::metadata::MAX_NAME_BYTES;
use beui::NodeId;
use beui::icons::{
    ICON_CLOSE, ICON_DRIVE_FILE_RENAME_OUTLINE, ICON_INFO, ICON_MORE_VERT, ICON_REDO, ICON_SHARE,
    ICON_TERMINAL, ICON_UNDO,
};
use beui::reactive::{
    Action, Chord, ClickCallback, ForEach, Frame, ItemSize, List, Memo, ReadSignal, Show,
    WriteSignal, clone, component, create_effect, create_memo, create_signal, menu_actions,
    on_finger_tap, provide_context, use_context, view,
};
use beui::styled::{
    ActionRow, Button, ButtonVariant, CommandPalette, IconButton, MenuButton, ModalSheet, Scroll,
    TextInput,
};
use beui::unstyled::MenuItem;
use beui::{Context, Document, Key};
use block_plugin_api::BarAction;
use block_ui::{BlockLabel, BlockTypes};
use uuid::Uuid;

use crate::chrome::ChromeRoot;
use crate::{BlockInfo, BlockList, BlockQuery, Editor, Toolbar};

const BAR_SPACING: f32 = 6.0;
const NAME_WIDTH: f32 = 280.0;
const SHEET_PADDING: f32 = 8.0;
const MORE_STOPS: [f32; 2] = [0.5, 0.9];
const PALETTE: Chord = Chord::ctrl(Key::P).shift();

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameBar {
    pub shown: bool,
    pub closable: bool,
    pub on_phone: bool,
    pub more: bool,
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
    set_bar: WriteSignal<FrameBar>,
    exit: Rc<Cell<bool>>,
}

impl BeuiFrame {
    pub fn build(editor: &Editor, view: impl FnOnce() -> NodeId + 'static) -> Self {
        let (bar, set_bar) = create_signal(FrameBar::default());
        let exit = Rc::new(Cell::new(false));
        let exit_writer = exit.clone();
        let content_slot: Rc<Cell<Option<NodeId>>> = Rc::new(Cell::new(None));
        let content_slot_writer = content_slot.clone();
        let editor = editor.clone();
        let document = beui::reactive::build(move || {
            provide_context(PhoneLayout(create_memo(
                clone!(bar -> move || bar.get().on_phone),
            )));
            let phone = create_memo(clone!(bar -> move || bar.get().on_phone));
            let (palette, set_palette) = create_signal(false);
            let opening = set_palette.clone();
            let opener = Action::new("editor.palette", "Command palette", move || {
                opening.set(true)
            })
            .glyph(ICON_TERMINAL)
            .shortcut(PALETTE)
            .register();
            view! {
                <List spacing=0.0>
                    <TopBar editor bar palette={opener} on_exit={move || exit_writer.set(true)} />
                    <CommandPalette open={palette} on_close={move || set_palette.set(false)} />
                    <ChromeRoot @sizing=ItemSize::Percent(100.0) phone>
                        {move || {
                            let content = view();
                            content_slot_writer.set(Some(content));
                            content
                        }}
                    </ChromeRoot>
                </List>
            }
        });
        Self {
            document,
            content: content_slot
                .get()
                .expect("view() builds its root node synchronously"),
            set_bar,
            exit,
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

    pub fn set_bar(&self) -> WriteSignal<FrameBar> {
        self.set_bar.clone()
    }

    pub fn exit(&self) -> Rc<Cell<bool>> {
        self.exit.clone()
    }
}

#[derive(Clone, Default, PartialEq)]
struct BarState {
    can_undo: bool,
    can_redo: bool,
    can_rename: bool,
    can_share: bool,
    name: String,
    placeholder: String,
    type_name: String,
    glyph: String,
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

    fn block_type(&self) -> Option<Uuid> {
        self.editor
            .host()
            .block_type()
            .or_else(|| self.info().map(|info| info.block_type))
    }

    fn can_edit(&self) -> bool {
        self.info()
            .is_none_or(|info| info.access == crate::AccessLevel::Edit)
    }

    fn read(&self) -> BarState {
        let id = self.editor.block_id();
        let host = self.editor.host();
        let types = self.editor.block_types();
        if !self.watching.replace(true) {
            host.watch_history([id]);
        }
        let info = self.info();
        let block_type = self.block_type();
        let label = BlockLabel::new(
            types.as_ref(),
            block_type.unwrap_or_else(Uuid::nil),
            info.as_ref().and_then(|info| info.name.as_deref()),
            info.as_ref().is_some_and(|info| info.named_by_hand),
        );
        let fallback = block_type
            .and_then(|block_type| types.display_name(block_type))
            .unwrap_or("Untitled")
            .to_owned();
        let glyph = block_type
            .and_then(|block_type| types.icon(block_type))
            .unwrap_or_default()
            .to_owned();
        let editable = self.editor.editable().get() && self.can_edit();
        let (can_undo, can_redo) = match editable {
            true => self.history(),
            false => (false, false),
        };
        let (name, placeholder) = match label.automatic {
            true => (String::new(), label.name),
            false => (label.name, fallback.clone()),
        };
        BarState {
            can_undo,
            can_redo,
            can_rename: editable,
            can_share: self.can_edit(),
            name,
            placeholder,
            type_name: fallback,
            glyph,
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

    fn rename(&self, typed: &str) {
        let name = typed.trim();
        if name.len() > MAX_NAME_BYTES {
            return;
        }
        let manual = self
            .info()
            .filter(|info| info.named_by_hand)
            .and_then(|info| info.name);
        let next = match (name.is_empty(), manual) {
            (true, None) => return,
            (false, Some(current)) if current == name => return,
            (true, Some(_)) => None,
            (false, _) => Some(name.to_owned()),
        };
        self.editor.blocks().set_name(self.editor.block_id(), next);
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
    editor: &Editor,
    watched: &Rc<Watched>,
    state: &ReadSignal<BarState>,
    live: &Memo<bool>,
    closable: &Memo<bool>,
    on_exit: &ClickCallback,
) -> FrameActions {
    let host = editor.host().clone();
    let id = editor.block_id();
    let can = |read: fn(&BarState) -> bool| {
        create_memo(clone!(state live -> move || live.get() && state.with(read)))
    };
    FrameActions {
        undo: Action::new(
            "editor.undo",
            "Undo",
            clone!(watched -> move || {
                watched.try_step(false);
            }),
        )
        .glyph(ICON_UNDO)
        .shortcut(Chord::ctrl(Key::Z))
        .enabled(can(|state| state.can_undo))
        .register(),
        redo: Action::new(
            "editor.redo",
            "Redo",
            clone!(watched -> move || {
                watched.try_step(true);
            }),
        )
        .glyph(ICON_REDO)
        .shortcut(Chord::ctrl(Key::Z).shift())
        .shortcut(Chord::ctrl(Key::Y))
        .enabled(can(|state| state.can_redo))
        .register(),
        rename: Action::new(
            "editor.rename",
            "Rename",
            clone!(host -> move || host.rename_block(id)),
        )
        .glyph(ICON_DRIVE_FILE_RENAME_OUTLINE)
        .enabled(can(|state| state.can_rename))
        .register(),
        share: Action::new(
            "editor.share",
            "Share",
            clone!(host -> move || host.share_block(id)),
        )
        .glyph(ICON_SHARE)
        .enabled(can(|state| state.can_share))
        .register(),
        details: Action::new(
            "editor.details",
            "File details",
            clone!(host -> move || host.bar_action(BarAction::Details)),
        )
        .glyph(ICON_INFO)
        .register(),
        close: Action::new(
            "editor.close",
            "Close",
            clone!(on_exit -> move || on_exit.call()),
        )
        .glyph(ICON_CLOSE)
        .enabled(closable.clone())
        .register(),
    }
}

#[component]
pub(crate) fn TopBar(
    editor: Editor,
    bar: ReadSignal<FrameBar>,
    palette: Action,
    on_exit: ClickCallback,
) -> NodeId {
    let on_phone = create_memo(clone!(bar -> move || bar.get().on_phone));
    let shown = create_memo(clone!(bar on_phone -> move || bar.get().shown && !on_phone.get()));
    let closable = create_memo(clone!(bar -> move || bar.get().closable));
    let more = create_memo(clone!(bar on_phone -> move || on_phone.get() && bar.get().more));
    let live = create_memo(clone!(shown on_phone -> move || shown.get() || on_phone.get()));
    let (state, set_state) = create_signal(BarState::default());
    let watched = Rc::new(Watched {
        editor: editor.clone(),
        list: RefCell::new(None),
        watching: Cell::new(false),
    });
    let actions = frame_actions(&editor, &watched, &state, &live, &closable, &on_exit);
    let reading = Rc::clone(&watched);
    let visible = live.clone();
    let taps = Rc::clone(&watched);
    let tapping = live;
    on_finger_tap(move |fingers: usize| tapping.get_untracked() && taps.finger_tap(fingers));
    create_effect(move || {
        if visible.get() {
            set_state.set(reading.read());
        }
    });
    let phone_actions = actions.clone();
    view! {
        <List spacing=0.0>
            <DesktopBar watched state shown closable={closable.clone()} actions />
            <PhoneMore editor more closable actions={phone_actions} palette />
        </List>
    }
}

#[component]
fn DesktopBar(
    watched: Rc<Watched>,
    state: ReadSignal<BarState>,
    shown: Memo<bool>,
    closable: Memo<bool>,
    actions: FrameActions,
) -> NodeId {
    let rename_off = field(&state, |state| !state.can_rename);
    let name = field(&state, |state| state.name.clone());
    let placeholder = field(&state, |state| state.placeholder.clone());
    let submitted = Rc::clone(&watched);
    let left = Rc::clone(&watched);
    let typed = Rc::new(RefCell::new(None::<String>));
    let changed = Rc::clone(&typed);
    let blurred = Rc::clone(&typed);
    let items = menu_actions();
    let listed = items.clone();
    let offered = create_memo(move || listed.with(|items| !items.is_empty()));
    let FrameActions {
        undo,
        redo,
        share,
        close,
        ..
    } = actions;
    view! {
        <Toolbar shown={shown} spacing=BAR_SPACING fit=true>
            <IconButton @test_id={"editor.undo"} action={undo} />
            <IconButton @test_id={"editor.redo"} action={redo} />
            <Frame @sizing=ItemSize::Percent(100.0) max_width=NAME_WIDTH>
                <TextInput
                    @test_id={"editor.name"}
                    value={name}
                    placeholder={placeholder}
                    label="Block name"
                    plain=true
                    disabled={rename_off}
                    on_change={move |value: String| {
                        *changed.borrow_mut() = Some(value);
                    }}
                    on_submit={move |value: String| {
                        typed.borrow_mut().take();
                        submitted.rename(&value);
                    }}
                    on_focus_change={move |focused: bool| {
                        if focused {
                            return;
                        }
                        if let Some(value) = blurred.borrow_mut().take() {
                            left.rename(&value);
                        }
                    }}
                />
            </Frame>
            <Button @test_id={"editor.share"} variant=ButtonVariant::Secondary action={share} />
            <Show condition={offered}>
                <MoreMenu items />
            </Show>
            <Show condition={closable}>
                <Button
                    label="Close"
                    variant=ButtonVariant::Secondary
                    on_click={move || {
                        close.run();
                    }}
                    @test_id={"editor.close"}
                />
            </Show>
        </Toolbar>
    }
}

#[component]
fn MoreMenu(items: Memo<Vec<Action>>) -> NodeId {
    let keys = create_memo(clone!(items -> move || {
        items.with(|items| items.iter().map(Action::key).collect::<Vec<u64>>())
    }));
    let listed = items;
    view! {
        <MenuButton
            label="More"
            glyph={ICON_MORE_VERT.to_owned()}
            icon_only=true
            arrow=false
            @test_id={"editor.menu"}
            items={view! {
                <ForEach keys={keys}>
                    {move |key: u64| {
                        let item = listed.with_untracked(|items| {
                            items.iter().find(|action| action.key() == key).cloned()
                        });
                        match item {
                            Some(action) => view! {
                                <MenuItem action />
                            },
                            None => view! {
                                <MenuItem label="" disabled=true />
                            },
                        }
                    }}
                </ForEach>
            }}
        />
    }
}

#[component]
fn PhoneMore(
    editor: Editor,
    more: Memo<bool>,
    closable: Memo<bool>,
    actions: FrameActions,
    palette: Action,
) -> NodeId {
    let (dismissed, set_dismissed) = create_signal(false);
    let resetting = set_dismissed.clone();
    create_effect(clone!(more -> move || {
        if !more.get() {
            resetting.set(false);
        }
    }));
    let open = create_memo(clone!(dismissed -> move || more.get() && !dismissed.get()));
    let host = editor.host().clone();
    let done = ClickCallback::new(move || {
        if dismissed.get_untracked() {
            return;
        }
        set_dismissed.set(true);
        host.bar_action(BarAction::CloseMore);
    });
    let closing = done.clone();
    view! {
        <ModalSheet
            open={open}
            rest={MORE_STOPS[0]}
            stops={MORE_STOPS.to_vec()}
            on_close={move || closing.call()}
        >
            <MoreSheet closable actions palette done={move || done.call()} />
        </ModalSheet>
    }
}

#[component]
fn MoreSheet(
    closable: Memo<bool>,
    actions: FrameActions,
    palette: Action,
    done: ClickCallback,
) -> NodeId {
    let items = menu_actions();
    let keys = create_memo(clone!(items -> move || {
        items.with(|items| items.iter().map(Action::key).collect::<Vec<u64>>())
    }));
    let FrameActions {
        undo,
        redo,
        rename,
        share,
        details,
        close,
    } = actions;
    let row = |action: Action| {
        let done = done.clone();
        move || {
            done.call();
            action.run();
        }
    };
    let palette_run = row(palette.clone());
    let (undo_run, redo_run, rename_run, share_run, details_run, close_run) = (
        row(undo.clone()),
        row(redo.clone()),
        row(rename.clone()),
        row(share.clone()),
        row(details.clone()),
        row(close.clone()),
    );
    let running = done.clone();
    view! {
        <Scroll>
            <Frame padding_horizontal=SHEET_PADDING padding_vertical=SHEET_PADDING>
                <List spacing=0.0>
                    <SheetRow @test_id="editor.more.undo" action={undo} on_click={undo_run} />
                    <SheetRow @test_id="editor.more.redo" action={redo} on_click={redo_run} />
                    <SheetRow @test_id="editor.more.rename" action={rename} on_click={rename_run} />
                    <SheetRow @test_id="editor.more.share" action={share} on_click={share_run} />
                    <SheetRow
                        @test_id="editor.more.palette"
                        action={palette}
                        on_click={palette_run}
                    />
                    <ForEach keys={keys}>
                        {move |key: u64| {
                            let item = items.with_untracked(|items| {
                                items.iter().find(|action| action.key() == key).cloned()
                            });
                            let closing = running.clone();
                            match item {
                                Some(action) => {
                                    let test_id = format!("editor.more.item.{}", action.id());
                                    let running = action.clone();
                                    view! {
                                        <SheetRow
                                            @test_id={test_id}
                                            action
                                            on_click={move || {
                                                closing.call();
                                                running.run();
                                            }}
                                        />
                                    }
                                }
                                None => view! {
                                    <ActionRow label="" disabled=true />
                                },
                            }
                        }}
                    </ForEach>
                    <SheetRow
                        @test_id="editor.more.details"
                        action={details}
                        on_click={details_run}
                    />
                    <Show condition={closable}>
                        <SheetRow
                            @test_id="editor.more.close"
                            action={close}
                            on_click={close_run}
                        />
                    </Show>
                </List>
            </Frame>
        </Scroll>
    }
}

#[component]
fn SheetRow(action: Action, on_click: ClickCallback) -> NodeId {
    let label = action.label();
    let glyph = action.glyph().to_owned();
    let disabled = action.disabled();
    view! {
        <ActionRow label glyph disabled on_click={move || on_click.call()} />
    }
}

fn field<T: Clone + PartialEq + 'static>(
    state: &ReadSignal<BarState>,
    read: impl Fn(&BarState) -> T + 'static,
) -> Memo<T> {
    let state = state.clone();
    create_memo(move || state.with(&read))
}

pub(crate) fn escaped(context: &Context) -> bool {
    context.input(|input| {
        input.events.iter().any(|event| {
            matches!(
                event,
                beui::Event::Key {
                    key: Key::Escape,
                    pressed: true,
                    ..
                }
            )
        })
    })
}
