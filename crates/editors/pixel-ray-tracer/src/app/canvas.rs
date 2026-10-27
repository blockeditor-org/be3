use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{
    CursorIcon, ImageFit, Key, KeyPress, NodeId, PointerPress, Pos2, Rect, Vec2,
};

use crate::geometry::{artwork_rect, screen_to_world};

use super::state::{RayState, Tool};
use beui::ItemSize;

#[component]
pub(crate) fn Artwork(state: RayState) -> NodeId {
    let placed_held = create_signal(Rect::ZERO);
    let placed = placed_held.read_only();
    let editor = state.editor();
    let world = editor.world();
    let artwork = create_memo(move || {
        let available = world
            .get()
            .unwrap_or_else(|| placed.get().size())
            .max(Vec2::new(1.0, 1.0));
        artwork_rect(Rect::from_min_size(Pos2::ZERO, available))
    });
    let left = create_memo(move || artwork.get().left());
    let top = create_memo(move || artwork.get().top());
    let side = create_memo(move || artwork.get().width());

    let press_view = state.editor().canvas();
    let on_press = move |press: PointerPress| {
        let Some(at) = world_point(press.pos, &press_view, editor, artwork.get_untracked()) else {
            return;
        };
        state.press(at);
    };

    let drag_view = state.editor().canvas();
    let on_drag = move |press: PointerPress| {
        let Some(at) = world_point(press.pos, &drag_view, editor, artwork.get_untracked()) else {
            return;
        };
        state.drag(at);
    };

    let hover_view = state.editor().canvas();
    let on_hover_move = move |press: PointerPress| {
        state.hover(world_point(
            press.pos,
            &hover_view,
            editor,
            artwork.get_untracked(),
        ));
    };

    let lighting = state.lighting;
    let rays = state.rays;
    let overlay = state.overlay;
    let tool = state.tool;
    let tracing = create_memo(move || tool.get() == Tool::RayTrace && rays.get().is_some());
    let camera = state.editor().canvas();

    let built = view! {
        <Interactive
            focusable=true
            on_key={move |press: KeyPress| {
                if press.key == Key::Delete || press.key == Key::Backspace {
                    state.delete_selected();
                    return true;
                }
                false
            }}
            cursor=CursorIcon::Crosshair
            on_press={on_press}
            on_drag={on_drag}
            on_hover_move={on_hover_move}
            on_active_change={move |active: bool| {
                if !active {
                    state.release();
                }
            }}
        >
            <Frame fill=ColorRole::Background>
                <List spacing=Space::None>
                    <Canvas view={camera} @sizing=ItemSize::Percent(100.0)>
                        <CanvasItem
                            x={left}
                            y={top}
                            width={side}
                            height={side}
                            @test_id={"pixel_ray_tracer.artwork"}
                        >
                            <Picture image={lighting} fit=ImageFit::Fill smooth=false />
                        </CanvasItem>
                        <Show condition={tracing}>
                            <CanvasItem x={left} y={top} width={side} height={side}>
                                <Picture image={rays} fit=ImageFit::Fill smooth=false />
                            </CanvasItem>
                        </Show>
                        <CanvasItem x={left} y={top} width={side} height={side}>
                            <Drawing draw={overlay} />
                        </CanvasItem>
                    </Canvas>
                </List>
            </Frame>
        </Interactive>
    };
    let placed_measured = measure(built);
    create_effect(move || {
        if let Some(measured) = placed_measured.get() {
            placed_held.set(measured.rect);
        }
    });
    built
}

fn world_point(
    at: Pos2,
    view: &block_editor_beui::beui::reactive::ReadSignal<
        Option<block_editor_beui::beui::CanvasView>,
    >,
    editor: block_editor_beui::Editor,
    artwork: Rect,
) -> Option<pixel_ray_tracer_types::Point> {
    let camera = view.get_untracked()?;
    let _ = editor;
    Some(screen_to_world(camera.to_canvas(at), artwork))
}
