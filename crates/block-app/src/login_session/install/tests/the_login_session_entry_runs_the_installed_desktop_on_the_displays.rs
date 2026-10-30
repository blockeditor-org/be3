use super::*;

#[test]
fn the_login_session_entry_runs_the_installed_desktop_on_the_displays() {
    let entry = session_entry(Path::new("/opt/block/lib/block-app/block-app"));

    assert!(entry.starts_with("[Desktop Entry]\n"));
    assert!(entry.contains("\nExec=env BEUI_PLATFORM=drm /opt/block/lib/block-app/block-app --desktop\n"));
    assert!(entry.contains("\nTryExec=/opt/block/lib/block-app/block-app\n"));
    assert!(entry.contains("\nName=Block\n"));
    assert!(
        entry.contains("\nX-LightDM-Session-Type=wayland\n"),
        "LightDM runs it as Wayland whichever directory it is in"
    );
}
