use super::*;

#[test]
fn a_page_renders_the_same_twice_and_escapes_what_it_shows() {
    let message = page(Screen::Message {
        title: "<script>bad</script>".into(),
        text: "<&\"'".into(),
    });
    let first = render(&message);
    assert_eq!(first, render(&message));
    assert!(first.contains("&lt;script&gt;bad&lt;/script&gt;"));
    assert!(!first.contains("<script>"));
    let hydrated = Page {
        assets: "/auth/assets/v/".into(),
        ..message
    };
    let page = render(&hydrated);
    assert!(page.contains("src=\"/auth/assets/v/start.js\""));
    assert!(page.contains(&crate::import_map("/auth/assets/v/")));
    assert!(page.contains("url(\"/auth/assets/v/Inter-Regular.ttf\")"));
    assert!(!page.contains("<script>"));
}
