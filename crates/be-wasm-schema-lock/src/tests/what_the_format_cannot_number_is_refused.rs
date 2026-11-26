use super::*;

#[test]
fn what_the_format_cannot_number_is_refused() {
    for (source, error) in [
        (
            "#[derive(Schema)] struct Pair(u32, u32);",
            "name its fields",
        ),
        (
            "#[derive(Schema)] struct Boxed<T> { t: T }",
            "no type parameters",
        ),
        (
            "#[derive(Schema)] enum Shape { Line(f64, f64) }",
            "make it a struct variant",
        ),
        (
            "#[derive(Schema)] enum Shape { #[schema(default)] Line(f64) }",
            "only a unit variant",
        ),
    ] {
        one_error(lock_of(&[("a.rs", source)], &Lock::default()), error);
    }
}
