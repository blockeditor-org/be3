use super::*;

#[test]
fn the_demo_floats_its_panes_into_windows_where_a_host_opens_windows() {
    let (year, month, day) = TODAY;
    let document = DemoApp::on(Date::new(year, month, day)).document;
    let mut test = DocumentTest::hosting_windows(document, WIDE);
    test.frame(Vec::new());
    test.frame(Vec::new());

    let opened = test.windows().open.clone();
    let [catalog, ..] = opened.as_slice() else {
        panic!("the demo floats its panes into windows, but opened none");
    };
    test.snapshot("demo_floating_only_base");
    test.snapshot_window("demo_floating_only_catalog_window", catalog.id);
}
