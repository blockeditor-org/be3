use super::*;
use rustix::process::Pid;

#[test]
fn a_process_the_compositor_launched_can_be_ended() {
    let mut harness = Harness::new();
    let fifo = std::env::temp_dir().join(format!("be-wayland-launched-{}", std::process::id()));
    let _ = std::fs::remove_file(&fifo);
    rustix::fs::mknodat(
        rustix::fs::CWD,
        &fifo,
        rustix::fs::FileType::Fifo,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        0,
    )
    .expect("a fifo is made");

    harness.app.windows().run(Launch {
        arguments: vec![
            "sh".to_owned(),
            "-c".to_owned(),
            format!("sleep 1000 & echo $! > '{}'; wait", fifo.display()),
        ],
        working_dir: None,
    });
    harness.frame(Vec::new());
    let told = std::fs::read_to_string(&fifo).expect("the shell names its child");
    let _ = std::fs::remove_file(&fifo);
    let program = Pid::from_raw(told.trim().parse().expect("a pid")).expect("the pid is not zero");

    assert!(
        harness.app.launched(program),
        "a descendant of a launched process counts as launched"
    );
    assert!(
        !harness
            .app
            .launched(Pid::from_raw(std::process::id() as i32).unwrap()),
        "a process the compositor did not launch does not"
    );

    assert!(harness.app.end_launched(program));
    harness
        .app
        .exited
        .1
        .recv_timeout(std::time::Duration::from_secs(60))
        .expect("the shell exits once its program is killed");
}
