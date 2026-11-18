use crate::words;

#[test]
fn words_are_runs_of_word_clusters() {
    let text = "Hello, wo\u{301}rld! snake_case 42\n\u{1F469}x 'don't'";
    let found: Vec<&str> = words(text.as_bytes()).map(|range| &text[range]).collect();
    assert_eq!(
        found,
        ["Hello", "wo\u{301}rld", "snake_case", "42", "x", "don't"]
    );
    assert_eq!(words(b"").count(), 0);
    assert_eq!(words(b" .,").count(), 0);
}
