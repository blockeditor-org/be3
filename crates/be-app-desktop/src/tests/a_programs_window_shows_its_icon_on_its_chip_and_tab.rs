use super::*;

#[test]
fn a_programs_window_shows_its_icon_on_its_chip_and_tab() {
    const SIDE: u32 = 64;
    let mut fixture = Fixture::new();
    fixture.settle();
    let rgba = (0..SIDE * SIDE)
        .flat_map(|at| {
            let (x, y) = (at % SIDE, at / SIDE);
            let inside = (8..56).contains(&x) && (8..56).contains(&y);
            let prompt = (16..28).contains(&x) && (20..32).contains(&y) && x - 16 <= y - 20;
            match (inside, prompt) {
                (true, true) => [0xf5, 0xf5, 0xf5, 0xff],
                (true, false) => [0x26, 0x32, 0x38, 0xff],
                (false, _) => [0, 0, 0, 0],
            }
        })
        .collect();
    fixture.show_windows(&[HostWindow {
        focused: true,
        icon: Some(HostImage {
            width: SIDE,
            height: SIDE,
            rgba,
        }),
        ..foot(1)
    }]);
    fixture.settle();
    fixture
        .test
        .snapshot("a_programs_window_shows_its_icon_on_its_chip_and_tab");
}
