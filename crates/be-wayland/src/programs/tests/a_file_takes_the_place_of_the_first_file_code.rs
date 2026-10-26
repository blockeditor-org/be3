use super::*;

#[test]
fn a_file_takes_the_place_of_the_first_file_code() {
    let book = Some("/data/files/book.epub");
    assert_eq!(
        arguments(&entry("koreader %f", ""), book).expect("the line expands"),
        ["koreader", "/data/files/book.epub"]
    );
    assert_eq!(
        arguments(&entry("viewer --open %U --then %F", ""), book).expect("the line expands"),
        ["viewer", "--open", "/data/files/book.epub", "--then"],
        "the file is given once, at the first code"
    );
    assert_eq!(
        arguments(&entry("reader --fullscreen", ""), book).expect("the line expands"),
        ["reader", "--fullscreen", "/data/files/book.epub"],
        "a line with no file code takes the file at its end"
    );
    assert_eq!(
        arguments(&entry("%f", ""), book),
        Err(ExecError::Empty),
        "a file alone names no program"
    );
}
