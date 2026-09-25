use super::*;

#[test]
fn piped_output_returns_to_the_first_column_on_each_newline() {
    assert_eq!(
        with_carriage_returns(b"one\ntwo\r\nthree\rfour"),
        b"one\r\ntwo\r\r\nthree\rfour".to_vec()
    );
}
