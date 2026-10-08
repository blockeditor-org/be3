use super::*;

#[test]
fn checking_a_box_on_both_sides_checks_it_once() {
    let (simulation, left, right) = simulated("- [ ] task\n");
    toggle(left);
    toggle(right);
    send(&simulation, Side::Right);
    send(&simulation, Side::Left);
    assert_eq!(text(&simulation, Side::Left), "- [x] task\n");
    assert_eq!(text(&simulation, Side::Right), "- [x] task\n");
}
