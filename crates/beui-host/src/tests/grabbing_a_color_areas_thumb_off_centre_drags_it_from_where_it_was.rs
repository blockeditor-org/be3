use super::*;
use crate::color::Hsva;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn grabbing_a_color_areas_thumb_off_centre_drags_it_from_where_it_was() {
    let _root = crate::reactive::RootScope::new();
    let changes = create_signal(Vec::new());
    let picker = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ColorPicker
                    @node_ref=&picker
                    value={Binding::new(Hsva::new(0.0, 0.5, 0.5, 1.0).to_color(), move |next| changes.update(|changes| changes.push(next)))}
                />
            </List>
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());
    let area = harness.rect(picker.get());
    let grabbed = pos2(
        area.left() + PICKER_WIDTH / 2.0 + 4.0,
        area.top() + 78.0 + 4.0,
    );

    harness.drag(grabbed, pos2(grabbed.x + 61.0, grabbed.y));
    harness.frame(Vec::new());

    let changes = changes.get_untracked();
    assert_eq!(changes.len(), 1);
    let picked = Hsva::from_color(changes[0]);
    assert!((picked.saturation - 0.75).abs() < 0.01, "{picked:?}");
    assert!((picked.value - 0.5).abs() < 0.01, "{picked:?}");
}
