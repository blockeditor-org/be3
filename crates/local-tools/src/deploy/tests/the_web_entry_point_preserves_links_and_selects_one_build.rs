use super::*;

#[test]
fn the_web_entry_point_preserves_links_and_selects_one_build() {
    let html = web_entry_point(
        "<html><head><script type=\"importmap\"></script></head></html>",
        "0123456789abcdef",
    )
    .unwrap();
    assert!(html.contains("<base href=\"/web/0123456789abcdef/\">"));
    assert!(html.find("<base").unwrap() < html.find("<script").unwrap());
    assert!(!html.contains("location.replace"));
    assert!(web_entry_point("not an HTML document", "hash").is_err());
    for (name, mime) in [
        ("app.wasm", "application/wasm"),
        ("worker.js", "text/javascript; charset=utf-8"),
        ("index.html", "text/html; charset=utf-8"),
    ] {
        assert_eq!(web_content_type(Path::new(name)), mime);
    }
}
