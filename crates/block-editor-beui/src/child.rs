use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;
use block_plugin_api::{BarAction, ChildLayer, ChildMode, TopBar, ViewChange};

use crate::{ChildState, ChildTarget, Editor, SubregionContent};

#[derive(Clone)]
pub struct ChildHandle {
    pub state: ReadSignal<ChildState>,
    pub menu: Memo<Vec<Action>>,
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
    on_view_change: Callback<ViewChange>,
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
        on_view_change,
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
    on_view_change: Callback<ViewChange>,
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
        on_view_change,
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
    on_view_change: Callback<ViewChange>,
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
        on_view_change,
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
        on_view_change,
        on_bar,
        on_focus,
    );
    let entries = create_memo(move || state.with(|state| (state.child, state.menu.clone())));
    let host = editor.host();
    let menu = create_memo(move || {
        let (child, entries) = entries.get();
        let Some(child) = child else {
            return Vec::new();
        };
        entries
            .into_iter()
            .map(|entry| {
                let host = host.clone();
                let id = entry.id.clone();
                Action::new(entry.id, entry.label, move || {
                    host.pick_child_menu(child, id.clone());
                })
                .glyph(&entry.glyph)
                .enabled(entry.enabled)
                .detached()
            })
            .collect()
    });
    let handle = ChildHandle { state, menu };
    let children = content.map(|content| content.call(handle));
    view! {
        <Embed slot={slot} punch={punch} rotation={turn} children={children} />
    }
}
