use super::*;

#[test]
fn the_restore_chip_leads_the_windows_bar_while_windows_from_last_time_wait() {
    let mut before = Fixture::new();
    before.settle();
    before.show_windows(&[foot(1), notes(6)]);

    let mut resumed = Fixture::resumed(before.saved());
    resumed.show_windows(&[host_window(9, true)]);
    assert!(resumed.says("Restore 2"), "it counts the windows waiting");
    assert!(
        resumed.test.rect_of("desktop.windows.restore").left()
            < resumed.test.rect_of("desktop.windows.9").left(),
        "and leads the row"
    );

    resumed.test.clear_calls();
    resumed.test.click("desktop.windows.restore");
    resumed.settle();
    assert_eq!(
        resumed
            .test
            .take_host_calls(block_ui_test::requested::program)
            .len(),
        2,
        "it starts every program waiting"
    );
}
