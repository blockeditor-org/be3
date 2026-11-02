use crate::BrowserSnapshot;

#[test]
fn browser_paintings_reject_incomplete_resources_and_unknown_versions() {
    assert!(BrowserSnapshot::decode(b"BE3BROWSER 99\n{}").is_err());
    assert!(BrowserSnapshot::decode(b"BE3BROWSER 1\n{}").is_err());
    assert!(BrowserSnapshot::new([0, 800], "").encode().is_err());
    let mut snapshot =
        BrowserSnapshot::new([800, 600], "").file("/style.css", "text/css", b"body {}");
    snapshot.files.get_mut("/style.css").unwrap().text = None;
    assert!(snapshot.encode().is_err());
    assert!(
        BrowserSnapshot::new([800, 600], "")
            .file("relative.css", "text/css", b"")
            .encode()
            .is_err()
    );
}
