use std::cell::Cell;
use std::rc::Rc;

use beui_macros::{component, view};

use crate::base::overlay::{Overlay, OverlayAnchor, OverlayMode, Placement};
use crate::geometry::{Pos2, Vec2, pos2, vec2};
use crate::input::{CursorIcon, PointerPress};
use crate::node::NodeId;
use crate::reactive::{
    Callback, Canvas, CanvasItem, ClickCatcher, Frame, IntoProp, NodeRef, Prop, Render, clone,
    create_memo, create_signal, with_document,
};

const GRIP: f32 = 16.0;
const MIN_SIZE: Vec2 = vec2(200.0, 120.0);

pub struct WindowHandle {
    pub grab: Callback<PointerPress>,
    pub drag: Callback<PointerPress>,
}

#[component]
pub fn Window(
    open: Prop<bool>,
    #[prop(default = pos2(96.0, 96.0))] position: Pos2,
    #[prop(default = vec2(480.0, 360.0))] size: Vec2,
    content: Render<WindowHandle>,
) -> NodeId {
    let (origin, set_origin) = create_signal(position);
    let (extent, set_extent) = create_signal(size);
    let overlay = NodeRef::new();
    let grabbed: Rc<Cell<(Pos2, Pos2)>> = Rc::new(Cell::new((position, position)));
    let held = grabbed.clone();
    let start = origin.clone();
    let moved = set_origin.clone();
    let face = content.call(WindowHandle {
        grab: Callback::new(move |press: PointerPress| {
            held.set((start.get_untracked(), press.pos));
        }),
        drag: Callback::new(move |press: PointerPress| {
            let (start, from) = grabbed.get();
            moved.set(start + (press.pos - from));
        }),
    });
    let anchor = origin.clone().into_prop().map(OverlayAnchor::Point);
    let width = create_memo(clone!(extent -> move || extent.get().x));
    let height = create_memo(clone!(extent -> move || extent.get().y));
    let frame_width = create_memo(clone!(width -> move || Some(width.get())));
    let frame_height = create_memo(clone!(height -> move || Some(height.get())));
    let grip_x = create_memo(clone!(width -> move || width.get() - GRIP));
    let grip_y = create_memo(clone!(height -> move || height.get() - GRIP));
    let resize_from: Rc<Cell<(Vec2, Pos2)>> = Rc::new(Cell::new((size, Pos2::ZERO)));
    let resize_start = resize_from.clone();
    let raised = overlay.clone();
    view! {
        <Overlay
            @node_ref=&overlay
            anchor={anchor}
            placement=Placement::BelowStart
            mode=OverlayMode::Floating
            traps_focus=false
            open={open}
        >
            <Frame width={frame_width} height={frame_height}>
                <Canvas>
                    <CanvasItem x=0.0 y=0.0 width={width} height={height}>
                        <ClickCatcher
                            on_press={move |_press: PointerPress| {
                                if let Some(overlay) = raised.try_get() {
                                    with_document(|document| document.raise_overlay(overlay));
                                }
                            }}
                            children={face}
                        />
                    </CanvasItem>
                    <CanvasItem x={grip_x} y={grip_y} width=GRIP height=GRIP>
                        <ClickCatcher
                            cursor=CursorIcon::ResizeNwSe
                            on_press={move |press: PointerPress| {
                                resize_start.set((extent.get_untracked(), press.pos));
                            }}
                            on_drag={move |press: PointerPress| {
                                let (start, from) = resize_from.get();
                                set_extent.set((start + (press.pos - from)).max(MIN_SIZE));
                            }}
                        />
                    </CanvasItem>
                </Canvas>
            </Frame>
        </Overlay>
    }
}
