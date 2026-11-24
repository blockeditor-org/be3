use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_shows_a_presentations_slides_as_previews() {
    // be-host's own UI makes a presentation with a title page; started again with the root in
    // place of its UI, the presentation's filmstrip places the slide as a preview, which the root
    // shows as the slide canvas's preview view in its place, and the Files and the tab name the
    // presentation by its type, from the host's catalog the root tells its apps.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    add_to_my_files(&mut host, "Presentation");
    host.click("#presentation.add");
    host.wait("#picker.slides");
    host.click("MenuItem \"Title page\"");
    host.gone("#picker.slides");
    host.wait("#infinite-canvas.dock");
    host.run(&["settle"]);
    host.restart_as_root(ROOT);
    // A preview painted again soon after it was is held a while (be-sdk's `Cooldown`), as under
    // be-host: the stopped clock moved past it lets the slide's last preview land. Without it a
    // glyph of the preview shows at a stray place in some runs (plans/host_as_platform_adapter.md,
    // 4g7b3d).
    host.pause(600);
    host.run(&["settle"]);
    host.snapshot("be_root_shows_a_presentations_slides_as_previews");
    host.close();
}
