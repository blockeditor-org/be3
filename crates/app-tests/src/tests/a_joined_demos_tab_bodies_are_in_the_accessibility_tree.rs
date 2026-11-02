use super::*;

#[test]
fn a_joined_demos_tab_bodies_are_in_the_accessibility_tree() {
    let mut host = HostTest::start();
    open_beui_demo(&mut host);
    host.wait("#demo.catalog.Docking");
    host.wait("Heading \"The dock around this page\"");
    host.click("#demo.catalog.Text");
    host.wait("#demo.code.Text styles");
    host.wait("Label value=\"Body text for labels and values.\"");
    host.close();
}
