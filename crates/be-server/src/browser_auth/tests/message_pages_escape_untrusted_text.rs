use super::*;

#[test]
fn message_pages_escape_untrusted_text() {
    let page = message("<script>bad</script>", "<&\"'");
    assert!(page.contains("<h1>&lt;script&gt;bad&lt;/script&gt;</h1>"));
    assert!(page.contains("<p>&lt;&amp;&quot;&#39;</p>"));
    assert!(!page.contains("<script>"));
}
