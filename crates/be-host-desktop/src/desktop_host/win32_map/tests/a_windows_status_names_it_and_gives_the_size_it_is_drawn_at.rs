use super::*;

use crate::window_body::WindowStatus;

#[test]
fn a_windows_status_names_it_and_gives_the_size_it_is_drawn_at() {
    let mut entry = window(5, Rect::new(100, 100, 900, 700));
    entry.title = "Untitled - Paint".to_owned();
    entry.raised = 2;
    entry.responding = false;
    let snapshot = snapshot(vec![entry.clone()]);
    let mut shaping = Shaping::default();
    shaping.observe(&snapshot);
    let record = host_window(
        &snapshot,
        &entry,
        &shaping,
        &Notes::default(),
        &ProgramNames::default(),
    );
    let drawn = drawn_size(&snapshot, &entry);
    assert_eq!(
        drawn,
        Vec2::new(400.0, 300.0),
        "a window is drawn at its frame, in its monitor's points"
    );
    let status = WindowStatus::of(&record, None, drawn);
    assert_eq!(status.title, "Paint \u{2014} Untitled");
    assert_eq!(status.name, "Paint");
    assert_eq!(status.group, "C:\\Programs\\paint.exe");
    assert_eq!(status.raised, 2);
    assert!(!status.responding);
    assert_eq!(status.intrinsic(), Some(Vec2::new(400.0, 300.0)));
}
