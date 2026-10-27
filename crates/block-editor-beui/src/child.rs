use beui::NodeId;
use beui::icons::Glyph;
use beui::native::{Embed, EmbedSlot};
use beui::reactive::*;
use beui::styled::*;
use std::cell::RefCell;
use std::collections::HashMap;

use block_plugin_api::{
    BarAction, ChildId, ChildLayer, ChildMode, ChildPreview, DialogFrame, DockTabPath, KeyChord,
    MenuCheck, MenuEntry, MenuPin, TopBar,
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
pub struct ChildHandle {
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
    on_state: Callback<ChildState>,
    on_bar: Callback<BarAction>,
    on_focus: Callback<()>,
    docked_tab: Option<RenderFn<ChildDockTab, NodeId>>,
    #[prop(children)] content: Option<Render<ChildHandle>>,
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
        intrinsic,
        dialog: Prop::Static(None),
        on_state,
        on_bar,
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
    #[prop(default = None)] intrinsic: Prop<Option<beui::Vec2>>,
    #[prop(default = false)] dialog: bool,
    on_state: Callback<ChildState>,
    on_bar: Callback<BarAction>,
    on_focus: Callback<()>,
    #[prop(children)] content: Option<Render<ChildHandle>>,
) -> NodeId {
    let layout = use_context::<DialogLayout>();
    let dialog = match dialog {
        true => Prop::dynamic(move || {
            Some(DialogFrame {
                narrow: layout.is_some_and(|layout| layout.narrow.get()),
                bottom_inset: layout.map_or(0.0, |layout| layout.bottom_inset.get()),
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
        intrinsic,
        dialog,
        on_state,
        on_bar,
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
    intrinsic: Prop<Option<beui::Vec2>>,
    dialog: Prop<Option<DialogFrame>>,
    on_state: Callback<ChildState>,
    on_bar: Callback<BarAction>,
    on_focus: Callback<()>,
    content: Option<Render<ChildHandle>>,
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
        intrinsic,
        dialog,
        on_state,
        on_bar,
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
    let docked = beui::dock::unstyled::use_dock_tab().filter(|tab| tab.is_hosted());
    let join = match docked {
        Some(_) => None,
        None => beui::dock::unstyled::use_dock_join(),
    };
    let dock_tab = docked
        .as_ref()
        .map(|docked| docked.tab().value())
        .or_else(|| join.as_ref().map(|join| join.host().value()));
    let (docks, link) = match (claims, &docked, &join) {
        (false, _, _) => (false, None),
        (true, Some(docked), _) => (docked.claim_body(), None),
        (true, None, Some(join)) => match join.claim() {
            Some(link) => (true, Some(link)),
            None => (false, None),
        },
        (true, None, None) => (false, None),
    };
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
        dialog,
        on_state,
        on_bar,
        on_focus,
        dock_tab,
        docks,
    );
    if let Some(link) = link {
        crate::docked::bridge_child_dock(editor, link, state, docked_tab);
    }
    let preview = create_memo(move || state.with(|state| state.preview));
    let pictured = create_memo(move || {
        mode.get() == ChildMode::Preview || preview.get() != ChildPreview::Hosted
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
    let handle = ChildHandle { state, menu };
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
        <Embed slot={slot} punch={punch} rotation={turn} children={Some(inner)} />
    }
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
        ChildPreview::Hosted | ChildPreview::Waiting | ChildPreview::Label => None,
    });
    let labelled = create_memo(move || preview.get() == ChildPreview::Label);
    let target = create_memo(move || match placed.get() {
        Some(SubregionContent::Block(target)) => Some(target),
        _ => None,
    });
    view! {
        <Layers>
            <Drawing draw={Prop::dynamic(move || draw_gpu(drawing.get()))} />
            <Show condition={labelled}>
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
    let has_glyph = create_memo(move || glyph.get().is_some());
    view! {
        <Frame
            fill=ColorRole::SurfaceRaised
            border={Border::new(ColorRole::Border)}
            radius=Radius::Md
            padding=Space::Lg
            align=Align2::CENTER
        >
            <List spacing=Space::Md align=Align::Center>
                <Show condition={has_glyph}>
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
            commands: HashMap::new(),
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
                    if label.get_untracked() != command.label {
                        label.set(command.label.clone());
                    }
                    if enabled.get_untracked() != command.enabled {
                        enabled.set(command.enabled);
                    }
                    if ticked.get_untracked() != checked {
                        ticked.set(checked);
                    }
                    Shape::Command {
                        id: command.id.clone(),
                        label: None,
                        glyph: Glyph::from_codepoint(&command.glyph),
                        check,
                        shortcut: command.shortcut,
                        pin: command.pin,
                    }
                }
                MenuEntry::Submenu(submenu) => Shape::Submenu {
                    id: submenu.id.clone(),
                    label: submenu.label.clone(),
                    glyph: Glyph::from_codepoint(&submenu.glyph),
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
