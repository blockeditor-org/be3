use beui::NodeId;
use beui::icons::Glyph;
use beui::reactive::*;
use beui::styled::*;
use std::cell::RefCell;
use std::collections::HashMap;

use block_plugin_api::{
    BarAction, ChildId, ChildLayer, ChildMode, KeyChord, MenuCheck, MenuEntry, MenuPin, TopBar,
};

use crate::{ChildState, ChildTarget, Editor, SubregionContent};

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
    #[prop(children)] content: Option<Render<ChildHandle>>,
) -> NodeId {
    let placed = create_memo(move || block.get().map(SubregionContent::Block));
    let placing = Placing {
        mode,
        layer,
        own_frame,
        top_bar,
        punch,
        rotation,
        opacity,
        intrinsic,
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
    on_state: Callback<ChildState>,
    on_bar: Callback<BarAction>,
    on_focus: Callback<()>,
    #[prop(children)] content: Option<Render<ChildHandle>>,
) -> NodeId {
    let placing = Placing {
        mode,
        layer,
        own_frame,
        top_bar,
        punch,
        rotation,
        opacity,
        intrinsic,
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
    mode: Prop<ChildMode>,
    layer: Prop<ChildLayer>,
    own_frame: Prop<bool>,
    top_bar: Prop<TopBar>,
    punch: Prop<bool>,
    rotation: Prop<f32>,
    opacity: Prop<f32>,
    intrinsic: Prop<Option<beui::Vec2>>,
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
        mode,
        layer,
        own_frame,
        top_bar,
        punch,
        rotation,
        opacity,
        intrinsic,
        on_state,
        on_bar,
        on_focus,
        content,
    } = placing;
    let slot = EmbedSlot::new();
    let layer = create_memo(move || layer.get());
    let below = create_memo(move || matches!(layer.get(), ChildLayer::Below));
    let punch = create_memo(move || below.get() && punch.get());
    let turn = create_memo(move || rotation.get());
    let state = editor.register_child(
        slot,
        placed,
        mode,
        layer.into_prop(),
        own_frame,
        top_bar,
        turn.into_prop(),
        opacity,
        intrinsic,
        on_state,
        on_bar,
        on_focus,
    );
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
    let children = content.map(|content| content.call(handle));
    view! {
        <Embed slot={slot} punch={punch} rotation={turn} children={children} />
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

struct MenuMirror {
    editor: Editor,
    commands: HashMap<String, Mirrored>,
    shape: Option<(Option<ChildId>, Vec<Shape>)>,
}

impl MenuMirror {
    fn new(editor: Editor) -> Self {
        Self {
            editor,
            commands: HashMap::new(),
            shape: None,
        }
    }

    fn update(&mut self, child: Option<ChildId>, entries: &[MenuEntry]) -> Option<Vec<MenuNode>> {
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
                    let mut action = Action::new(id.clone(), label, move || {
                        host.pick_child_menu(child, picked.clone())
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
