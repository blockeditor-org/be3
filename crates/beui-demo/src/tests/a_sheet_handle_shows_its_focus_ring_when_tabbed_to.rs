use super::*;
use beui::vec2;
use beui_macros::view;

#[test]
fn a_sheet_handle_shows_its_focus_ring_when_tabbed_to() {
    let document = build(move || {
        view! {
            <Root>
                <ModalSheet open=true rest=0.5 on_close={|| ()}>
                    <Text>"Sheet content"</Text>
                </ModalSheet>
            </Root>
        }
    });
    let mut test = DocumentTest::new(document, vec2(400.0, 600.0));
    test.settle();
    test.key(beui::Key::Tab, beui::Modifiers::NONE);
    test.settle();
    test.snapshot("a_sheet_handle_focused_from_the_keyboard");
}
