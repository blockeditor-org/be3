use super::*;

#[test]
fn typing_after_a_pause_undoes_as_its_own_step() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Text");
    app.click("#text.surface");
    app.type_text("Quickly");
    app.type_text(" typed");
    app.wait("Quickly typed");
    app.pause(1000);
    app.type_text(" then later");
    app.wait("Quickly typed then later");
    app.key("ctrl+z");
    app.gone("then later");
    app.wait("Quickly typed");
    app.key("ctrl+z");
    app.gone("Quickly");
    app.close();
}
