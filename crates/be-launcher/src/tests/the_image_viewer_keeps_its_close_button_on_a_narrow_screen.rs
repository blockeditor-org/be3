use beui::reactive::{build, create_memo, view};
use beui::{Context, FreetypeFonts, Pos2, RawInput, Rect, Vec2};

use crate::viewer::ViewerBar;

#[test]
fn the_image_viewer_keeps_its_close_button_on_a_narrow_screen() {
    let viewport = Vec2::new(320.0, 200.0);
    let mut document = build(|| {
        let position = create_memo(|| {
            "Image 3 of 12 · the arrow keys move between them".to_owned()
        });
        let first = create_memo(|| false);
        let last = create_memo(|| false);
        view! {
            <ViewerBar
                position
                first
                last
                on_previous={|| {}}
                on_next={|| {}}
                on_browse={|| {}}
                on_close={|| {}}
            />
        }
    });
    let context = Context::new(FreetypeFonts::default());
    context.run(RawInput { events: Vec::new() }, |context| {
        document.show(context, Rect::from_min_size(Pos2::ZERO, viewport));
    });

    let close = document.find_test_id("viewer.close").expect("a close button");
    let rect = document.node_rect(close).expect("the close button is laid out");
    assert!(
        rect.right() <= viewport.x,
        "the close button ends at {} in a screen {} wide",
        rect.right(),
        viewport.x
    );
}
