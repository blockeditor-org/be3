use super::*;

#[test]
fn checking_a_box_against_a_check_and_uncheck_keeps_it_checked() {
    let (simulation, left, right) = simulated("- [ ] task\n");
    toggle(left);
    let right = Arc::new(right);
    toggle(Arc::clone(&right));
    toggle(right);
    send(&simulation, Side::Right);
    send(&simulation, Side::Left);
    assert_eq!(text(&simulation, Side::Left), "- [x] task\n");
    assert_eq!(text(&simulation, Side::Right), "- [x] task\n");
}
