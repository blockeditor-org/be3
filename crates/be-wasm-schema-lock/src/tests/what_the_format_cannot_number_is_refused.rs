use super::*;

#[test]
fn what_the_format_cannot_number_is_refused() {
    for (source, error) in [
        ("struct Pair(u32, u32);", "name its fields"),
        (
            "struct Sized<const N: usize> { t: [u8; N] }",
            "no const parameters",
        ),
        ("enum Shape { Line(f64, f64) }", "make it a struct variant"),
        (
            "enum Shape { #[schema(default)] Line(f64) }",
            "only a unit variant",
        ),
        (
            "struct Page { offset: usize }",
            "Page.offset: usize is as wide as a pointer",
        ),
        (
            "struct Offsets(Vec<isize>);",
            "Offsets: isize is as wide as a pointer",
        ),
        ("enum Seek { To(usize) }", "Seek::To: usize"),
    ] {
        let source = format!("#[derive(Schema)] #[schema(unfrozen)] {source}");
        one_error(lock_of(&[("a.rs", &source)], &Lock::default()), error);
    }
}
