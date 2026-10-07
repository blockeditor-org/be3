use super::*;

fn reflect_sample_with(target_fn: &str, extra: &str) -> Result<(), Vec<TokenizationError>> {
    let source = include_str!("../../../samples/reflect.qxc").replace(
        "std.reflect.function(toy, clamp)",
        &format!("std.reflect.function(toy, {target_fn})"),
    );
    import_file("reflect.qxc", &format!("{source}\n{extra}")).map(|_| ())
}

#[test]
fn import_file_reflect_generator_rejects_unsupported_types() {
    let word = "Word :: std.Type[]\nkeep :: (w: Word) => Word: w\nlen :: (w: Word) => std.c.int: 0";

    reflect_sample_with("clamp", word).expect("a function no generator reads may use any type");

    let errors = reflect_sample_with("len", word).expect_err("toy has no Word");
    assert_eq!(errors.len(), 1, "{errors:?}");
    let entries = &errors[0].entries;
    assert_eq!(entries[0].message, "toy doesn't support Word");
    let len_line = include_str!("../../../samples/reflect.qxc").lines().count() + 4;
    assert_eq!(entries[0].pos.as_ref().map(|p| p.lyn), Some(len_line));
    assert_eq!(entries[1].message, "reported here");

    let errors = reflect_sample_with("keep", word).expect_err("toy has no Word");
    assert_eq!(errors[0].entries[0].message, "toy doesn't support Word");

    let errors = reflect_sample_with(
        "count",
        "count :: (a: std.c.int) => std.c.int: :out {\n  std.kw.loop {\n    out: a\n  }\n}",
    )
    .expect_err("toy has no loops");
    assert_eq!(
        errors[0].entries[0].message,
        "toy doesn't support std.kw.loop"
    );
}
