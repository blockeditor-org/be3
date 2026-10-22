use super::super::lines::summary;

#[test]
fn the_line_summary_names_every_kind_even_when_none_changed() {
    let numstat = "3\t1\tcrates/beui/src/layout.rs\n\
                   10\t0\tcrates/beui/src/tests/a_row_fits.rs\n\
                   2\t2\tcrates/beui/src/tests.rs\n\
                   -\t-\tcrates/beui/icons.ttf\n\
                   5\t0\tcrates/block-app/web/index.js\n";
    assert_eq!(
        summary(numstat),
        "**+3 -1** of Rust (excluding tests and build tooling) in 1 file.\n\n\
         **+12 -2** of tests in 2 files.\n\n\
         **+0 -0** of build system in 0 files.\n\n\
         **+0 -0** of Markdown in 0 files.\n\n\
         **+5 -0** of everything else in 1 file.\n\n"
    );
}
