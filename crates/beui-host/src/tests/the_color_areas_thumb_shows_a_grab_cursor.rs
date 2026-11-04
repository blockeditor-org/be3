use super::*;
use crate::color::Hsva;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;
use beui::CursorIcon;

#[test]
fn the_color_areas_thumb_shows_a_grab_cursor() {
    let _root = crate::reactive::RootScope::new();
    let picker = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ColorPicker @node_ref=&picker value={Hsva::new(0.0, 0.5, 0.5, 1.0).to_color()} />
            </List>
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());
    let area = harness.rect(picker.get());
    let thumb = pos2(area.left() + PICKER_WIDTH / 2.0, area.top() + 78.0);

    harness.frame(vec![Input::PointerMoved(thumb)]);
    let over = harness.frame(Vec::new());
    assert_eq!(over.main().window.requests.cursor, CursorIcon::Grab);

    harness.frame(vec![Input::PointerMoved(pos2(
        area.left() + 10.0,
        area.top() + 10.0,
    ))]);
    let beside = harness.frame(Vec::new());
    assert_eq!(beside.main().window.requests.cursor, CursorIcon::Crosshair);
}
