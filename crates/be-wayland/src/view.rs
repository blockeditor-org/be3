use std::rc::Rc;

use beui::reactive::{
    Action, Chord, Drawing, Frame, Interactive, Layers, NodeRef, Prop, clone, component,
    component_rect,
    create_effect, create_memo, draw_gpu, on_cleanup, view,
};
use beui::{Color32, ForwardedInput, Key, NodeId, Pos2, Rect};

use crate::state::WindowId;
use crate::windows::Windows;

const FROZEN_DIM: Color32 = Color32::from_rgba_unmultiplied(0, 0, 0, 140);

#[component]
pub fn WindowView(
    windows: Windows,
    id: WindowId,
    #[prop(default = Vec::new())] occluders: Prop<Vec<Rect>>,
) -> NodeId {
    let Some(signals) = windows.signals(id) else {
        return view! {
            <Frame />
        };
    };
    let rect = component_rect();
    let catcher = NodeRef::new();
    create_effect(clone!(windows rect catcher -> move || {
        windows.placed(id, rect.get());
        if let Some(node) = catcher.try_get() {
            windows.caught_by(id, node);
        }
    }));
    let list = windows.list();
    let inhibits = create_memo(clone!(list -> move || {
        list.with(|list| {
            list.iter()
                .any(|info| info.id == id && info.inhibits_shortcuts)
        })
    }));
    let dim = create_memo(move || {
        let responding = list.with(|list| {
            list.iter()
                .find(|info| info.id == id)
                .is_none_or(|info| info.responding)
        });
        match responding {
            true => Color32::TRANSPARENT,
            false => FROZEN_DIM,
        }
    });
    let cursor = windows.cursor();
    let focus = windows.clone();
    let forward = windows.clone();
    let takes = move |local: Pos2| {
        let position = local + rect.get_untracked().min.to_vec2();
        !occluders
            .peek()
            .iter()
            .any(|occluder| occluder.contains(position))
    };
    on_cleanup(move || windows.unplaced(id));
    view! {
        <Interactive
            @node_ref={&catcher}
            focusable=true
            @test_id={format!("wayland.window.{}", id.0)}
            focused={signals.focused}
            on_focus_change={move |focused: bool| focus.focus(id, focused)}
            cursor
            on_forward={move |input: ForwardedInput| forward.forward(id, input)}
            inhibits_shortcuts={inhibits}
            forward_at={takes}
        >
            <Layers>
                <Drawing draw={Prop::Dynamic(Rc::new(move || draw_gpu(signals.drawing.get())))} />
                <Frame color={dim} />
            </Layers>
        </Interactive>
    }
}

pub const RESTORE_SHORTCUTS: Chord = Chord::logo(Key::Escape);

pub fn register_restore_shortcuts(windows: &Windows) {
    let list = windows.list();
    let focus = windows.clone();
    let inhibiting = create_memo(move || {
        list.with(|list| {
            list.iter().any(|info| {
                info.inhibits_shortcuts
                    && focus
                        .signals(info.id)
                        .is_some_and(|signals| signals.focused.get())
            })
        })
    });
    let restoring = windows.clone();
    Action::new(
        "wayland.restore_shortcuts",
        "Restore keyboard shortcuts",
        move || restoring.restore_shortcuts(),
    )
    .shortcut(RESTORE_SHORTCUTS)
    .escape_hatch()
    .enabled(inhibiting)
    .register();
}
