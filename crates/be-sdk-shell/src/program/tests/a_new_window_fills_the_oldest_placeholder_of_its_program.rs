use super::*;

#[test]
fn a_new_window_fills_the_oldest_placeholder_of_its_program() {
    let mut programs = Programs::new();
    programs.insert(TabId::new(3), waiting("notes"));
    programs.insert(TabId::new(5), waiting("foot"));
    programs.insert(TabId::new(6), waiting("foot"));
    let mut next = 8;
    place(
        &mut programs,
        &[window(2, "foot", None), window(9, "kate", None)],
        false,
        &mut next,
        |_| true,
    );
    assert_eq!(
        bound(&programs),
        vec![(3, None), (5, Some(2)), (6, None), (8, Some(9))],
        "a window of a program with no placeholder gets a new tab"
    );
    assert_eq!(next, 9);
}
