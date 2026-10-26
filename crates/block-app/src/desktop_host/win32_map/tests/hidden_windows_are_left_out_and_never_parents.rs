use super::*;

#[test]
fn hidden_windows_are_left_out_and_never_parents() {
    let parent = window(1, Rect::new(0, 0, 100, 100));
    let mut child = window(2, Rect::new(0, 0, 50, 50));
    child.parent = Some(EntryId(1));
    let snapshot = snapshot(vec![parent, child]);
    let listed = listed(&snapshot, &[HostWindowId(1)], &Shaping::default());
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, HostWindowId(2));
    assert_eq!(listed[0].parent, None);
}
