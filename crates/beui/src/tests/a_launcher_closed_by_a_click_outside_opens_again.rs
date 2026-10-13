use super::*;

#[test]
fn a_launcher_closed_by_a_click_outside_opens_again() {
    reopens_after(|harness| harness.click(Pos2::new(2.0, 2.0)));
}
