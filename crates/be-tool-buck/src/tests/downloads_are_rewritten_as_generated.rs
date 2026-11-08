use crate::generate::{checksums, downloads, render_downloads};
use crate::lock::pinned_sizes;

#[test]
fn downloads_are_rewritten_as_generated() {
    let lock = "[[package]]\nname = \"a\"\nversion = \"1.0.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"aa\"\n\n[[package]]\nname = \"local\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"b\"\nversion = \"0.2.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"bb\"\n";
    let checksums = checksums(lock);
    let mut sizes = [("a-1.0.0".to_owned(), 10)].into_iter().collect();
    assert!(downloads(&checksums, &sizes).is_err());
    sizes.insert("b-0.2.0".to_owned(), 20);
    let rendered = render_downloads(downloads(&checksums, &sizes).unwrap());
    assert_eq!(pinned_sizes(&rendered), sizes);
    assert!(rendered.starts_with("downloads = {\n    \"a-1.0.0\": {\n"));
}
