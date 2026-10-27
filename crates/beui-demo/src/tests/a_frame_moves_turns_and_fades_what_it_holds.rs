use super::*;

#[test]
fn a_frame_moves_turns_and_fades_what_it_holds() {
    let mut test = alone(WIDE, Page::Layering);
    reveal(&mut test, "demo.layering.frame_effects");
    test.snapshot("a_frame_moves_turns_and_fades_what_it_holds");
}
