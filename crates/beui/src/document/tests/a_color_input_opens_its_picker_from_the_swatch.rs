use accesskit::Role;

use super::*;
use crate::reactive::view;
use crate::styled::ColorInput;

#[test]
fn a_color_input_opens_its_picker_from_the_swatch() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let sink = changes.clone();
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <ColorInput
                value={Color32::from_rgb(0x11, 0x22, 0x33)}
                label="Fill"
                on_change={move |color| sink.borrow_mut().push(color)}
            />
        }]
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());
    let open = |harness: &Harness| {
        harness
            .accessible()
            .iter()
            .any(|node| node.role() == Role::Dialog && node.label() == Some("Choose Fill"))
    };
    assert!(!open(&harness));

    let swatch = harness.rect(input);
    harness.click(pos2(swatch.left() + 8.0, swatch.center().y));
    harness.frame(Vec::new());

    assert!(open(&harness), "clicking the swatch opens the picker");
    let focused = harness
        .document()
        .focused_node()
        .expect("the picker took the focus");
    let area = harness.rect(focused);
    assert!(
        area.width() > 100.0,
        "the saturation and brightness area has the focus"
    );

    harness.key(Key::ArrowUp, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(changes.borrow().len(), 1, "{:?}", changes.borrow());

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!(!open(&harness), "escape closes the picker");
    let trigger = harness
        .document()
        .focused_node()
        .map(|focused| harness.rect(focused))
        .expect("the focus went back to the swatch");
    assert!(trigger.contains(pos2(swatch.left() + 8.0, swatch.center().y)));
}
