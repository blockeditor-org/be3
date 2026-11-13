use super::*;

#[test]
fn typing_after_a_pause_undoes_as_its_own_step() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Text");
    host.click("#text.surface");
    host.type_text("Quick");
    host.wait("Quick");
    host.pause(1000);
    host.type_text("ly");
    host.wait("Quickly");
    host.key("ctrl+z");
    host.gone("Quickly");
    host.wait("Quick");
    host.key("ctrl+z");
    host.gone("Quick");
    host.close();
}
