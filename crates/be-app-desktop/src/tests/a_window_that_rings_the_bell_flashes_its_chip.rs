use std::time::Duration;

use super::*;

#[test]
fn a_window_that_rings_the_bell_flashes_its_chip() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.show_windows(&[host_window(1, true), host_window(2, false)]);
    fixture.settle();

    fixture.show_windows(&[
        host_window(1, true),
        HostWindow {
            rung: 1,
            ..host_window(2, false)
        },
    ]);
    fixture
        .test
        .snapshot("a_window_that_rings_the_bell_flashes_its_chip");

    fixture.test.advance(Duration::from_secs(1));
    fixture.settle();
    fixture
        .test
        .snapshot("a_window_that_rings_the_bell_flashes_its_chip_then_settles");
}
