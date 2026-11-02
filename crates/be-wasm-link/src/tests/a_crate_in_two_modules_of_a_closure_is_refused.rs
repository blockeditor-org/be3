use super::*;

#[test]
fn a_crate_in_two_modules_of_a_closure_is_refused() {
    let error = closure(vec![
        library("base", &[], &["libstd-1.rlib"], &[]),
        library("left", &["base"], &["libpdfium-7.rlib"], &[]),
        library("right", &["base"], &["libpdfium-7.rlib"], &[]),
    ])
    .unwrap_err();
    assert!(
        error.contains("the crate pdfium") && error.contains("left") && error.contains("right"),
        "{error}"
    );
    let error = closure(vec![
        library("base", &[], &["-lc++"], &[]),
        library("left", &["base"], &["-lc++"], &[]),
    ])
    .unwrap_err();
    assert!(error.contains("the C library c++"), "{error}");
}
