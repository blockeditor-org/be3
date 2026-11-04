use super::*;

#[test]
fn closing_a_window_tab_asks_its_program_to_close() {
    let (mut fixture, _) = editor();
    fixture.show_windows(&[window(3, "Terminal", None)]);
    fixture.settle();

    fixture.close_active_tab();

    assert_eq!(
        fixture.closed_windows(),
        vec![3],
        "the host is asked to close the window"
    );
    assert_eq!(
        placed_windows(&fixture)
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>(),
        vec![3],
        "the tab stays while its program decides"
    );

    fixture.show_windows(&[]);

    assert!(
        placed_windows(&fixture).is_empty(),
        "the tab goes when its program closes the window"
    );
}
