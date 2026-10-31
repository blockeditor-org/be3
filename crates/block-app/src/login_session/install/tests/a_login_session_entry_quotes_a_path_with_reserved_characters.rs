use super::*;

#[test]
fn a_login_session_entry_quotes_a_path_with_reserved_characters() {
    let entry = session_entry(Path::new("/opt/my apps/100%/block-app"));

    assert!(
        entry.contains("\nExec=env BEUI_PLATFORM=drm \"/opt/my apps/100%%/block-app\" --desktop\n")
    );
    assert!(entry.contains("\nTryExec=/opt/my apps/100%/block-app\n"));
}
