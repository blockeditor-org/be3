use std::cell::{Cell, RefCell};
use std::rc::Rc;

use be_block::metadata::MAX_NAME_BYTES;
use beui::NodeId;
use beui::accesskit::{Node as AccessNode, Role};
use beui::icons::{
    ICON_ARROW_BACK, ICON_CLOSE, ICON_DRIVE_FILE_RENAME_OUTLINE, ICON_EXPAND_MORE, ICON_INFO,
    ICON_MORE_VERT, ICON_REDO, ICON_SHARE, ICON_UNDO,
};
use beui::reactive::{
    Align, ClickCallback, Direction, ForEach, Frame, ItemSize, List, Memo, ReadSignal, Show, Text,
    WriteSignal, clone, component, create_effect, create_memo, create_signal, focus_ring,
    focus_takes_text, on_cleanup, on_finger_tap, on_shortcut, provide_context, use_context, view,
};
use beui::styled::theme::{FONT_BODY, FONT_SMALL};
use beui::styled::{
    ActionRow, Button, ButtonVariant, Caption, Icon, IconButton, ListRow, MenuButton, ModalSheet,
    Scroll, TextInput, use_theme,
};
use beui::unstyled;
use beui::unstyled::MenuItem;
use beui::{Context, Document, Key, KeyPress};
use block_plugin_api::BarAction;
use block_ui::{BlockLabel, BlockTypes};
use uuid::Uuid;

use crate::chrome::ChromeRoot;
use crate::{BlockInfo, BlockList, BlockQuery, Editor, Toolbar};

const BAR_SPACING: f32 = 6.0;
const PHONE_BAR_SPACING: f32 = 2.0;
const NAME_WIDTH: f32 = 280.0;
const TITLE_SPACING: f32 = 10.0;
const COUNT_SIDE: f32 = 22.0;
const COUNT_TARGET: f32 = 40.0;
const SHEET_PADDING: f32 = 8.0;
const MORE_STOPS: [f32; 2] = [0.5, 0.9];

#[derive(Clone)]
pub struct BarItem {
    pub label: String,
    pub glyph: String,
    pub disabled: Memo<bool>,
    pub run: Rc<dyn Fn()>,
}

#[derive(Clone)]
struct BarItems {
    items: ReadSignal<Vec<(u64, BarItem)>>,
    set_items: WriteSignal<Vec<(u64, BarItem)>>,
    next: Rc<Cell<u64>>,
}

impl BarItems {
    fn new() -> Self {
        let (items, set_items) = create_signal(Vec::new());
        Self {
            items,
            set_items,
            next: Rc::new(Cell::new(0)),
        }
    }
}

pub fn bar_item(
    label: impl Into<String>,
    glyph: &str,
    disabled: Memo<bool>,
    run: impl Fn() + 'static,
) {
    let Some(items) = use_context::<BarItems>() else {
        return;
    };
    let key = items.next.get();
    items.next.set(key + 1);
    let item = BarItem {
        label: label.into(),
        glyph: glyph.to_owned(),
        disabled,
        run: Rc::new(run),
    };
    items.set_items.update(|items| items.push((key, item)));
    let set_items = items.set_items;
    on_cleanup(move || set_items.update(|items| items.retain(|(other, _)| *other != key)));
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameBar {
    pub shown: bool,
    pub closable: bool,
    pub phone: Option<u32>,
    pub on_phone: bool,
    pub open_files: u32,
}

#[derive(Clone)]
struct PhoneLayout(Memo<bool>);

#[derive(Clone)]
struct OpenFileCount(Memo<u32>);

pub fn open_files() -> Memo<u32> {
    match use_context::<OpenFileCount>() {
        Some(OpenFileCount(count)) => count,
        None => create_memo(|| 0),
    }
}

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
            provide_context(BarItems::new());
            provide_context(PhoneLayout(create_memo(
                clone!(bar -> move || bar.get().on_phone),
            )));
            provide_context(OpenFileCount(create_memo(
                clone!(bar -> move || bar.get().open_files),
            )));
            let phone = create_memo(clone!(bar -> move || bar.get().phone.is_some()));
            view! {
                <List spacing=0.0>
                    <TopBar editor bar on_exit={move || exit_writer.set(true)} />
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

    fn shortcut(&self, press: KeyPress) -> bool {
        if !press.pressed || !press.modifiers.ctrl || press.modifiers.alt || focus_takes_text() {
            return false;
        }
        let redo = match press.key {
            Key::Z => press.modifiers.shift,
            Key::Y => true,
            _ => return false,
        };
        self.try_step(redo)
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

#[component]
pub(crate) fn TopBar(editor: Editor, bar: ReadSignal<FrameBar>, on_exit: ClickCallback) -> NodeId {
    let shown = create_memo(clone!(bar -> move || bar.get().shown));
    let closable = create_memo(clone!(bar -> move || bar.get().closable));
    let phone = create_memo(clone!(bar -> move || bar.get().phone));
    let desktop = create_memo(clone!(shown phone -> move || shown.get() && phone.get().is_none()));
    let phoned = create_memo(clone!(shown phone -> move || shown.get() && phone.get().is_some()));
    let (state, set_state) = create_signal(BarState::default());
    let watched = Rc::new(Watched {
        editor: editor.clone(),
        list: RefCell::new(None),
        watching: Cell::new(false),
    });
    let reading = Rc::clone(&watched);
    let visible = shown.clone();
    let shortcuts = Rc::clone(&watched);
    let active = shown.clone();
    on_shortcut(move |press: KeyPress| active.get_untracked() && shortcuts.shortcut(press));
    let taps = Rc::clone(&watched);
    let tapping = shown.clone();
    on_finger_tap(move |fingers: usize| tapping.get_untracked() && taps.finger_tap(fingers));
    create_effect(move || {
        if visible.get() {
            set_state.set(reading.read());
        }
    });
    let phone_watched = Rc::clone(&watched);
    let phone_state = state.clone();
    let phone_editor = editor.clone();
    let phone_exit = on_exit.clone();
    view! {
        <List spacing=0.0>
            <DesktopBar
                editor
                watched
                state
                shown={desktop}
                closable={closable.clone()}
                on_exit={move || on_exit.call()}
            />
            <PhoneBar
                editor={phone_editor}
                watched={phone_watched}
                state={phone_state}
                shown={phoned}
                open_files={create_memo(move || phone.get().unwrap_or(0))}
                closable
                on_exit={move || phone_exit.call()}
            />
        </List>
    }
}

#[component]
fn DesktopBar(
    editor: Editor,
    watched: Rc<Watched>,
    state: ReadSignal<BarState>,
    shown: Memo<bool>,
    closable: Memo<bool>,
    on_exit: ClickCallback,
) -> NodeId {
    let undo_off = field(&state, |state| !state.can_undo);
    let redo_off = field(&state, |state| !state.can_redo);
    let rename_off = field(&state, |state| !state.can_rename);
    let share_off = field(&state, |state| !state.can_share);
    let name = field(&state, |state| state.name.clone());
    let placeholder = field(&state, |state| state.placeholder.clone());
    let undo = Rc::clone(&watched);
    let redo = Rc::clone(&watched);
    let submitted = Rc::clone(&watched);
    let left = Rc::clone(&watched);
    let typed = Rc::new(RefCell::new(None::<String>));
    let changed = Rc::clone(&typed);
    let blurred = Rc::clone(&typed);
    let shared = editor.clone();
    let items = use_context::<BarItems>().map(|items| items.items);
    let offered = create_memo(move || {
        items
            .as_ref()
            .is_some_and(|items| items.with(|items| !items.is_empty()))
    });
    view! {
        <Toolbar shown={shown} spacing=BAR_SPACING fit=true>
            <IconButton
                @test_id={"editor.undo"}
                glyph={ICON_UNDO.to_owned()}
                label="Undo (Ctrl/Cmd+Z)"
                disabled={undo_off}
                on_click={move || undo.step(false)}
            />
            <IconButton
                @test_id={"editor.redo"}
                glyph={ICON_REDO.to_owned()}
                label="Redo (Ctrl+Y or Ctrl/Cmd+Shift+Z)"
                disabled={redo_off}
                on_click={move || redo.step(true)}
            />
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
            <Button
                @test_id={"editor.share"}
                label="Share"
                glyph={ICON_SHARE.to_owned()}
                variant=ButtonVariant::Secondary
                disabled={share_off}
                on_click={move || shared.host().share_block(shared.block_id())}
            />
            <Show condition={offered}>
                <MoreMenu />
            </Show>
            <Show condition={closable}>
                <Button
                    label="Close"
                    variant=ButtonVariant::Secondary
                    on_click={move || on_exit.call()}
                    @test_id={"editor.close"}
                />
            </Show>
        </Toolbar>
    }
}

#[component]
fn MoreMenu() -> NodeId {
    let Some(BarItems { items, .. }) = use_context::<BarItems>() else {
        return view! {
            <List spacing=0.0 />
        };
    };
    let keys = create_memo(clone!(items -> move || {
        items.with(|items| items.iter().map(|(key, _)| *key).collect::<Vec<u64>>())
    }));
    let listed = items.clone();
    let chosen = move |path: Vec<usize>| {
        let Some(index) = path.first().copied() else {
            return;
        };
        let run =
            items.with_untracked(|items| items.get(index).map(|(_, item)| Rc::clone(&item.run)));
        if let Some(run) = run {
            run();
        }
    };
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
                            items
                                .iter()
                                .find(|(other, _)| *other == key)
                                .map(|(_, item)| item.clone())
                        });
                        match item {
                            Some(BarItem { label, disabled, .. }) => view! {
                                <MenuItem label disabled />
                            },
                            None => view! {
                                <MenuItem label="" disabled=true />
                            },
                        }
                    }}
                </ForEach>
            }}
            on_select={chosen}
        />
    }
}

#[component]
fn PhoneBar(
    editor: Editor,
    watched: Rc<Watched>,
    state: ReadSignal<BarState>,
    shown: Memo<bool>,
    open_files: Memo<u32>,
    closable: Memo<bool>,
    on_exit: ClickCallback,
) -> NodeId {
    let theme = use_theme();
    let title = field(&state, |state| match state.name.is_empty() {
        true => state.placeholder.clone(),
        false => state.name.clone(),
    });
    let kind = field(&state, |state| state.type_name.clone());
    let glyph = field(&state, |state| state.glyph.clone());
    let (more, set_more) = create_signal(false);
    let host = editor.host().clone();
    let back = clone!(host -> move || host.bar_action(BarAction::Back));
    let switch = clone!(host -> move || host.bar_action(BarAction::Switch));
    let counted = clone!(host -> move || host.bar_action(BarAction::Switch));
    let opening = set_more.clone();
    let closing = set_more.clone();
    view! {
        <List spacing=0.0>
            <Toolbar shown={shown} spacing=PHONE_BAR_SPACING fit=true>
                <IconButton
                    @test_id={"editor.back"}
                    glyph={ICON_ARROW_BACK.to_owned()}
                    label="Back to files"
                    on_click={back}
                />
                <ListRow
                    @sizing=ItemSize::Percent(100.0)
                    @test_id={"editor.switch"}
                    on_click={switch}
                >
                    <List direction=Direction::Horizontal align=Align::Center spacing=TITLE_SPACING>
                        <Icon glyph color={theme.accent.clone()} />
                        <List @sizing=ItemSize::Percent(100.0) spacing=0.0>
                            <Text
                                string={title}
                                font_size=FONT_BODY
                                color={theme.text.clone()}
                                ellipsis=true
                            />
                            <Caption content={kind} ellipsis=true />
                        </List>
                        <Icon
                            glyph={ICON_EXPAND_MORE.to_owned()}
                            color={theme.text_muted.clone()}
                        />
                    </List>
                </ListRow>
                <OpenFiles count={open_files} on_click={counted} id={"editor.files".to_owned()} />
                <IconButton
                    @test_id={"editor.more"}
                    glyph={ICON_MORE_VERT.to_owned()}
                    label="More"
                    on_click={move || opening.set(true)}
                />
            </Toolbar>
            <ModalSheet
                open={more}
                rest={MORE_STOPS[0]}
                stops={MORE_STOPS.to_vec()}
                on_close={move || closing.set(false)}
            >
                <MoreSheet
                    editor
                    watched
                    state
                    closable
                    on_exit={move || on_exit.call()}
                    set_more
                />
            </ModalSheet>
        </List>
    }
}

#[component]
pub fn OpenFiles(count: Memo<u32>, on_click: ClickCallback, id: String) -> NodeId {
    let theme = use_theme();
    let label = create_memo(clone!(count -> move || count.get().to_string()));
    let accessibility = create_memo(clone!(count -> move || {
        let mut node = AccessNode::new(Role::Button);
        node.set_label(match count.get() {
            1 => "1 open file".to_owned(),
            count => format!("{count} open files"),
        });
        node
    }));
    view! {
        <unstyled::Button
            @test_id={id}
            accessibility
            on_click={move || on_click.call()}
            content={move |handle: unstyled::ButtonHandle| {
                let unstyled::ButtonHandle { hovered, focused, .. } = handle;
                let theme = theme.clone();
                let fill = create_memo(clone!(theme -> move || match hovered.get() {
                    true => theme.hover.get(),
                    false => beui::Color32::TRANSPARENT,
                }));
                let label = label.clone();
                view! {
                    <Frame
                        width=COUNT_TARGET
                        height=COUNT_TARGET
                        color={fill}
                        radius=8
                        outline={theme.accent.clone()}
                        outline_width=2.0
                        outline_visible={focus_ring(focused)}
                    >
                        <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                            <Frame @sizing=ItemSize::Percent(50.0) />
                            <Frame
                                width=COUNT_SIDE
                                height=COUNT_SIDE
                                radius=6
                                outline={theme.text.clone()}
                                outline_width=2.0
                                outline_visible=true
                            >
                                <Text
                                    string={label}
                                    font_size=FONT_SMALL
                                    color={theme.text.clone()}
                                    align=beui::TextAlign::Center
                                />
                            </Frame>
                            <Frame @sizing=ItemSize::Percent(50.0) />
                        </List>
                    </Frame>
                }
            }}
        />
    }
}

#[component]
fn MoreSheet(
    editor: Editor,
    watched: Rc<Watched>,
    state: ReadSignal<BarState>,
    closable: Memo<bool>,
    on_exit: ClickCallback,
    set_more: WriteSignal<bool>,
) -> NodeId {
    let undo_off = field(&state, |state| !state.can_undo);
    let redo_off = field(&state, |state| !state.can_redo);
    let rename_off = field(&state, |state| !state.can_rename);
    let share_off = field(&state, |state| !state.can_share);
    let items = use_context::<BarItems>().map(|items| items.items);
    let keys = create_memo(clone!(items -> move || {
        items
            .as_ref()
            .map(|items| items.with(|items| items.iter().map(|(key, _)| *key).collect()))
            .unwrap_or_default()
    }));
    let host = editor.host().clone();
    let id = editor.block_id();
    let undo = clone!(watched set_more -> move || {
        set_more.set(false);
        watched.step(false);
    });
    let redo = clone!(watched set_more -> move || {
        set_more.set(false);
        watched.step(true);
    });
    let rename = clone!(host set_more -> move || {
        set_more.set(false);
        host.rename_block(id);
    });
    let share = clone!(host set_more -> move || {
        set_more.set(false);
        host.share_block(id);
    });
    let details = clone!(host set_more -> move || {
        set_more.set(false);
        host.bar_action(BarAction::Details);
    });
    let leave = clone!(set_more -> move || {
        set_more.set(false);
        on_exit.call();
    });
    let running = set_more.clone();
    view! {
        <Scroll>
            <Frame padding_horizontal=SHEET_PADDING padding_vertical=SHEET_PADDING>
                <List spacing=0.0>
                    <ActionRow
                        @test_id={"editor.more.undo"}
                        label="Undo"
                        glyph={ICON_UNDO.to_owned()}
                        disabled={undo_off}
                        on_click={undo}
                    />
                    <ActionRow
                        @test_id={"editor.more.redo"}
                        label="Redo"
                        glyph={ICON_REDO.to_owned()}
                        disabled={redo_off}
                        on_click={redo}
                    />
                    <ActionRow
                        @test_id={"editor.more.rename"}
                        label="Rename"
                        glyph={ICON_DRIVE_FILE_RENAME_OUTLINE.to_owned()}
                        disabled={rename_off}
                        on_click={rename}
                    />
                    <ActionRow
                        @test_id={"editor.more.share"}
                        label="Share"
                        glyph={ICON_SHARE.to_owned()}
                        disabled={share_off}
                        on_click={share}
                    />
                    <ForEach keys={keys}>
                        {move |key: u64| {
                            let item = items.as_ref().and_then(|items| {
                                items.with_untracked(|items| {
                                    items
                                        .iter()
                                        .find(|(other, _)| *other == key)
                                        .map(|(_, item)| item.clone())
                                })
                            });
                            let BarItem { label, glyph, disabled, run } = item.unwrap_or(BarItem {
                                label: String::new(),
                                glyph: String::new(),
                                disabled: create_memo(|| true),
                                run: Rc::new(|| {}),
                            });
                            let closing = running.clone();
                            view! {
                                <ActionRow
                                    @test_id={format!("editor.more.item.{key}")}
                                    label
                                    glyph
                                    disabled
                                    on_click={move || {
                                        closing.set(false);
                                        run();
                                    }}
                                />
                            }
                        }}
                    </ForEach>
                    <ActionRow
                        @test_id={"editor.more.details"}
                        label="File details"
                        glyph={ICON_INFO.to_owned()}
                        on_click={details}
                    />
                    <Show condition={closable}>
                        <ActionRow
                            @test_id={"editor.more.close"}
                            label="Close"
                            glyph={ICON_CLOSE.to_owned()}
                            on_click={leave}
                        />
                    </Show>
                </List>
            </Frame>
        </Scroll>
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
