use std::rc::Rc;

use beui_types::drag::Driver;

/// Drives the drag board from the browser. While a drag is carried, pointer moves,
/// releases and cancels are caught on the window, so the drag goes on after the element
/// that began it is gone or the pointer leaves it; Escape cancels the drag.
pub fn drive(driver: Rc<dyn Driver>) {
    let escape = Rc::clone(&driver);
    crate::keys::listen(
        crate::keys::KeyAt::Window,
        crate::keys::KeyPhase::Capture,
        Rc::new(move |event: crate::keys::KeyEvent| match event.input {
            crate::keys::KeyInput::Press(press) => {
                press.pressed && press.key == beui_types::Key::Escape && escape.cancel()
            }
            crate::keys::KeyInput::Tap(_) => false,
        }),
    );
    #[cfg(target_arch = "wasm32")]
    web::follow(driver);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = driver;
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::rc::Rc;

    use beui_types::drag::{Driver, Order};
    use beui_types::{DragPoint, Pos2, WindowId, pos2};
    use wasm_bindgen::JsCast;

    #[derive(Clone)]
    struct Following(#[allow(dead_code)] Rc<Vec<crate::dom::Listener>>);

    fn flat(_: beui_types::NodeId) -> (Vec<usize>, WindowId) {
        (Vec::new(), WindowId::MAIN)
    }

    const FLAT: Order<'static> = &flat;

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

    pub(super) fn follow(driver: Rc<dyn Driver>) {
        let Some(window) = crate::dom::window() else {
            return;
        };
        let target: &web_sys::EventTarget = window.as_ref();
        let (moved, released, cancelled) = (Rc::clone(&driver), Rc::clone(&driver), driver);
        let listeners = vec![
            crate::dom::listen(
                target,
                "pointermove",
                true,
                move |event: web_sys::PointerEvent| {
                    if moved.carrying() {
                        moved.track(point(&event), FLAT);
                        beui_reactive::flush();
                    }
                },
            ),
            crate::dom::listen(
                target,
                "pointerup",
                true,
                move |event: web_sys::PointerEvent| {
                    if released.carrying() {
                        released.track(point(&event), FLAT);
                        released.release(false, FLAT);
                        beui_reactive::flush();
                    }
                },
            ),
            crate::dom::listen(
                target,
                "pointercancel",
                true,
                move |_: web_sys::PointerEvent| {
                    if cancelled.carrying() {
                        cancelled.release(true, FLAT);
                        beui_reactive::flush();
                    }
                },
            ),
        ];
        crate::document::extension_or_insert_with(|| Following(Rc::new(listeners)));
    }
}
