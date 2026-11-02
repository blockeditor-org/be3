use super::*;

#[test]
fn choosing_a_scale_draws_the_host_larger() {
    let mut host = HostTest::start();
    host.click("Tab \"Settings\"");
    host.click("#settings.display");
    host.click("#display.scale");
    host.key("Home");
    for _ in 0..5 {
        host.key("Down");
    }
    host.key("Enter");
    host.wait("Scale now applies on your devices");
    host.run(&["settle"]);
    host.snapshot("choosing_a_scale_draws_the_host_larger");
    host.close();
}
