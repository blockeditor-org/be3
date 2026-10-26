use super::*;

#[test]
fn a_launched_program_is_given_a_token_that_brings_a_window_forward() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let (window, id) = harness.open();
    harness.click("test.input");
    assert_eq!(harness.compositor().windows().focused(), None);

    let fifo = std::env::temp_dir().join(format!("be-wayland-token-{}", std::process::id()));
    let _ = std::fs::remove_file(&fifo);
    rustix::fs::mknodat(
        rustix::fs::CWD,
        &fifo,
        rustix::fs::FileType::Fifo,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        0,
    )
    .expect("a fifo is made");
    harness.compositor().windows().run(Launch {
        arguments: vec![
            "sh".to_owned(),
            "-c".to_owned(),
            format!(
                "printf '%s %s' \"$XDG_ACTIVATION_TOKEN\" \"$DESKTOP_STARTUP_ID\" > '{}'",
                fifo.display()
            ),
        ],
        working_dir: None,
        tag: None,
    });
    harness.frame(Vec::new());
    let told = std::fs::read_to_string(&fifo).expect("the program tells its token");
    let _ = std::fs::remove_file(&fifo);
    let (token, startup) = told.split_once(' ').expect("two tokens");
    assert!(
        !token.is_empty(),
        "the program is given XDG_ACTIVATION_TOKEN"
    );
    assert_eq!(token, startup, "and the same token as DESKTOP_STARTUP_ID");

    harness.client.activate(token, &window.surface);
    harness.settle();
    assert_eq!(
        harness.compositor().windows().focused(),
        Some(id),
        "the token the host made for the launch brings a window forward"
    );
    assert!(harness.client.received.keyboard_entered);
}
