use super::*;

#[test]
fn a_window_the_host_runs_floats_until_its_program_closes_it() {
    let mut fixture = Fixture::new();
    fixture.settle();

    fixture
        .test
        .set_host_value::<HostWindows>(&vec![window(7, "Terminal", None)]);
    fixture.settle();

    assert_eq!(
        placed_windows(&fixture)
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>(),
        vec![7],
        "the window is placed for the host to draw"
    );
    assert!(fixture.says("Terminal"), "its tab carries its title");

    fixture
        .test
        .set_host_value::<HostWindows>(&vec![window(7, "Terminal - ~", None)]);
    fixture.settle();
    assert!(fixture.says("Terminal - ~"), "the tab follows the title");

    fixture.test.set_host_value::<HostWindows>(&Vec::new());
    fixture.settle();

    assert!(placed_windows(&fixture).is_empty());
    assert!(!fixture.says("Terminal"), "the tab goes with the window");
}
