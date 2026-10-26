use super::super::docs::painting_references;

#[test]
fn the_docs_name_the_paintings_they_show() {
    let markdown = "Intro ![A counter](paint:beui-demo.counter#frame=1) and `![no](paint:inline.code)`.\n\n```markdown\n![no](paint:in.a.fence)\n```\n\n![Two](paint:a.one) ![Three](paint:a.two)\n";

    assert_eq!(
        painting_references(markdown),
        [
            (1, "beui-demo.counter".to_owned()),
            (7, "a.one".to_owned()),
            (7, "a.two".to_owned()),
        ]
    );
}
