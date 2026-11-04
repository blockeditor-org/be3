use super::*;

#[test]
fn the_login_session_entry_runs_the_installed_desktop_on_the_displays() {
    let entry = session_entry(Path::new("/opt/be-host/lib/be-host/be-host"));

    assert!(entry.starts_with("[Desktop Entry]\n"));
    assert!(
        entry.contains("\nExec=env BEUI_PLATFORM=drm /opt/be-host/lib/be-host/be-host --desktop\n")
    );
    assert!(entry.contains("\nTryExec=/opt/be-host/lib/be-host/be-host\n"));
    assert!(entry.contains("\nName=be-host\n"));
    assert!(entry.contains("\nDesktopNames=be-host\n"));
    assert!(
        entry.contains("\nX-LightDM-Session-Type=wayland\n"),
        "LightDM runs it as Wayland whichever directory it is in"
    );
}
