use super::*;

#[test]
fn typing_after_a_pause_undoes_as_its_own_step() {
    let mut host = HostTest::start();
    add_to_my_files(&mut host, "Text");
    host.click("#text.surface");
    host.type_text("Quickly");
    host.type_text(" typed");
    host.wait("Quickly typed");
    host.pause(1000);
    host.type_text(" then later");
    host.wait("Quickly typed then later");
    host.key("ctrl+z");
    host.gone("then later");
    host.wait("Quickly typed");
    host.key("ctrl+z");
    host.gone("Quickly");
    host.close();
}
