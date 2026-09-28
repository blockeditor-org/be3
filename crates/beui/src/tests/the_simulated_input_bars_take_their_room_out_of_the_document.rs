use super::*;

#[test]
fn the_simulated_input_bars_take_their_room_out_of_the_document() {
    let mut harness = Harness::sized(hello_column().document, TALL_VIEWPORT);
    harness.frame(Vec::new());
    let whole = harness.document_bottom();
    assert_eq!(whole, TALL_VIEWPORT.y);

    harness.enable_mouse_simulation();
    harness.frame(Vec::new());
    let beside_the_bar = harness.document_bottom();
    assert!(
        beside_the_bar < whole,
        "the mouse bar took no room out of {whole}"
    );

    let keyboard = harness.simulated_button(3);
    harness.finger(1, TouchPhase::Start, keyboard);
    harness.finger(1, TouchPhase::End, keyboard);
    harness.frame(Vec::new());
    assert!(
        harness.document_bottom() < beside_the_bar,
        "the simulated keyboard took no room out of {beside_the_bar}"
    );
}
