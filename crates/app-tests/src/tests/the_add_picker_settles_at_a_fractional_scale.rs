use super::*;

#[test]
fn the_add_picker_settles_at_a_fractional_scale() {
    let mut app = AppTest::start();
    open_add_to_my_files(&mut app);
    app.wait("#picker.import");
    app.run(&["settle"]);
    for size in ["800x500@1.5", "800x500@1.25", "800x500@1.75", "1100x720@1"] {
        app.run(&["--no-settle", "resize", size]);
        app.run(&["settle"]);
        app.wait("#picker.import");
    }
    app.close();
}
