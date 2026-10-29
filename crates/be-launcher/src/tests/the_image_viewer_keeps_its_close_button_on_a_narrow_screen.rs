use beui::native::WindowSize;
use beui::reactive::*;
use beui::{Vec2, native::Context};
use beui_host::native::FreetypeFonts;
use beui_host::native::testing::{Showing, Stepped};

use crate::viewer::ViewerBar;
use beui_host::native::build;

#[test]
fn the_image_viewer_keeps_its_close_button_on_a_narrow_screen() {
    let viewport = Vec2::new(320.0, 200.0);
    let document = build(|| {
        let position =
            create_memo(|| "Image 3 of 12 · the arrow keys move between them".to_owned());
        let first = create_memo(|| false);
        let last = create_memo(|| false);
        view! {
            <beui::styled::Root>
                <ViewerBar
                    position
                    first
                    last
                    on_previous={|| {}}
                    on_next={|| {}}
                    on_browse={|| {}}
                    on_close={|| {}}
                />
            </beui::styled::Root>
        }
    });
    let context = Context::new(FreetypeFonts::default());
    let window = WindowSize {
        size: viewport,
        scale: 1.0,
    };
    let mut stepped = Stepped::new(context, Showing(document), window, None);
    stepped.frame(Vec::new());

    let document = stepped.document();
    let close = document
        .find_test_id("viewer.close")
        .expect("a close button");
    let rect = document
        .node_rect(close)
        .expect("the close button is laid out");
    assert!(
        rect.right() <= viewport.x,
        "the close button ends at {} in a screen {} wide",
        rect.right(),
        viewport.x
    );
}
