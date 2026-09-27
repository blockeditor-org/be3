use super::*;
use crate::color::Hsva;
use crate::input::CursorIcon;
use crate::reactive::view;
use crate::styled::{ColorPicker, PICKER_WIDTH};

#[test]
fn the_color_areas_thumb_shows_a_grab_cursor() {
    let picker = NodeRef::new();
    let named = picker.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <ColorPicker @node_ref=&named value={Hsva::new(0.0, 0.5, 0.5, 1.0).to_color()} />
            </List>
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());
    let area = harness.rect(picker.get());
    let thumb = pos2(area.left() + PICKER_WIDTH / 2.0, area.top() + 78.0);

    harness.frame(vec![Event::PointerMoved(thumb)]);
    let over = harness.frame(Vec::new());
    assert_eq!(over.cursor_icon, CursorIcon::Grab);

    harness.frame(vec![Event::PointerMoved(pos2(
        area.left() + 10.0,
        area.top() + 10.0,
    ))]);
    let beside = harness.frame(Vec::new());
    assert_eq!(beside.cursor_icon, CursorIcon::Crosshair);
}
