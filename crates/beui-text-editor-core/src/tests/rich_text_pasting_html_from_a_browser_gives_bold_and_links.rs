use super::*;

#[test]
fn rich_text_pasting_html_from_a_browser_gives_bold_and_links() {
    let html = "<html><head><style>b { color: red }</style></head><body>\
        <!--StartFragment--><h2>A &amp; B</h2>\
        <p>Some <b>bold</b>, <a href=\"https://a.test/?q=1&amp;r=2\">a  link</a>,\n\
        <span style=\"font-weight: 700\">heavy</span>, \
        <b style=\"font-weight:normal\">light</b>, \
        <a href=\"javascript:alert(1)\">no link</a><script>alert(2)</script>\
        <i>x < y</i></p><ul><li>one</li><li>two<ol><li>nested</li></ol></li></ul>\
        <!--EndFragment--></body></html>";
    let schema = schema::basic::Basic;
    let fragment = schema.from_html(html).expect("the basic schema reads html");
    let mut tester = EditorTester::rich(b"");
    tester.execute(EditorCommand::Paste(&fragment));
    assert_eq!(
        formatted(&tester),
        "[heading=2]A & B\n\
         Some {bold}bold{}, {link=https://a.test/?q=1&r=2}a link{}, {bold}heavy{}, light, \
         no link{italic}x < y{}\n\
         [list=bullet]one\n\
         [list=bullet]two\n\
         [indent=1,list=number]nested",
        "bold and links come through, a script and an unsafe link do not, \
         and what the schema has no mark for is kept as text"
    );

    for hostile in ["<", "<a", "<a href='", "&#xffffffff;", "<<<>>>", "</b></p>", "<!--"] {
        let read = schema.from_html(hostile).expect("any input reads");
        assert!(read.chunks[0].marks.iter().all(|mark| mark.range.end <= read.chunks[0].bytes.len()));
    }
}
