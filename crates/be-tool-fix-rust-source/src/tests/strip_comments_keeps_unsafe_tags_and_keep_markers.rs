use crate::strip_comments as strip;

#[test]
fn strip_comments_keeps_unsafe_tags_and_keep_markers() {
    let source = br#"// unsafe(ffi)
unsafe extern "C" {
    fn abs(value: i32) -> i32;
}
fn main() {
    // removed
    // unsafe(memory)
    let value = unsafe { std::mem::transmute::<u32, f32>(1) }; // unsafe(ffi)
    // a note @yeskeep
    let other = 1; /* @yeskeep block */
}
"#;

    let stripped = String::from_utf8(strip(source).unwrap()).unwrap();

    assert_eq!(
        stripped,
        r#"// unsafe(ffi)
unsafe extern "C" {
    fn abs(value: i32) -> i32;
}
fn main() {
    // unsafe(memory)
    let value = unsafe { std::mem::transmute::<u32, f32>(1) };
    // a note @yeskeep
    let other = 1; /* @yeskeep block */
}
"#
    );
}
