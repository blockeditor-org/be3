use super::*;

#[test]
fn the_session_entry_runs_the_installed_app_in_session_mode() {
    let entry = session_entry(Path::new("/opt/block/lib/block-app/block-app"));

    assert!(entry.starts_with("[Desktop Entry]\n"));
    assert!(entry.contains("\nExec=/opt/block/lib/block-app/block-app --session\n"));
    assert!(entry.contains("\nTryExec=/opt/block/lib/block-app/block-app\n"));
    assert!(entry.contains("\nName=Block\n"));
}
