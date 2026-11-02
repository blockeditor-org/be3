use super::*;

#[test]
fn the_web_entry_point_preserves_links_and_selects_one_build() {
    let html = web_entry_point("0123456789abcdef");
    assert!(html.contains("<base href=\"/data/0123456789abcdef/web/\">"));
    assert!(html.find("<base").unwrap() < html.find("<script").unwrap());
    assert!(!html.contains("location.replace"));
    assert!(html.contains("<script type=\"importmap\">"));
    assert!(html.contains("<!doctype html><html"));
    for (name, mime) in [
        ("app.wasm", "application/wasm"),
        ("worker.js", "text/javascript; charset=utf-8"),
        ("index.html", "text/html; charset=utf-8"),
    ] {
        assert_eq!(web_content_type(Path::new(name)), mime);
    }
}
