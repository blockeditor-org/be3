use super::*;

#[test]
fn unchanged_browser_paintings_do_not_start_the_rasterizer() {
    let root = std::env::temp_dir().join(format!("be3-browser-previews-{}", std::process::id()));
    let before = root.join("before");
    let after = root.join("after");
    let output = root.join("output");
    std::fs::create_dir_all(&before).unwrap();
    std::fs::create_dir_all(&after).unwrap();
    let snapshot = BrowserSnapshot::new([390, 844], "<html><body>Hello</body></html>").file(
        "/style.css",
        "text/css",
        b"body { color: red; }",
    );
    for directory in [&before, &after] {
        std::fs::write(
            directory.join("sample.browser.paint"),
            snapshot.encode().unwrap(),
        )
        .unwrap();
    }
    let renderer = root.join("missing-rasterizer");
    let comment = generate(&before, &after, &output, Some(&renderer)).unwrap();
    assert!(comment.contains("No paintings changed."));
    let changed = snapshot.file("/style.css", "text/css", b"body { color: blue; }");
    std::fs::write(
        after.join("sample.browser.paint"),
        changed.encode().unwrap(),
    )
    .unwrap();
    assert!(generate(&before, &after, &output, Some(&renderer)).is_err());
    std::fs::remove_dir_all(root).unwrap();
}
