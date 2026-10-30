use super::*;
use beui_core::input::{ImeArea, Keyboard};

#[test]
fn a_views_keyboard_reaches_the_host_with_its_asks() {
    let asks = Rc::new(Cell::new(0));
    let asked = Rc::clone(&asks);
    let mut surface = launched(
        &EditorHost::default(),
        Windowed::empty().hooked(move |context| {
            context.set_ime_area(Some(ImeArea {
                rect: Rect::from_min_size(pos2(4.0, 4.0), vec2(20.0, 10.0)),
                cursor: Rect::from_min_size(pos2(6.0, 4.0), vec2(1.0, 10.0)),
                text: None,
                keyboard: Keyboard::Shown { asks: asked.get() },
            }));
        }),
    );
    let shown = whole(40.0, 30.0);
    let keyboard = |frame: Frame| frame.window.ime.map(|ime| ime.keyboard);
    assert_eq!(
        keyboard(one(&mut surface, &shown)),
        Some(block_plugin_api::Keyboard::Shown { asks: 0 })
    );

    asks.set(1);
    assert_eq!(
        keyboard(one(&mut surface, &shown)),
        Some(block_plugin_api::Keyboard::Shown { asks: 1 }),
        "a tap that asked for the keyboard again reaches the host as a new count"
    );
}
