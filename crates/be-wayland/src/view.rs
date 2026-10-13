use beui::KeyAt;
use beui::reactive::LayerPolicy;

use beui::reactive::{
    Action, Chord, Drawing, Frame, Interactive, Layers, NodeRef, Overlay, OverlayAnchor, Placement,
    PlacementBuilder, Prop, ReadSignal, clone, component, create_effect, create_memo, draw_gpu,
    on_cleanup, view,
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
    view! {
        <PlacementBuilder>
            {move |rect: ReadSignal<Rect>| {
                let catcher = NodeRef::new();
                let popup_catcher = NodeRef::new();
                create_effect(clone!(windows -> move || {
                    windows.placed(id, rect.get());
                    if let Some(node) = catcher.try_get() {
                        windows.caught_by(id, node);
                    }
                    if let Some(node) = popup_catcher.try_get() {
                        windows.popups_caught_by(id, node);
                    }
                }));
                let list = windows.list();
                let inhibits = create_memo(move || {
                    list.with(|list| {
                        list.iter()
                            .any(|info| info.id == id && info.inhibits_shortcuts)
                    })
                });
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
                let placed = rect;
                let takes = move |local: Pos2| {
                    let position = local + placed.get_untracked().min.to_vec2();
                    !occluders
                        .peek()
                        .iter()
                        .any(|occluder| occluder.contains(position))
                };
                let popups = signals.popups;
                let shown = create_memo(move || popups.with(|popups| {
                    popups.as_ref().map(|popups| popups.rect)
                }));
                let open = create_memo(move || shown.get().is_some());
                let anchor = create_memo(move || {
                    let at = shown.get().unwrap_or(Rect::ZERO);
                    OverlayAnchor::Rect(at.translate(rect.get().min.to_vec2()))
                });
                let width = create_memo(move || shown.get().map(|at| at.width()));
                let height = create_memo(move || shown.get().map(|at| at.height()));
                let reach = windows.clone();
                let into_window = move |mut input: ForwardedInput| {
                    let at = shown.get_untracked().unwrap_or(Rect::ZERO);
                    input.rect = input.rect.translate(-at.min.to_vec2());
                    reach.forward(id, input);
                };
                let popup_cursor = windows.cursor();
                let areas = signals.popups;
                let on_popup = move |local: Pos2| {
                    areas.with_untracked(|popups| {
                        popups
                            .as_ref()
                            .is_some_and(|popups| popups.areas.iter().any(|area| area.contains(local)))
                    })
                };
                on_cleanup(move || windows.unplaced(id));
                view! {
                    <Layers>
                        <Interactive
                            @node_ref={&catcher}
                            focusable=true
                            @test_id={format!("wayland.window.{}", id.0)}
                            focused={signals.focused}
                            on_focus_change={move |focused: bool| focus.focus(id, focused)}
                            cursor
                            on_forward={move |input: ForwardedInput| forward.forward(id, input)}
                            inhibits_shortcuts={inhibits}
                            hit_at={takes}
                        >
                            <Layers>
                                <Drawing
                                    draw={Prop::dynamic(move || draw_gpu(signals.drawing.get()))}
                                />
                                <Frame color={dim} />
                            </Layers>
                        </Interactive>
                        <Overlay
                            anchor={anchor}
                            placement=Placement::At
                            policy=LayerPolicy::Pass
                            traps_focus=false
                            open={open}
                        >
                            <Frame width height>
                                <Interactive
                                    @node_ref={&popup_catcher}
                                    focusable=true
                                    press_focus=false
                                    tab_stop=false
                                    @test_id={format!("wayland.popups.{}", id.0)}
                                    cursor={popup_cursor}
                                    on_forward={into_window}
                                    hit_at={on_popup}
                                >
                                    <Drawing
                                        draw={Prop::dynamic(move || {
                                            draw_gpu(popups.with(|popups| {
                                                popups.as_ref().map(|popups| popups.drawing.clone())
                                            }))
                                        })}
                                    />
                                </Interactive>
                            </Frame>
                        </Overlay>
                    </Layers>
                }
            }}
        </PlacementBuilder>
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
    .at(KeyAt::Host)
    .capture()
    .enabled(inhibiting)
    .register();
}
