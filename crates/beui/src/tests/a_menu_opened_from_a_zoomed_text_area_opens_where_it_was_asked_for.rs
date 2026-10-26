use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::TextAreaState;
use std::sync::Arc;
use text_editor_core::TextBuffer;

const SCALE: f32 = 2.0;

#[test]
fn a_menu_opened_from_a_zoomed_text_area_opens_where_it_was_asked_for() {
    let _root = crate::reactive::RootScope::new();
    let area = NodeRef::new();
    let document = build_node(move || {
        let document = Arc::new(TextBuffer::new(b"first line\nsecond line\n"))
            as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        view! {
            <Canvas view={Some(CanvasView::new(pos2(40.0, 30.0), SCALE))}>
                <CanvasItem x=10.0 y=10.0 width=200.0 height=120.0 scales=true>
                    <TextArea @node_ref=&area state={state} />
                </CanvasItem>
            </Canvas>
        }
    });
    let mut harness = Harness::sized(document, vec2(900.0, 700.0));
    harness.frame(Vec::new());
    let root = area.get();
    let rect = harness.rect(root);

    let pressed = rect.min + vec2(60.0, 20.0) * SCALE;
    harness.frame(vec![Input::PointerMoved(pressed)]);
    harness.frame(vec![Input::PointerButton {
        pos: pressed,
        button: PointerButton::Secondary,
        pressed: true,
    }]);
    harness.frame(vec![Input::PointerButton {
        pos: pressed,
        button: PointerButton::Secondary,
        pressed: false,
    }]);
    harness.frame(Vec::new());
    let paste = text_within(harness.document(), root, "Paste")
        .map(|paste| harness.rect(paste))
        .expect("a right click opens the text menu");
    assert!(
        paste.left() >= pressed.x
            && paste.left() < pressed.x + 40.0
            && paste.top() >= pressed.y
            && paste.top() < pressed.y + 40.0,
        "the menu {paste:?} opens at the pressed point {pressed:?}, in the window's coordinates"
    );
    harness.key(Key::Escape, Modifiers::NONE);

    harness.click(rect.min + vec2(150.0, 16.0) * SCALE);
    harness.type_text(" :");
    harness.frame(Vec::new());
    let caret = harness
        .document()
        .focused_ime_area()
        .expect("the focused text area reports its caret")
        .cursor;
    let row = harness
        .document()
        .find_test_id("text.completion.0")
        .expect("a colon opens the emoji menu");
    let menu = harness.rect(row);
    assert!(
        menu.top() >= caret.bottom() - 1.0
            && menu.top() < caret.bottom() + 40.0
            && (menu.left() - caret.left()).abs() < 60.0,
        "the emoji menu {menu:?} opens under the zoomed caret {caret:?}"
    );
}
