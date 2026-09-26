use super::*;

#[test]
fn markdown_becomes_paragraphs_lists_code_images_and_tables() {
    let markdown = "## What\r\n\
        <!-- a comment\nover lines -->\n\
        Adds **bold** and *soft* words, a [link](https://example.com) and `code`,\n\
        wrapped onto two lines.\n\
        \n\
        - first item\n  continued\n\
        1. numbered\n\
        > quoted\n\
        \n\
        ```rust\nfn main() {}\n```\n\
        ![screenshot](https://example.com/a.png) and <img alt=\"b\" src=\"https://example.com/b.png\">\n\
        | Before | After |\n\
        | --- | --- |\n\
        | ![](https://example.com/c.png) | 2 * 3 |\n\
        ---\n";
    assert_eq!(
        blocks(markdown),
        vec![
            Block::Heading("What".to_owned()),
            Block::Paragraph(
                "Adds bold and soft words, a link and code, wrapped onto two lines.".to_owned()
            ),
            Block::Item("first item continued".to_owned()),
            Block::Item("numbered".to_owned()),
            Block::Quote("quoted".to_owned()),
            Block::Code("fn main() {}".to_owned()),
            Block::Image {
                url: "https://example.com/a.png".to_owned(),
                alt: "screenshot".to_owned(),
            },
            Block::Paragraph("and".to_owned()),
            Block::Image {
                url: "https://example.com/b.png".to_owned(),
                alt: "b".to_owned(),
            },
            Block::Table(vec![
                vec![
                    vec![Block::Paragraph("Before".to_owned())],
                    vec![Block::Paragraph("After".to_owned())],
                ],
                vec![
                    vec![Block::Image {
                        url: "https://example.com/c.png".to_owned(),
                        alt: String::new(),
                    }],
                    vec![Block::Paragraph("2 * 3".to_owned())],
                ],
            ]),
            Block::Rule,
        ]
    );
    assert_eq!(
        blocks(
            "<details><summary>Notes</summary><p>One</p><ul><li>two</li><li>three</li></ul></details>"
        ),
        vec![
            Block::Paragraph("Notes".to_owned()),
            Block::Paragraph("One".to_owned()),
            Block::Item("two".to_owned()),
            Block::Item("three".to_owned()),
        ]
    );
}
