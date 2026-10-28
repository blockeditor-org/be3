use std::rc::Rc;

use beui_types::node::NodeId;
use beui_types::{Rect, WindowId};

pub use beui_drag::{
    Accepts, DragPoint, DragSource, Dropped, EDGE_BAND, EdgeScroller, Outside, Over, Pending,
    Surface, Target,
};

pub type Board = beui_drag::Board<Measured>;

#[derive(Default)]
pub struct Measured {
    #[cfg(target_arch = "wasm32")]
    following: std::cell::RefCell<Vec<crate::dom::Listener>>,
}

pub struct Smaller(f32);

impl PartialEq for Smaller {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for Smaller {}

impl PartialOrd for Smaller {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Smaller {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.0.total_cmp(&self.0)
    }
}

impl Surface for Measured {
    type Order = Smaller;

    fn placed(&self, node: NodeId) -> Option<(Rect, f32)> {
        Some((crate::measure::rect(node)?, 1.0))
    }

    fn visible(&self, node: NodeId) -> Option<Rect> {
        crate::measure::rect(node)
    }

    fn window(&self, _node: NodeId) -> WindowId {
        WindowId::MAIN
    }

    fn target_order(&self, _node: NodeId) -> Smaller {
        Smaller(0.0)
    }

    fn scroller_order(&self, _node: NodeId, visible: Rect) -> Smaller {
        Smaller(visible.width() * visible.height())
    }

    fn carrying(&self, carrying: bool) {
        #[cfg(target_arch = "wasm32")]
        {
            *self.following.borrow_mut() = match carrying {
                true => web::follow(),
                false => Vec::new(),
            };
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = carrying;
    }
}

pub fn drag_board() -> Rc<Board> {
    crate::document::extension_or_insert_with(|| {
        crate::keys::listen(
            crate::keys::KeyAt::Window,
            crate::keys::KeyPhase::Capture,
            Rc::new(|event: crate::keys::KeyEvent| match event.input {
                crate::keys::KeyInput::Press(press) => {
                    press.pressed && press.key == beui_types::Key::Escape && drag_board().cancel()
                }
                crate::keys::KeyInput::Tap(_) => false,
            }),
        );
        Rc::new(Board::new(Measured::default()))
    })
}

#[cfg(target_arch = "wasm32")]
mod web {
    use beui_types::{DragPoint, Pos2, pos2};
    use wasm_bindgen::JsCast;

    fn point(event: &web_sys::PointerEvent) -> DragPoint {
        let origin = crate::run::root()
            .and_then(crate::dom::dom)
            .and_then(|dom| dom.dyn_into::<web_sys::Element>().ok())
            .map(|root| {
                let rect = root.get_bounding_client_rect();
                pos2(rect.left() as f32, rect.top() as f32)
            })
            .unwrap_or(Pos2::ZERO);
        let modifiers = beui_web_input::keys::modifiers(
            event.alt_key(),
            event.ctrl_key(),
            event.meta_key(),
            event.shift_key(),
        );
        DragPoint::at(
            pos2(
                event.client_x() as f32 - origin.x,
                event.client_y() as f32 - origin.y,
            ),
            modifiers,
        )
    }

    pub(super) fn follow() -> Vec<crate::dom::Listener> {
        let Some(window) = crate::dom::window() else {
            return Vec::new();
        };
        let target: &web_sys::EventTarget = window.as_ref();
        vec![
            crate::dom::listen(
                target,
                "pointermove",
                true,
                |event: web_sys::PointerEvent| {
                    super::drag_board().track(point(&event));
                    reactive::flush();
                },
            ),
            crate::dom::listen(target, "pointerup", true, |event: web_sys::PointerEvent| {
                let board = super::drag_board();
                board.track(point(&event));
                board.finish();
                reactive::flush();
            }),
            crate::dom::listen(target, "pointercancel", true, |_: web_sys::PointerEvent| {
                super::drag_board().end(None);
                reactive::flush();
            }),
        ]
    }
}
