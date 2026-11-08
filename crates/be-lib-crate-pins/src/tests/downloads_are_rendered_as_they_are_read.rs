use super::*;

#[test]
fn downloads_are_rendered_as_they_are_read() {
    let lock = "[[package]]\nname = \"a\"\nversion = \"1.0.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"aa\"\n\n[[package]]\nname = \"local\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"b\"\nversion = \"0.2.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"bb\"\n";
    let locked = locked(lock);
    assert_eq!(locked.keys().collect::<Vec<_>>(), ["a-1.0.0", "b-0.2.0"]);
    assert_eq!(
        locked["a-1.0.0"].url(),
        "https://static.crates.io/crates/a/a-1.0.0.crate"
    );
    let mut sizes = [("a-1.0.0".to_owned(), 10)].into_iter().collect();
    assert!(render(&locked, &sizes).is_err());
    sizes.insert("b-0.2.0".to_owned(), 20);
    let rendered = render(&locked, &sizes).unwrap();
    assert_eq!(pinned_sizes(&rendered), sizes);
    assert!(rendered.starts_with("downloads = {\n    \"a-1.0.0\": {\n        \"name\": \"a\",\n"));
    let crates_bzl = format!("crates = {{}}\n\n{rendered}");
    assert_eq!(
        split(&crates_bzl),
        Some(("crates = {}\n\n", rendered.as_str()))
    );
}
