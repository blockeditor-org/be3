use beui::KeyAt;
use beui::reactive::*;

use beui::styled::*;
use beui::{
    Color32, ForwardedInput, Key, NodeId, Pos2, Rect, Vec2, WindowPlacement, WindowReport,
    WindowRole,
};

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
        <PlacementBuilder>|rect: ReadSignal<Rect>| {
            let catcher = NodeRef::new();
            let popup_catcher = NodeRef::new();
            create_effect(move || {
                windows.placed(id, rect.get());
                if let Some(node) = catcher.try_get() {
                    windows.caught_by(id, node);
                }
            });
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
            let asks = signals.typing;
            let typing = create_memo(move || asks.get().is_some());
            let takes = move |local: Pos2| {
                let position = local + rect.get_untracked().min.to_vec2();
                !occluders
                    .get_untracked()
                    .iter()
                    .any(|occluder| occluder.contains(position))
            };
            let popups = signals.popups;
            let shown = create_memo(move || popups.with(|popups| {
                popups.as_ref().map(|popups| popups.rect)
            }));
            let open = create_memo(move || shown.get().is_some());
            let at = create_memo(move || {
                rect.get().min + shown.get().unwrap_or(Rect::ZERO).min.to_vec2()
            });
            let size = create_memo(move || shown.get().map_or(Vec2::ZERO, |at| at.size()));
            let parent = use_window().map_or(beui::WindowId::MAIN, |window| window.id);
            let hosted = opens_windows();
            let openings = create_memo(move || match (hosted.get(), open.get()) {
                (true, true) => {
                    let at = at.get();
                    vec![Some((at.x.to_bits(), at.y.to_bits()))]
                }
                (true, false) => Vec::new(),
                (false, _) => vec![None],
            });
            let into_window = move |mut input: ForwardedInput| {
                let at = shown.get_untracked().unwrap_or(Rect::ZERO);
                input.rect = input.rect.translate(-at.min.to_vec2());
                windows.forward(id, input);
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
            let built = view! {
                <Layers>
                    <Interactive
                        @node_ref={&catcher}
                        focusable=true
                        @test_id={format!("wayland.window.{}", id.0)}
                        on_focus_change={move |focused: bool| windows.focus(id, focused)}
                        cursor
                        on_forward={move |input: ForwardedInput| windows.forward(id, input)}
                        inhibits_shortcuts={inhibits}
                        ime={typing}
                        keyboard_asks={asks}
                        hit_at={takes}
                    >
                        <Layers>
                            <Drawing
                                draw={Prop::dynamic(move || draw_gpu(signals.drawing.get()))}
                            />
                            <Frame fill={ColorRole::custom(dim)} />
                        </Layers>
                    </Interactive>
                    <ForEach keys={openings}>|_: Option<(u32, u32)>| {
                        let placement = WindowPlacement::Over {
                            window: parent,
                            at: at.get_untracked(),
                        };
                        view! {
                            <Window
                                id={None}
                                size
                                placement
                                at
                                open
                                role={WindowRole::Popup}
                                on_report={|_: WindowReport| {}}
                            >|_: WindowHandle| {
                                create_effect(move || {
                                    rect.get();
                                    if let Some(node) = popup_catcher.try_get() {
                                        windows.popups_caught_by(id, node);
                                    }
                                });
                                view! {
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
                                }
                            }</Window>
                        }
                    }</ForEach>
                </Layers>
            };
            let asks = signals.asks;
            let answered = signals.answered.clone();
            create_effect(move || {
                let asked = asks.get();
                if answered.get() >= asked {
                    return;
                }
                let Some(node) = catcher.try_get() else {
                    return;
                };
                answered.set(asked);
                untrack(|| with_document(|document| document.focus_focusable(node)));
            });
            built
        }</PlacementBuilder>
    }
}

pub const RESTORE_SHORTCUTS: Chord = Chord::logo(Key::Escape);

pub fn register_restore_shortcuts(windows: Windows) {
    let list = windows.list();
    let inhibiting = create_memo(move || {
        list.with(|list| {
            list.iter().any(|info| {
                info.inhibits_shortcuts
                    && windows
                        .signals(info.id)
                        .is_some_and(|signals| signals.focused.get())
            })
        })
    });
    Action::new(
        "wayland.restore_shortcuts",
        "Restore keyboard shortcuts",
        move || windows.restore_shortcuts(),
    )
    .shortcut(RESTORE_SHORTCUTS)
    .at(KeyAt::Host)
    .capture()
    .enabled(inhibiting)
    .register();
}
