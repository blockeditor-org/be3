use super::*;

#[test]
fn a_session_entry_quotes_a_path_with_reserved_characters() {
    let entry = session_entry(Path::new("/opt/my apps/100%/block-desktop"));

    assert!(entry.contains("\nExec=\"/opt/my apps/100%%/block-desktop\" --session\n"));
    assert!(entry.contains("\nTryExec=/opt/my apps/100%/block-desktop\n"));
}
