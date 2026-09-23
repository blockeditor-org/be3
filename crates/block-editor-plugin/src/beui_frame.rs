use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui::NodeId;
use beui::icons::{ICON_REDO, ICON_SHARE, ICON_UNDO};
use beui::reactive::{
    ClickCallback, Frame, ItemSize, List, Memo, ReadSignal, Show, Spacer, WriteSignal, clone,
    component, create_memo, create_signal, view,
};
use beui::styled::{Button, ButtonVariant, IconButton, TextInput};
use beui::{Context, Document, Key};
use block_client::BlockHandleAccess;
use block_client::properties::{BlockName, MAX_NAME_BYTES, read_name};
use block_ui::{BlockLabel, BlockTypes};
use uuid::Uuid;

use crate::{Editor, Toolbar};

const BAR_SPACING: f32 = 6.0;
const NAME_WIDTH: f32 = 280.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameBar {
    pub shown: bool,
    pub closable: bool,
}

pub struct BeuiFrame {
    document: Document,
    content: NodeId,
    set_bar: WriteSignal<FrameBar>,
    exit: Rc<Cell<bool>>,
}

impl BeuiFrame {
    pub fn build(editor: &Editor, view: impl FnOnce() -> NodeId) -> Self {
        let (bar, set_bar) = create_signal(FrameBar::default());
        let exit = Rc::new(Cell::new(false));
        let exit_writer = exit.clone();
        let content_slot: Rc<Cell<Option<NodeId>>> = Rc::new(Cell::new(None));
        let content_slot_writer = content_slot.clone();
        let editor = editor.clone();
        let document = beui::reactive::build(move || {
            let content = view();
            content_slot_writer.set(Some(content));
            view! {
                <List spacing=0.0>
                    <TopBar editor bar on_exit={move || exit_writer.set(true)} />
                    {content} @sizing=ItemSize::Percent(100.0)
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
}

struct Watched {
    editor: Editor,
    handle: RefCell<Option<Box<dyn BlockHandleAccess>>>,
    watching: Cell<bool>,
}

impl Watched {
    fn block_type(&self) -> Option<Uuid> {
        self.editor.host().block_type().or_else(|| {
            self.editor
                .client()
                .cached_block(self.editor.block_id())
                .map(|cached| cached.block_type)
        })
    }

    fn name(&self) -> Option<BlockName> {
        self.with_handle(|handle| Some(handle.block_name()))
            .unwrap_or_else(|| {
                self.editor
                    .client()
                    .cached_block(self.editor.block_id())
                    .and_then(|cached| read_name(&cached.properties))
            })
    }

    fn read(&self) -> BarState {
        let id = self.editor.block_id();
        let client = self.editor.client();
        let host = self.editor.host();
        let types = self.editor.block_types();
        if !self.watching.replace(true) {
            host.watch_history([id]);
        }
        let block_type = self.block_type();
        let label = BlockLabel::new(
            types.as_ref(),
            block_type.unwrap_or_else(Uuid::nil),
            self.name().as_ref(),
        );
        let fallback = block_type
            .and_then(|block_type| types.display_name(block_type))
            .unwrap_or("Untitled")
            .to_owned();
        let editable = host.editable() && client.block_access(id).can_edit();
        let (can_undo, can_redo) = match editable {
            true => self.history(),
            false => (false, false),
        };
        let (name, placeholder) = match label.automatic {
            true => (String::new(), label.name),
            false => (label.name, fallback),
        };
        BarState {
            can_undo,
            can_redo,
            can_rename: editable,
            can_share: client.block_access(id).can_edit(),
            name,
            placeholder,
        }
    }

    fn history(&self) -> (bool, bool) {
        self.with_handle(|handle| {
            handle
                .history()
                .map(|history| (history.can_undo(), history.can_redo()))
        })
        .unwrap_or_else(|| {
            let history = self.editor.host().history(self.editor.block_id());
            (history.can_undo, history.can_redo)
        })
    }

    fn step(&self, redo: bool) {
        let handled = self.with_handle(|handle| {
            let history = handle.history()?;
            match redo {
                true => history.redo(),
                false => history.undo(),
            }
            Some(())
        });
        if handled.is_some() {
            return;
        }
        let id = self.editor.block_id();
        match redo {
            true => self.editor.host().redo(id),
            false => self.editor.host().undo(id),
        }
    }

    fn with_handle<R>(&self, read: impl FnOnce(&dyn BlockHandleAccess) -> Option<R>) -> Option<R> {
        let mut handle = self.handle.borrow_mut();
        if handle.is_none() {
            *handle = block_client::blocks::open(
                self.editor.client(),
                self.editor.block_id(),
                self.block_type()?,
            );
        }
        read(handle.as_deref()?)
    }

    fn rename(&self, typed: &str) {
        let name = typed.trim();
        if name.len() > MAX_NAME_BYTES {
            return;
        }
        let manual = self.name().filter(|current| current.manual);
        let next = match (name.is_empty(), manual) {
            (true, None) => return,
            (false, Some(current)) if current.value == name => return,
            (true, Some(_)) => None,
            (false, _) => Some(name.to_owned()),
        };
        let renamed = self.with_handle(|handle| {
            handle.set_name(next.clone());
            Some(())
        });
        if renamed.is_some() {
            return;
        }
        let id = self.editor.block_id();
        let client = self.editor.client();
        match next {
            Some(name) => client.set_block_name(id, name),
            None => client.clear_block_name(id),
        }
    }
}

#[component]
pub(crate) fn TopBar(editor: Editor, bar: ReadSignal<FrameBar>, on_exit: ClickCallback) -> NodeId {
    let shown = create_memo(clone!(bar -> move || bar.get().shown));
    let closable = create_memo(move || bar.get().closable);
    let (state, set_state) = create_signal(BarState::default());
    let watched = Rc::new(Watched {
        editor: editor.clone(),
        handle: RefCell::new(None),
        watching: Cell::new(false),
    });
    let reading = Rc::clone(&watched);
    let visible = shown.clone();
    editor.each_frame(move || {
        if visible.get_untracked() {
            set_state.set(reading.read());
        }
    });
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
    view! {
        <Toolbar shown={shown} spacing=BAR_SPACING>
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
            <Frame width=NAME_WIDTH>
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
            <Spacer @sizing=ItemSize::Percent(100.0) />
            <Button
                @test_id={"editor.share"}
                label="Share"
                glyph={ICON_SHARE.to_owned()}
                variant=ButtonVariant::Secondary
                disabled={share_off}
                on_click={move || shared.host().share_block(shared.block_id())}
            />
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
