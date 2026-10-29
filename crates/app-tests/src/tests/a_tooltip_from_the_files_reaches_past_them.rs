use super::*;

#[test]
fn a_tooltip_from_the_files_reaches_past_them() {
    let mut app = AppTest::start();
    app.wait(ADD_TO_MY_FILES);
    app.run(&["move", ADD_TO_MY_FILES]);
    app.run(&["pause", "1500"]);
    let (files_x, _, files_width, _) = app.place("#workspace.files");
    let (tip_x, _, tip_width, _) = app.place("#tooltip");
    assert!(
        tip_x + tip_width > files_x + files_width,
        "the tooltip keeps its size and reaches past the files: {tip_x} + {tip_width}"
    );
    app.snapshot("a_tooltip_from_the_files_reaches_past_them");
    app.close();
}
