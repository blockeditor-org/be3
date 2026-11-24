use be_lib_stable_hash::HashMap;
use beui::NodeId;
use beui::icons::Glyph;
use beui::native::{Embed, EmbedSlot};
use beui::reactive::*;
use beui::styled::*;
use std::cell::RefCell;

use be_sdk_api::{
    ChildId, ChildLayer, ChildMode, ChildPreview, DialogFrame, DockTabPath, KeyChord, MenuCheck,
    MenuEntry, MenuPin, TopBar,
};
use uuid::Uuid;

use crate::{ChildState, ChildTarget, Editor, SubregionContent, watch_block_label};
use beui::native::draw_gpu;
use beui::{Align, Align2};

#[derive(Clone, Copy)]
pub struct ChildDockTab {
    pub menu: Memo<Vec<MenuNode>>,
    pub body: NodeId,
    pub owner: Option<Uuid>,
}

#[derive(Clone)]
pub struct ChildBlockHandle {
    pub state: ReadSignal<ChildState>,
    pub menu: Memo<Vec<MenuNode>>,
}

#[component]
pub fn ChildBlock(
    editor: Editor,
    block: Prop<Option<ChildTarget>>,
    #[prop(default = ChildMode::Passive)] mode: Prop<ChildMode>,
    #[prop(default = ChildLayer::Below)] layer: Prop<ChildLayer>,
    #[prop(default = false)] own_frame: Prop<bool>,
    #[prop(default = TopBar::Hidden)] top_bar: Prop<TopBar>,
    #[prop(default = true)] punch: Prop<bool>,
    #[prop(default = 0.0)] rotation: Prop<f32>,
    #[prop(default = 1.0)] opacity: Prop<f32>,
    #[prop(default = None)] intrinsic: Prop<Option<beui::Vec2>>,
    /// How many times the editor asked that the child take the keyboard, a count that only grows:
    /// each time it grows above the highest it reached, the host moves the keyboard to the child
    /// at the layout commit that carries it, if the keyboard is within this editor's views and
    /// no view that takes the keyboard is open.
    #[prop(default = 0)]
    keyboard: Prop<u64>,
    on_state: Callback<ChildState>,
    on_details: Callback<()>,
    on_focus: Callback<()>,
    docked_tab: Option<RenderFn<ChildDockTab, NodeId>>,
    #[prop(children)] content: Option<Render<ChildBlockHandle, NodeId>>,
) -> NodeId {
    let placed = create_memo(move || block.get().map(SubregionContent::Block));
    let placing = Placing {
        claims: true,
        docked_tab,
        mode,
        layer,
        own_frame,
        top_bar,
        punch,
        rotation,
        opacity,
        radius: Prop::Static(0.0),
        intrinsic,
        fit: Prop::Static(None),
        dialog: Prop::Static(None),
        fullscreen: Prop::Static(false),
        keyboard,
        on_state,
        on_details,
        on_focus,
        content,
    };
    view! {
        <SubregionView editor={editor} placed={placed} placing={placing} />
    }
}

#[component]
pub fn Subregion(
    editor: Editor,
    placed: Prop<Option<SubregionContent>>,
    #[prop(default = ChildMode::Passive)] mode: Prop<ChildMode>,
    #[prop(default = ChildLayer::Below)] layer: Prop<ChildLayer>,
    #[prop(default = false)] own_frame: Prop<bool>,
    #[prop(default = TopBar::Hidden)] top_bar: Prop<TopBar>,
    #[prop(default = true)] punch: Prop<bool>,
    #[prop(default = 0.0)] rotation: Prop<f32>,
    #[prop(default = 1.0)] opacity: Prop<f32>,
    #[prop(default = 0.0)] radius: Prop<f32>,
    #[prop(default = None)] intrinsic: Prop<Option<beui::Vec2>>,
    #[prop(default = None)] fit: Prop<Option<beui::Vec2>>,
    #[prop(default = false)] dialog: bool,
    #[prop(default = false)] fullscreen: Prop<bool>,
    /// How many times the editor asked that the child take the keyboard (`ChildBlock`'s).
    #[prop(default = 0)]
    keyboard: Prop<u64>,
    on_state: Callback<ChildState>,
    on_details: Callback<()>,
    on_focus: Callback<()>,
    #[prop(children)] content: Option<Render<ChildBlockHandle, NodeId>>,
) -> NodeId {
    let layout = use_context::<DialogLayout>();
    let modal = use_modal();
    let dialog = match dialog {
        true => Prop::dynamic(move || {
            Some(DialogFrame {
                narrow: layout.is_some_and(|layout| layout.narrow.get()),
                bottom_inset: layout.map_or(0.0, |layout| layout.bottom_inset.get()),
                modal: modal.get(),
            })
        }),
        false => Prop::Static(None),
    };
    let placing = Placing {
        claims: false,
        docked_tab: None,
        mode,
        layer,
        own_frame,
        top_bar,
        punch,
        rotation,
        opacity,
        radius,
        intrinsic,
        fit,
        dialog,
        fullscreen,
        keyboard,
        on_state,
        on_details,
        on_focus,
        content,
    };
    view! {
        <SubregionView editor={editor} placed={placed} placing={placing} />
    }
}

struct Placing {
    claims: bool,
    docked_tab: Option<RenderFn<ChildDockTab, NodeId>>,
    mode: Prop<ChildMode>,
    layer: Prop<ChildLayer>,
    own_frame: Prop<bool>,
    top_bar: Prop<TopBar>,
    punch: Prop<bool>,
    rotation: Prop<f32>,
    opacity: Prop<f32>,
    radius: Prop<f32>,
    intrinsic: Prop<Option<beui::Vec2>>,
    fit: Prop<Option<beui::Vec2>>,
    dialog: Prop<Option<DialogFrame>>,
    fullscreen: Prop<bool>,
    keyboard: Prop<u64>,
    on_state: Callback<ChildState>,
    on_details: Callback<()>,
    on_focus: Callback<()>,
    content: Option<Render<ChildBlockHandle, NodeId>>,
}

#[component]
fn SubregionView(
    editor: Editor,
    placed: Prop<Option<SubregionContent>>,
    placing: Placing,
) -> NodeId {
    let Placing {
        claims,
        docked_tab,
        mode,
        layer,
        own_frame,
        top_bar,
        punch,
        rotation,
        opacity,
        radius,
        intrinsic,
        fit,
        dialog,
        fullscreen,
        keyboard,
        on_state,
        on_details,
        on_focus,
        content,
    } = placing;
    let slot = EmbedSlot::new();
    let placed = create_memo(move || placed.get());
    let layer = create_memo(move || layer.get());
    let below = create_memo(move || matches!(layer.get(), ChildLayer::Below));
    let mode = create_memo(move || mode.get());
    let turn = create_memo(move || rotation.get());
    let fade = create_memo(move || opacity.get());
    let docked = beui::dock::use_dock_tab().filter(|tab| tab.is_hosted());
    let join = match docked {
        Some(_) => None,
        None => beui::dock::use_dock_join(),
    };
    let dock_tab = docked
        .as_ref()
        .map(|docked| docked.tab().value())
        .or_else(|| join.as_ref().map(|join| join.host().value()));
    let claims = claims && own_frame.get_untracked();
    let (docks, link) = match (claims, &docked, &join) {
        (false, _, _) => (false, None),
        (true, Some(docked), _) => (docked.claim_body(), None),
        (true, None, Some(join)) => match join.claim() {
            Some(link) => (true, Some(link)),
            None => (false, None),
        },
        (true, None, None) => (false, None),
    };
    let tab = beui::dock::use_dock_tab();
    let pane = beui::dock::use_dock_pane();
    let on_focus = Callback::new(move |()| {
        if let Some(pane) = pane {
            pane.focus();
        }
        if let Some(tab) = tab {
            tab.content_focused();
        }
        on_focus.call(());
    });
    // The editor's own asks, and one each time the dock gives this child's tab the keyboard.
    let tab_asks = match tab {
        Some(tab) => tab_keyboard_asks(editor, tab, placed, mode),
        None => create_memo(|| 0),
    };
    // Both counts only grow, so their sum does: the editor's own count is taken as how many
    // times it grew, so neither can hide the other's growth.
    let own_asks = growths(keyboard);
    let keyboard = Prop::dynamic(move || own_asks.get() + tab_asks.get());
    let state = editor.register_child(
        slot,
        placed.into_prop(),
        mode.into_prop(),
        layer.into_prop(),
        own_frame,
        top_bar,
        turn.into_prop(),
        fade.into_prop(),
        intrinsic,
        fit,
        dialog,
        fullscreen,
        keyboard,
        on_state,
        on_details,
        on_focus,
        dock_tab,
        docks,
        tab,
    );
    if let Some(link) = link {
        crate::docked::bridge_child_dock(editor, link, state, docked_tab);
    }
    let preview = create_memo(move || state.with(|state| state.preview));
    // A preview the host composites in place (a root's, `ChildPreview::Composited`) takes a
    // hole as any child does; one it hands over as a picture, or not yet, is drawn here.
    let pictured = create_memo(move || match preview.get() {
        ChildPreview::Composited => false,
        ChildPreview::Hosted => mode.get() == ChildMode::Preview,
        ChildPreview::Waiting | ChildPreview::Label | ChildPreview::Image(_) => true,
    });
    let punch = create_memo(move || below.get() && punch.get() && !pictured.get());
    let entries = create_memo(move || state.with(|state| (state.child, state.menu.clone())));
    let mirror = store(RefCell::new(MenuMirror::new(editor)));
    let nodes = create_signal(Vec::<MenuNode>::new());
    create_effect(move || {
        let (child, entries) = entries.get();
        let rebuilt = untrack(|| mirror.with(|mirror| mirror.borrow_mut().update(child, &entries)));
        if let Some(rebuilt) = rebuilt {
            nodes.set(rebuilt);
        }
    });
    let menu = create_memo(move || nodes.get());
    let framing = create_memo(move || {
        state.with(|state| state.active && !state.menu.is_empty())
            && matches!(mode.get(), ChildMode::Active | ChildMode::Live)
            && !own_frame.get()
    });
    let key = framing_key();
    create_effect(move || {
        let taken = framing.get().then_some(menu);
        untrack(|| editor.hold_frame_child_menu(key, taken));
    });
    on_cleanup(move || editor.hold_frame_child_menu(key, None));
    let handle = ChildBlockHandle { state, menu };
    let picture = view! {
        <PreviewPicture editor={editor} placed={placed} preview={preview} turn={turn} fade={fade} />
    };
    let inner = match content.map(|content| content.call(handle)) {
        Some(child) => view! {
            <Layers>{picture}{child}</Layers>
        },
        None => picture,
    };
    view! {
        <Embed slot={slot} punch={punch} rotation={turn} radius={radius} children={Some(inner)} />
    }
}

/// How many times `count` grew above the highest it reached.
fn growths(count: Prop<u64>) -> ReadSignal<u64> {
    let grown = create_signal(0_u64);
    let highest = StoredCell::new(0_u64);
    create_effect(move || {
        let now = count.get();
        if now > highest.get() {
            highest.set(now);
            untrack(|| grown.update(|grown| *grown += 1));
        }
    });
    grown.read_only()
}

/// Counts the times the dock gives `tab` the keyboard, or brings its window forward, while the
/// child it shows takes input (an active or live block, a dock tab's body, a program window) and
/// no popover or keyboard-taking layer of the editor's is open: each is an ask that the child
/// take the keyboard, which the child's placement carries (`keyboard-asks`).
fn tab_keyboard_asks(
    editor: Editor,
    tab: beui::dock::DockTabControl,
    placed: Memo<Option<SubregionContent>>,
    mode: Memo<ChildMode>,
) -> Memo<u64> {
    let given = create_memo(move || {
        let given = tab.keyboard().map(|count| (false, count));
        let forward = tab.brought_forward().map(|count| (true, count));
        given.into_iter().chain(forward).collect::<Vec<_>>()
    });
    let interactive = create_memo(move || {
        matches!(mode.get(), ChildMode::Active | ChildMode::Live)
            && placed.with(|placed| {
                matches!(
                    placed,
                    Some(
                        SubregionContent::Block(_)
                            | SubregionContent::DockTab(_)
                            | SubregionContent::Window(_)
                    )
                )
            })
    });
    let asks = create_signal(0_u64);
    let asked = StoredCell::new(Vec::<(bool, u64)>::new());
    create_effect(move || {
        let given = given.get();
        if !interactive.get() {
            return;
        }
        let held = asked.replace(given.clone());
        let trapped = editor.popped().with_untracked(Option::is_some)
            || beui::native::try_with_document(|document| document.layers().takes_keyboard())
                .unwrap_or(false);
        if !trapped && given.iter().any(|given| !held.contains(given)) {
            untrack(|| asks.update(|asks| *asks += 1));
        }
    });
    create_memo(move || asks.get())
}

#[component]
fn PreviewPicture(
    editor: Editor,
    placed: Memo<Option<SubregionContent>>,
    preview: Memo<ChildPreview>,
    turn: Memo<f32>,
    fade: Memo<f32>,
) -> NodeId {
    let drawing = create_memo(move || match preview.get() {
        ChildPreview::Image(image) => Some(crate::preview::picture(image, turn.get(), fade.get())),
        ChildPreview::Hosted
        | ChildPreview::Composited
        | ChildPreview::Waiting
        | ChildPreview::Label => None,
    });
    let target = create_memo(move || match placed.get() {
        Some(SubregionContent::Block(target)) => Some(target),
        _ => None,
    });
    view! {
        <Layers>
            <Drawing draw={move || draw_gpu(drawing.get())} />
            <Show value={move || preview.get() == ChildPreview::Label}>
                <PreviewLabel editor={editor} target={target} />
            </Show>
        </Layers>
    }
}

#[component]
fn PreviewLabel(editor: Editor, target: Memo<Option<ChildTarget>>) -> NodeId {
    let shown = watch_block_label(editor, target);
    let name = create_memo(move || shown.with(|shown| shown.name.clone()));
    let italic = create_memo(move || shown.with(|shown| shown.automatic));
    let glyph = create_memo(move || shown.with(|shown| shown.glyph));
    view! {
        <Frame
            fill=ColorRole::Surface
            border={Border::new(ColorRole::Border)}
            radius=Radius::Md
            padding=Space::Lg
            align=Align2::CENTER
        >
            <List spacing=Space::Md align=Align::Center>
                <Show value={move || glyph.with(Option::is_some)}>
                    <Text size=TextSize::Xl color=TextColor::Muted align=beui::TextAlign::Center>
                        <Icon glyph={glyph} />
                    </Text>
                </Show>
                <Text color=TextColor::Muted align=beui::TextAlign::Center italic={italic}>
                    {name}
                </Text>
            </List>
        </Frame>
    }
}

#[derive(Clone, PartialEq)]
enum Shape {
    Command {
        id: String,
        label: Option<String>,
        glyph: Option<Glyph>,
        check: Option<bool>,
        shortcut: Option<KeyChord>,
        pin: MenuPin,
    },
    Submenu {
        id: String,
        label: String,
        glyph: Option<Glyph>,
        entries: Vec<Shape>,
    },
    Separator,
}

struct Mirrored {
    label: OwnedSignal<String>,
    enabled: OwnedSignal<bool>,
    checked: OwnedSignal<bool>,
}

#[derive(Clone)]
pub(crate) enum MenuPicks {
    Child,
    DockTab(DockTabPath),
}

pub(crate) struct MenuMirror {
    editor: Editor,
    picks: MenuPicks,
    commands: HashMap<String, Mirrored>,
    shape: Option<(Option<ChildId>, Vec<Shape>)>,
}

impl MenuMirror {
    pub(crate) fn new(editor: Editor) -> Self {
        Self::picking(editor, MenuPicks::Child)
    }

    pub(crate) fn picking(editor: Editor, picks: MenuPicks) -> Self {
        Self {
            editor,
            picks,
            commands: HashMap::default(),
            shape: None,
        }
    }

    pub(crate) fn update(
        &mut self,
        child: Option<ChildId>,
        entries: &[MenuEntry],
    ) -> Option<Vec<MenuNode>> {
        let entries = match child {
            Some(_) => entries,
            None => &[],
        };
        let shape = (child, self.shape_of(entries));
        if self.shape.as_ref() == Some(&shape) {
            return None;
        }
        let nodes = match child {
            Some(child) => self.nodes(child, &shape.1),
            None => Vec::new(),
        };
        self.shape = Some(shape);
        Some(nodes)
    }

    fn shape_of(&mut self, entries: &[MenuEntry]) -> Vec<Shape> {
        entries
            .iter()
            .map(|entry| match entry {
                MenuEntry::Command(command) => {
                    let (check, checked) = match command.check {
                        MenuCheck::None => (None, false),
                        MenuCheck::Checkbox(checked) => (Some(false), checked),
                        MenuCheck::Radio(checked) => (Some(true), checked),
                    };
                    let mirrored =
                        self.commands
                            .entry(command.id.clone())
                            .or_insert_with(|| Mirrored {
                                label: OwnedSignal::new(String::new()),
                                enabled: OwnedSignal::new(false),
                                checked: OwnedSignal::new(false),
                            });
                    let (label, enabled, ticked) = (
                        mirrored.label.handle(),
                        mirrored.enabled.handle(),
                        mirrored.checked.handle(),
                    );
                    label.set(command.label.clone());
                    enabled.set(command.enabled);
                    ticked.set(checked);
                    Shape::Command {
                        id: command.id.clone(),
                        label: None,
                        glyph: beui::icons::named(&command.glyph),
                        check,
                        shortcut: command.shortcut,
                        pin: command.pin,
                    }
                }
                MenuEntry::Submenu(submenu) => Shape::Submenu {
                    id: submenu.id.clone(),
                    label: submenu.label.clone(),
                    glyph: beui::icons::named(&submenu.glyph),
                    entries: self.shape_of(&submenu.entries),
                },
                MenuEntry::Separator => Shape::Separator,
            })
            .collect()
    }

    fn nodes(&self, child: ChildId, shape: &[Shape]) -> Vec<MenuNode> {
        shape
            .iter()
            .filter_map(|shape| match shape {
                Shape::Command {
                    id,
                    glyph,
                    check,
                    shortcut,
                    pin,
                    ..
                } => {
                    let mirrored = self.commands.get(id)?;
                    let (label, enabled, checked) = (
                        mirrored.label.handle(),
                        mirrored.enabled.handle(),
                        mirrored.checked.handle(),
                    );
                    let host = self.editor.host();
                    let picked = id.clone();
                    let picks = self.picks.clone();
                    let mut action = Action::new(id.clone(), label, move || match &picks {
                        MenuPicks::Child => host.pick_child_menu(child, picked.clone()),
                        MenuPicks::DockTab(tab) => {
                            host.pick_child_dock_tab(child, tab.clone(), picked.clone())
                        }
                    })
                    .glyph(*glyph)
                    .enabled(enabled);
                    action = match check {
                        None => action,
                        Some(false) => action.checked(checked),
                        Some(true) => action.radio(checked),
                    };
                    if let Some(shortcut) = shortcut {
                        action = action.shortcut(Chord::from(*shortcut));
                    }
                    action = match pin {
                        MenuPin::None => action,
                        MenuPin::Icon => action.pin(Pin::Icon),
                        MenuPin::Label => action.pin(Pin::Label),
                        MenuPin::Title => action.pin(Pin::Title),
                    };
                    Some(MenuNode::action(&action.detached()))
                }
                Shape::Submenu {
                    id,
                    label,
                    glyph,
                    entries,
                } => Some(MenuNode::submenu(
                    id.clone(),
                    label.clone(),
                    *glyph,
                    self.nodes(child, entries),
                )),
                Shape::Separator => Some(MenuNode::Separator),
            })
            .collect()
    }
}

fn framing_key() -> u64 {
    thread_local! {
        static NEXT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    }
    NEXT.with(|next| {
        let key = next.get() + 1;
        next.set(key);
        key
    })
}
