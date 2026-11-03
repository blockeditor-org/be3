use super::*;

#[test]
fn a_launched_window_fills_the_tab_it_was_launched_for() {
    let mut programs = Programs::new();
    programs.insert(TabId::new(4), waiting("foot"));
    programs.insert(TabId::new(7), waiting("foot"));
    let mut next = 10;
    let gone = place(
        &mut programs,
        &[window(1, "foot", Some(7))],
        false,
        &mut next,
        |_| true,
    );
    assert!(gone.is_empty());
    assert_eq!(
        bound(&programs),
        vec![(4, None), (7, Some(1))],
        "the launch id names the tab, ahead of an older placeholder of the program"
    );
    assert_eq!(next, 10, "no tab was added");
}
