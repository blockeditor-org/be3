use crate::BrowserSnapshot;

#[test]
fn browser_paintings_keep_the_document_and_every_resource_as_text() {
    let html = "<!doctype html><html><head><link rel=\"stylesheet\" href=\"/style.css\"></head><body><p title=\"a > b\">Hello</p></body></html>";
    let snapshot = BrowserSnapshot::new([390, 844], html)
        .file("/style.css", "text/css", b"body {\n color: red;\n}\n")
        .file("/script.js", "text/javascript", b"document.title = 'test';")
        .file("/image.png", "image/png", &[0, 255, 1]);
    let bytes = snapshot.encode().unwrap();
    assert!(BrowserSnapshot::is_browser(&bytes));
    let text = std::str::from_utf8(&bytes).unwrap();
    assert!(text.contains("color: red;"));
    let restored = BrowserSnapshot::decode(&bytes).unwrap();
    assert_eq!(snapshot, restored);
    assert_eq!(restored.document.concat(), html);
    assert_eq!(
        restored.files["/style.css"].text.as_ref().unwrap().concat(),
        "body {\n color: red;\n}\n"
    );
    assert_eq!(
        restored.files["/image.png"].bytes.as_deref(),
        Some(&[0, 255, 1][..])
    );
    assert_eq!(restored.encode().unwrap(), bytes);
}
