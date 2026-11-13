use super::*;

#[test]
fn the_add_picker_settles_at_a_fractional_scale() {
    let mut host = HostTest::start(Options::default());
    open_add_to_my_files(&mut host);
    host.wait("#picker.import");
    host.run(&["settle"]);
    for size in ["800x500@1.5", "800x500@1.25", "800x500@1.75", "1100x720@1"] {
        host.run(&["--no-settle", "resize", size]);
        host.run(&["settle"]);
        host.wait("#picker.import");
    }
    host.close();
}
