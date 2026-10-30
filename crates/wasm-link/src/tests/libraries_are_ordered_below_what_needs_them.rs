use super::*;

#[test]
fn libraries_are_ordered_below_what_needs_them() {
    let ordered = closure(vec![
        library("beui", &["base"], &[], &[]),
        library("pdfium", &["base", "beui"], &[], &[]),
        library("base", &[], &[], &[]),
    ])
    .unwrap();
    let names: Vec<&str> = ordered.iter().map(|library| library.name.as_str()).collect();
    assert_eq!(names, ["base", "beui", "pdfium"]);
}
