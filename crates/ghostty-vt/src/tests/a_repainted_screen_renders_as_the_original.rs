use super::*;

#[test]
fn a_repainted_screen_renders_as_the_original() {
    let original = render(
        20,
        4,
        "plain\r\n\u{1b}[1;31mred bold\u{1b}[0m \u{1b}[44mblue\u{1b}[0m\r\n\u{1b}[7minverse\u{1b}[0m wide 世界!\r\n\u{1b}[3;4mlast\u{1b}[0m\u{1b}[2;6H",
    );

    let repainted = render(20, 4, std::str::from_utf8(&original.repaint()).unwrap());

    assert_eq!(repainted.text(), original.text());
    assert_eq!(repainted.cursor, original.cursor);
    assert_eq!(repainted.rows, original.rows);
}
