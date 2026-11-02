use super::*;

#[test]
fn a_response_file_is_split_as_lld_splits_it() {
    assert_eq!(
        tokenize("-L/one\n\"two words\" three\\ four '' five\n"),
        ["-L/one", "two words", "three four", "", "five"]
    );
}
