use super::*;

use block_editor_beui::HostImage;

fn icon() -> HostImage {
    const SIDE: u32 = 32;
    let rgba = (0..SIDE * SIDE)
        .flat_map(|at| {
            let (x, y) = (at % SIDE, at / SIDE);
            let inside = (x as i32 - 16).pow(2) + (y as i32 - 16).pow(2) < 14 * 14;
            match (inside, (x / 8 + y / 8) % 2 == 0) {
                (false, _) => [0, 0, 0, 0],
                (true, true) => [0xe8, 0x5d, 0x2a, 0xff],
                (true, false) => [0x2a, 0x8c, 0xe8, 0xff],
            }
        })
        .collect();
    HostImage {
        width: SIDE,
        height: SIDE,
        rgba,
    }
}

#[test]
fn a_window_tab_shows_the_icon_its_program_set() {
    let (mut fixture, _) = editor();
    let iconed = block_editor_beui::HostWindow {
        icon: Some(icon()),
        ..window(5, "Pinta", None)
    };
    fixture
        .host
        .set_watched::<WindowsWatch>(&vec![iconed, window(6, "Terminal", None)]);
    fixture.settle();

    assert!(fixture.says("Pinta"));
    fixture
        .test
        .snapshot("a_window_tab_shows_the_icon_its_program_set");
}
