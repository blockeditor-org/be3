use std::rc::Rc;

use block_editor_plugin::beui::reactive::{
    Canvas, CanvasItem, ClickCatcher, Focusable, Frame, ItemSize, List, Picture, clone, component,
    component_rect, create_memo, view,
};
use block_editor_plugin::beui::styled::use_theme;
use block_editor_plugin::beui::{
    CursorIcon, ImageFit, Key, KeyPress, NodeId, PointerPress, Pos2, Rect, Vec2,
};

use crate::geometry::{artwork_rect, screen_to_world};

use super::state::{RayState, Tool};

#[component]
pub(crate) fn Artwork(state: Rc<RayState>) -> NodeId {
    let placed = component_rect();
    let world = state.editor().world();
    let artwork = create_memo(clone!(world placed -> move || {
        let available = world
            .get()
            .unwrap_or_else(|| placed.get().size())
            .max(Vec2::new(1.0, 1.0));
        artwork_rect(Rect::from_min_size(Pos2::ZERO, available))
    }));
    let left = create_memo(clone!(artwork -> move || artwork.get().left()));
    let top = create_memo(clone!(artwork -> move || artwork.get().top()));
    let side = create_memo(clone!(artwork -> move || artwork.get().width()));

    let pressing = Rc::clone(&state);
    let press_artwork = artwork.clone();
    let press_view = state.editor().canvas();
    let press_editor = state.editor().clone();
    let on_press = move |press: PointerPress| {
        let Some(at) = world_point(
            press.pos,
            &press_view,
            &press_editor,
            press_artwork.get_untracked(),
        ) else {
            return;
        };
        pressing.press(at);
    };

    let dragging = Rc::clone(&state);
    let drag_artwork = artwork.clone();
    let drag_view = state.editor().canvas();
    let drag_editor = state.editor().clone();
    let on_drag = move |press: PointerPress| {
        let Some(at) = world_point(
            press.pos,
            &drag_view,
            &drag_editor,
            drag_artwork.get_untracked(),
        ) else {
            return;
        };
        dragging.drag(at);
    };

    let hovering = Rc::clone(&state);
    let hover_artwork = artwork.clone();
    let hover_view = state.editor().canvas();
    let hover_editor = state.editor().clone();
    let on_hover_move = move |press: PointerPress| {
        hovering.hover(world_point(
            press.pos,
            &hover_view,
            &hover_editor,
            hover_artwork.get_untracked(),
        ));
    };

    let releasing = Rc::clone(&state);
    let removing = Rc::clone(&state);
    let lighting = state.lighting.clone();
    let rays = state.rays.clone();
    let overlay = state.overlay.clone();
    let tool = state.tool.clone();
    let tracing = create_memo(clone!(tool rays -> move || {
        tool.get() == Tool::RayTrace && rays.get().is_some()
    }));
    let camera = state.editor().canvas();
    let theme = use_theme();

    view! {
        <Focusable
            on_key={move |press: KeyPress| {
                if press.key == Key::Delete || press.key == Key::Backspace {
                    removing.delete_selected();
                    return true;
                }
                false
            }}
        >
            <ClickCatcher
                cursor=CursorIcon::Crosshair
                on_press={on_press}
                on_drag={on_drag}
                on_hover_move={on_hover_move}
                on_active_change={move |active: bool| {
                    if !active {
                        releasing.release();
                    }
                }}
            >
                <Frame color={theme.background.clone()}>
                    <List spacing=0.0>
                        <Canvas view={camera} @sizing=ItemSize::Percent(100.0)>
                            <CanvasItem
                                x={left.clone()}
                                y={top.clone()}
                                width={side.clone()}
                                height={side.clone()}
                                @test_id={"pixel_ray_tracer.artwork"}
                            >
                                <Picture image={lighting} fit=ImageFit::Fill smooth=false />
                            </CanvasItem>
                            <CanvasItem
                                x={left.clone()}
                                y={top.clone()}
                                width={side.clone()}
                                height={side.clone()}
                            >
                                <Frame visible={tracing}>
                                    <Picture image={rays} fit=ImageFit::Fill smooth=false />
                                </Frame>
                            </CanvasItem>
                            <CanvasItem x={left} y={top} width={side.clone()} height={side}>
                                <Picture image={overlay} fit=ImageFit::Fill smooth=false />
                            </CanvasItem>
                        </Canvas>
                    </List>
                </Frame>
            </ClickCatcher>
        </Focusable>
    }
}

fn world_point(
    at: Pos2,
    view: &block_editor_plugin::beui::reactive::ReadSignal<
        Option<block_editor_plugin::beui::reactive::CanvasView>,
    >,
    editor: &block_editor_plugin::Editor,
    artwork: Rect,
) -> Option<block_client::blocks::pixel_ray_tracer::Point> {
    let camera = view.get_untracked()?;
    let _ = editor;
    Some(screen_to_world(camera.to_canvas(at), artwork))
}
