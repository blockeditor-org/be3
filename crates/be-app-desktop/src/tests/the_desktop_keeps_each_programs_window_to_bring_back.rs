use be_sdk_beui::beui::{Pos2, Vec2};

use be_sdk_shell::ProgramRecord;

use super::*;

#[test]
fn the_desktop_keeps_each_programs_window_to_bring_back() {
    let mut fixture = Fixture::new();
    fixture.settle();
    let full = TestWindow {
        fullscreen: Some(Rect::from_min_size(
            Pos2::new(0.0, 0.0),
            Vec2::new(800.0, 600.0),
        )),
        ..notes(2)
    };
    let dialog = TestWindow {
        parent: Some(HostWindowId(2)),
        title: "Save changes?".to_owned(),
        ..notes(3)
    };
    fixture.show_windows(&[foot(1), full, dialog]);

    let records: Vec<ProgramRecord> = fixture.kept().into_iter().map(|kept| kept.record).collect();
    assert_eq!(
        records,
        vec![
            ProgramRecord {
                window: Some(HostWindowId(1)),
                program_id: "foot".to_owned(),
                program: Some("foot.desktop".to_owned()),
                command: vec!["foot".to_owned()],
                title: "Foot \u{2014} Terminal 1".to_owned(),
                display: false,
            },
            ProgramRecord {
                window: Some(HostWindowId(2)),
                program_id: "org.example.Notes".to_owned(),
                program: None,
                command: vec!["/usr/bin/notes".to_owned()],
                title: "Notes \u{2014} Shopping list".to_owned(),
                display: false,
            },
        ],
        "each top-level window is kept with what brings it back, and a dialog is not"
    );

    let kept = fixture.tab_of(1);
    fixture.show_windows(&[foot(1)]);
    assert_eq!(
        kept_keys(&fixture),
        vec![kept],
        "a window that closes is no longer kept"
    );
}
