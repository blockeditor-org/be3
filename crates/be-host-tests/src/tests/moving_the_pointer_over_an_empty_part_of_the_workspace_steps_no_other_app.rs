use super::*;

#[test]
fn moving_the_pointer_over_an_empty_part_of_the_workspace_steps_no_other_app() {
    let mut host = HostTest::start(Options::default());
    host.wait("Pane \"Files\"");
    let (x, y, width, height) = host.place("Heading \"No file open\"");
    let x = x + width / 2.0;
    let y = y + height * 10.0;
    host.run(&["move", &format!("{x},{y}")]);
    host.run(&["frames"]);
    for step in 1..=5 {
        let at = format!("{},{}", x + 10.0 * step as f32, y + 10.0 * step as f32);
        host.run(&["move", &at]);
    }
    let frames = host.run(&["frames"]);
    assert!(
        frames.contains("be3.workspace-ui:"),
        "the workspace under the pointer took its moves:\n{frames}"
    );
    assert!(
        !frames.contains("be3.file-tree:"),
        "the file tree, away from the pointer, was not stepped:\n{frames}"
    );
    host.close();
}
